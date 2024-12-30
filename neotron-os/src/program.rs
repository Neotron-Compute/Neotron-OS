//! Program Loading and Execution

use neotron_api::FfiByteSlice;

use crate::{fs, osprintln, refcell::CsRefCell, API, FILESYSTEM};

#[allow(unused)]
pub static CALLBACK_TABLE: neotron_api::Api = neotron_api::Api {
    open: api_open,
    close: api_close,
    write: api_write,
    read: api_read,
    seek_set: api_seek_set,
    seek_cur: api_seek_cur,
    seek_end: api_seek_end,
    rename: api_rename,
    ioctl: api_ioctl,
    opendir: api_opendir,
    closedir: api_closedir,
    readdir: api_readdir,
    stat: api_stat,
    fstat: api_fstat,
    deletefile: api_deletefile,
    deletedir: api_deletedir,
    chdir: api_chdir,
    dchdir: api_dchdir,
    pwd: api_pwd,
    malloc: api_malloc,
    free: api_free,
};

/// The different kinds of state each open handle can be in
pub enum OpenHandle {
    /// Represents Standard Input
    StdIn,
    /// Represents Standard Output
    Stdout,
    /// Represents Standard Error
    StdErr,
    /// Represents an open file in the filesystem
    File(fs::File),
    /// Represents a closed handle.
    ///
    /// This is the default state for handles.
    Closed,
    /// Represents the audio device,
    Audio,
    /// Represents the framebuffer device
    Gfx(GfxState),
}

/// State for when writing to a graphics device
pub struct GfxState {
    cursor_x: u16,
    cursor_y: u16,
}

impl GfxState {
    fn new() -> GfxState {
        GfxState {
            cursor_x: 0,
            cursor_y: 0,
        }
    }
}

/// The open handle table
///
/// This is indexed by the file descriptors (or handles) that the application
/// uses. When an application says "write to handle 4", we look at the 4th entry
/// in here to work out what they are writing to.
///
/// The table is initialised when a program is started, and any open files are
/// closed when the program ends.
static OPEN_HANDLES: CsRefCell<[OpenHandle; 8]> = CsRefCell::new([
    OpenHandle::Closed,
    OpenHandle::Closed,
    OpenHandle::Closed,
    OpenHandle::Closed,
    OpenHandle::Closed,
    OpenHandle::Closed,
    OpenHandle::Closed,
    OpenHandle::Closed,
]);

/// Ways in which loading a program can fail.
#[derive(Debug)]
#[allow(unused)]
pub enum Error {
    /// A filesystem error occurred
    Filesystem(crate::fs::Error),
    /// An ELF error occurred loading from disk
    ElfFs(neotron_loader::Error<crate::fs::Error>),
    /// An ELF error occurred loading from ROM
    ElfRom(neotron_loader::Error<neotron_loader::traits::SliceError>),
    /// Tried to run when nothing was loaded
    NothingLoaded,
}

impl From<crate::fs::Error> for Error {
    fn from(value: crate::fs::Error) -> Self {
        Error::Filesystem(value)
    }
}

impl From<neotron_loader::Error<crate::fs::Error>> for Error {
    fn from(value: neotron_loader::Error<crate::fs::Error>) -> Self {
        Error::ElfFs(value)
    }
}

impl From<neotron_loader::Error<neotron_loader::traits::SliceError>> for Error {
    fn from(value: neotron_loader::Error<neotron_loader::traits::SliceError>) -> Self {
        Error::ElfRom(value)
    }
}

/// Something the ELF loader can use to get bytes off the disk
struct FileSource {
    file: crate::fs::File,
    buffer: core::cell::RefCell<[u8; Self::BUFFER_LEN]>,
    offset_cached: core::cell::Cell<Option<u32>>,
}

impl FileSource {
    const BUFFER_LEN: usize = 128;

    fn new(file: crate::fs::File) -> FileSource {
        FileSource {
            file,
            buffer: core::cell::RefCell::new([0u8; 128]),
            offset_cached: core::cell::Cell::new(None),
        }
    }

    fn uncached_read(&self, offset: u32, out_buffer: &mut [u8]) -> Result<(), crate::fs::Error> {
        self.file.seek_from_start(offset)?;
        self.file.read(out_buffer)?;
        Ok(())
    }
}

impl neotron_loader::traits::Source for &FileSource {
    type Error = crate::fs::Error;

    fn read(&self, mut offset: u32, out_buffer: &mut [u8]) -> Result<(), Self::Error> {
        for chunk in out_buffer.chunks_mut(FileSource::BUFFER_LEN) {
            if let Some(offset_cached) = self.offset_cached.get() {
                let cached_range = offset_cached..offset_cached + FileSource::BUFFER_LEN as u32;
                if cached_range.contains(&offset)
                    && cached_range.contains(&(offset + chunk.len() as u32 - 1))
                {
                    // Do a fast copy from the cache
                    let start = (offset - offset_cached) as usize;
                    let end = start + chunk.len();
                    chunk.copy_from_slice(&self.buffer.borrow()[start..end]);
                    return Ok(());
                }
            }

            self.file.seek_from_start(offset)?;
            self.file.read(self.buffer.borrow_mut().as_mut_slice())?;
            self.offset_cached.set(Some(offset));
            chunk.copy_from_slice(&self.buffer.borrow()[0..chunk.len()]);

            offset += chunk.len() as u32;
        }

        Ok(())
    }
}

/// Represents the Transient Program Area.
///
/// This is a piece of memory that can be used for loading and executing programs.
///
/// Only one program can be executed at a time.
pub struct TransientProgramArea {
    memory_bottom: *mut u32,
    memory_top: *mut u32,
    last_entry: u32,
}

extern "C" {
    #[cfg(all(target_os = "none", target_arch = "arm"))]
    static mut _tpa_start: u32;
}

impl TransientProgramArea {
    /// Construct a new [`TransientProgramArea`].
    pub unsafe fn new(start: *mut u32, length_in_bytes: usize) -> TransientProgramArea {
        let mut tpa = TransientProgramArea {
            memory_bottom: start,
            memory_top: start.add(length_in_bytes / core::mem::size_of::<u32>()),
            last_entry: 0,
        };

        // You have to take the address of a linker symbol to find out where
        // points to, as the linker can only invent symbols pointing at
        // addresses; it cannot actually put values in RAM.
        #[cfg(all(target_os = "none", target_arch = "arm"))]
        let official_tpa_start: Option<*mut u32> = Some(core::ptr::addr_of_mut!(_tpa_start));

        #[cfg(not(all(target_os = "none", target_arch = "arm")))]
        let official_tpa_start: Option<*mut u32> = None;

        if let Some(tpa_start) = official_tpa_start {
            let range = tpa.as_slice_u32().as_ptr_range();
            if !range.contains(&(tpa_start as *const u32)) {
                panic!("TPA doesn't contain system start address");
            }
            let offset = tpa_start.offset_from(tpa.memory_bottom);
            tpa.memory_bottom = tpa.memory_bottom.offset(offset);
        }

        tpa
    }

    /// Borrow the TPA region as a slice of words
    pub fn as_slice_u32(&mut self) -> &mut [u32] {
        unsafe { core::slice::from_raw_parts_mut(self.memory_bottom, self.size_words()) }
    }

    /// Borrow the TPA region as a slice of bytes
    pub fn as_slice_u8(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                self.memory_bottom as *mut u8,
                self.size_words() * core::mem::size_of::<u32>(),
            )
        }
    }

    /// Size of the TPA in 32-bit words
    fn size_words(&self) -> usize {
        unsafe { self.memory_top.offset_from(self.memory_bottom) as usize }
    }

    /// Loads a program from disk into the Transient Program Area.
    ///
    /// The program must be in the Neotron Executable format.
    pub fn load_program(&mut self, file_name: &str) -> Result<(), Error> {
        osprintln!("Loading /{} from Block Device 0", file_name);

        let file = FILESYSTEM.open_file(file_name, embedded_sdmmc::Mode::ReadOnly)?;

        let source = FileSource::new(file);
        let loader = neotron_loader::Loader::new(&source)?;

        let mut iter = loader.iter_program_headers();
        while let Some(Ok(ph)) = iter.next() {
            if ph.p_vaddr() as *mut u32 >= self.memory_bottom
                && ph.p_type() == neotron_loader::ProgramHeader::PT_LOAD
            {
                osprintln!("Loading {} bytes to 0x{:08x}", ph.p_memsz(), ph.p_vaddr());
                let ram = unsafe {
                    core::slice::from_raw_parts_mut(ph.p_vaddr() as *mut u8, ph.p_memsz() as usize)
                };
                // Zero all of it.
                for b in ram.iter_mut() {
                    *b = 0;
                }
                // Replace some of those zeros with bytes from disk.
                if ph.p_filesz() != 0 {
                    source.uncached_read(ph.p_offset(), &mut ram[0..ph.p_filesz() as usize])?;
                }
            }
        }

        self.last_entry = loader.e_entry();

        Ok(())
    }

    /// Loads a program from disk into the Transient Program Area.
    ///
    /// The program must be in the Neotron Executable format.
    pub fn load_rom_program(&mut self, contents: &[u8]) -> Result<(), Error> {
        let loader = neotron_loader::Loader::new(contents)?;

        let mut iter = loader.iter_program_headers();
        while let Some(Ok(ph)) = iter.next() {
            if ph.p_vaddr() as *mut u32 >= self.memory_bottom
                && ph.p_type() == neotron_loader::ProgramHeader::PT_LOAD
            {
                osprintln!("Loading {} bytes to 0x{:08x}", ph.p_memsz(), ph.p_vaddr());
                let ram = unsafe {
                    core::slice::from_raw_parts_mut(ph.p_vaddr() as *mut u8, ph.p_memsz() as usize)
                };
                // Zero all of it.
                for b in ram.iter_mut() {
                    *b = 0;
                }
                // Replace some of those zeros with bytes from disk.
                if ph.p_filesz() != 0 {
                    ram[0..ph.p_filesz() as usize].copy_from_slice(
                        &contents[ph.p_offset() as usize
                            ..(ph.p_offset() as usize + ph.p_filesz() as usize)],
                    );
                }
            }
        }

        self.last_entry = loader.e_entry();

        Ok(())
    }

    /// Execute a program.
    ///
    /// If the program returns, you get `Ok(<exit_code>)`. The program returning
    /// an exit code that is non-zero is not considered a failure from the point
    /// of view of this API. You wanted to run a program, and the program was
    /// run.
    pub fn execute(&mut self, args: &[&str]) -> Result<i32, Error> {
        if self.last_entry == 0 {
            return Err(Error::NothingLoaded);
        }

        // Record the current video mode
        let api = API.get();
        // TODO: Use VgaConsole to do this?
        let old_mode = (api.video_get_mode)();
        let old_ptr = (api.video_get_framebuffer)();

        // Setup the default file handles
        let mut open_handles = OPEN_HANDLES.lock();
        open_handles[0] = OpenHandle::StdIn;
        open_handles[1] = OpenHandle::Stdout;
        open_handles[2] = OpenHandle::StdErr;
        drop(open_handles);

        // We support a maximum of four arguments.
        #[allow(clippy::get_first)]
        let ffi_args = [
            #[allow(clippy::get_first)]
            neotron_api::FfiString::new(args.get(0).unwrap_or(&"")),
            neotron_api::FfiString::new(args.get(1).unwrap_or(&"")),
            neotron_api::FfiString::new(args.get(2).unwrap_or(&"")),
            neotron_api::FfiString::new(args.get(3).unwrap_or(&"")),
        ];

        let result = unsafe {
            let code: neotron_api::AppStartFn =
                ::core::mem::transmute(self.last_entry as *const ());
            code(&CALLBACK_TABLE, args.len(), ffi_args.as_ptr())
        };

        // Close any files the program left open
        let mut open_handles = OPEN_HANDLES.lock();
        for h in open_handles.iter_mut() {
            *h = OpenHandle::Closed;
        }
        drop(open_handles);

        let mut lock = crate::VGA_CONSOLE.lock();
        if let Some(console) = lock.as_mut() {
            // put the video mode back as it was
            let _ = unsafe { console.change_mode(old_mode, old_ptr) };
        }

        self.last_entry = 0;
        Ok(result)
    }

    /// Move data to the top of TPA and make TPA shorter.
    ///
    /// Moves `size` bytes to the top of the TPA, and then pretends the TPA is
    /// `size` bytes shorter than it was.
    ///
    /// `size` will be rounded up to a multiple of 4.
    ///
    /// Panics if `n` is too big to fit in the TPA.
    ///
    /// Returns a pointer to the data that now sits outside of the TPA. There
    /// will be `size` bytes at this address but you must manage the lifetimes
    /// yourself.
    pub fn steal_top(&mut self, size: usize) -> *const u8 {
        let stolen_words = (size + 3) / 4;
        if stolen_words >= self.size_words() {
            panic!("Stole too much from TPA!");
        }
        unsafe {
            // Top goes down to free memory above it
            let new_top = self.memory_top.sub(stolen_words);
            // Copy the data from the bottom to above the newly reduced TPA
            core::ptr::copy(self.memory_bottom, new_top, stolen_words);
            new_top as *mut u8
        }
    }

    /// Restore the TPA back where it was.
    pub unsafe fn restore_top(&mut self, size: usize) {
        let restored_words = (size + 3) / 4;
        self.memory_top = self.memory_top.add(restored_words);
    }
}

/// Store an open handle, or fail if we're out of space
fn allocate_handle(h: OpenHandle) -> Result<usize, OpenHandle> {
    for (idx, slot) in OPEN_HANDLES.lock().iter_mut().enumerate() {
        if matches!(*slot, OpenHandle::Closed) {
            *slot = h;
            return Ok(idx);
        }
    }
    Err(h)
}

/// Open a file, given a path as UTF-8 string.
///
/// If the file does not exist, or is already open, it returns an error.
///
/// Path may be relative to current directory, or it may be an absolute
/// path.
extern "C" fn api_open(
    path: neotron_api::FfiString,
    _flags: neotron_api::file::Flags,
) -> neotron_api::Result<neotron_api::file::Handle> {
    // Check for special devices
    if path.as_str().eq_ignore_ascii_case("AUDIO:") {
        match allocate_handle(OpenHandle::Audio) {
            Ok(n) => {
                return neotron_api::Result::Ok(neotron_api::file::Handle::new(n as u8));
            }
            Err(_f) => {
                return neotron_api::Result::Err(neotron_api::Error::OutOfMemory);
            }
        }
    }
    if path.as_str().eq_ignore_ascii_case("GFX:") {
        let initial_state = GfxState::new();
        match allocate_handle(OpenHandle::Gfx(initial_state)) {
            Ok(n) => {
                return neotron_api::Result::Ok(neotron_api::file::Handle::new(n as u8));
            }
            Err(_f) => {
                return neotron_api::Result::Err(neotron_api::Error::OutOfMemory);
            }
        }
    }

    // OK, let's assume it's a file relative to the root of our one and only volume
    let f = match FILESYSTEM.open_file(path.as_str(), embedded_sdmmc::Mode::ReadOnly) {
        Ok(f) => f,
        Err(fs::Error::Io(embedded_sdmmc::Error::NotFound)) => {
            return neotron_api::Result::Err(neotron_api::Error::InvalidPath);
        }
        Err(_e) => {
            return neotron_api::Result::Err(neotron_api::Error::DeviceSpecific);
        }
    };

    // 1. Put the file into the open handles array and get the index (or return an error)
    match allocate_handle(OpenHandle::File(f)) {
        Ok(n) => neotron_api::Result::Ok(neotron_api::file::Handle::new(n as u8)),
        Err(_f) => neotron_api::Result::Err(neotron_api::Error::OutOfMemory),
    }
}

/// Close a previously opened file.
extern "C" fn api_close(fd: neotron_api::file::Handle) -> neotron_api::Result<()> {
    let mut open_handles = OPEN_HANDLES.lock();
    match open_handles.get_mut(fd.value() as usize) {
        Some(h) => {
            *h = OpenHandle::Closed;
            neotron_api::Result::Ok(())
        }
        None => neotron_api::Result::Err(neotron_api::Error::BadHandle),
    }
}

/// Write to an open file handle, blocking until everything is written.
///
/// Some files do not support writing and will produce an error.
extern "C" fn api_write(
    fd: neotron_api::file::Handle,
    buffer: neotron_api::FfiByteSlice,
) -> neotron_api::Result<()> {
    let mut open_handles = OPEN_HANDLES.lock();
    let Some(h) = open_handles.get_mut(fd.value() as usize) else {
        return neotron_api::Result::Err(neotron_api::Error::BadHandle);
    };
    match h {
        OpenHandle::StdErr | OpenHandle::Stdout => {
            // Treat stderr and stdout the same
            let mut guard = crate::VGA_CONSOLE.lock();
            if let Some(console) = guard.as_mut() {
                console.write_bstr(buffer.as_slice());
            }
            let mut guard = crate::SERIAL_CONSOLE.lock();
            if let Some(console) = guard.as_mut() {
                // Ignore serial errors on stdout
                let _ = console.write_bstr(buffer.as_slice());
            }
            neotron_api::Result::Ok(())
        }
        OpenHandle::File(f) => match f.write(buffer.as_slice()) {
            Ok(_) => neotron_api::Result::Ok(()),
            Err(_e) => neotron_api::Result::Err(neotron_api::Error::DeviceSpecific),
        },
        OpenHandle::Audio => {
            let api = API.get();
            let mut slice = buffer.as_slice();
            // loop until we've sent all of it
            while !slice.is_empty() {
                let result = unsafe { (api.audio_output_data)(FfiByteSlice::new(slice)) };
                let this_time = match result {
                    neotron_common_bios::FfiResult::Ok(n) => n,
                    neotron_common_bios::FfiResult::Err(_e) => {
                        return neotron_api::Result::Err(neotron_api::Error::DeviceSpecific);
                    }
                };
                slice = &slice[this_time..];
            }
            neotron_api::Result::Ok(())
        }
        OpenHandle::StdIn | OpenHandle::Closed => {
            neotron_api::Result::Err(neotron_api::Error::BadHandle)
        }
        OpenHandle::Gfx(_state) => {
            // TODO: Allow users to write to the screen
            neotron_api::Result::Err(neotron_api::Error::Unimplemented)
        }
    }
}

/// Read from an open file, returning how much was actually read.
///
/// If you hit the end of the file, you might get less data than you asked for.
extern "C" fn api_read(
    fd: neotron_api::file::Handle,
    mut buffer: neotron_api::FfiBuffer,
) -> neotron_api::Result<usize> {
    let mut open_handles = OPEN_HANDLES.lock();
    let Some(h) = open_handles.get_mut(fd.value() as usize) else {
        return neotron_api::Result::Err(neotron_api::Error::BadHandle);
    };
    match h {
        OpenHandle::StdIn => {
            if let Some(buffer) = buffer.as_mut_slice() {
                let count = { crate::STD_INPUT.lock().get_data(buffer) };
                Ok(count).into()
            } else {
                neotron_api::Result::Err(neotron_api::Error::DeviceSpecific)
            }
        }
        OpenHandle::File(f) => {
            let Some(buffer) = buffer.as_mut_slice() else {
                return neotron_api::Result::Err(neotron_api::Error::InvalidArg);
            };
            match f.read(buffer) {
                Ok(n) => neotron_api::Result::Ok(n),
                Err(_e) => neotron_api::Result::Err(neotron_api::Error::DeviceSpecific),
            }
        }
        OpenHandle::Audio => {
            let api = API.get();
            let result = unsafe { (api.audio_input_data)(buffer) };
            match result {
                neotron_common_bios::FfiResult::Ok(n) => neotron_api::Result::Ok(n),
                neotron_common_bios::FfiResult::Err(_e) => {
                    neotron_api::Result::Err(neotron_api::Error::DeviceSpecific)
                }
            }
        }
        OpenHandle::Stdout | OpenHandle::StdErr | OpenHandle::Closed => {
            neotron_api::Result::Err(neotron_api::Error::BadHandle)
        }
        OpenHandle::Gfx(_state) => {
            // TODO: allow users to read the contents of the screen
            neotron_api::Result::Err(neotron_api::Error::Unimplemented)
        }
    }
}

/// Move the file offset (for the given file handle) to the given position.
///
/// Some files do not support seeking and will produce an error.
extern "C" fn api_seek_set(
    _fd: neotron_api::file::Handle,
    _position: u64,
) -> neotron_api::Result<()> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
}

/// Move the file offset (for the given file handle) relative to the current position
///
/// Some files do not support seeking and will produce an error.
extern "C" fn api_seek_cur(
    _fd: neotron_api::file::Handle,
    _offset: i64,
) -> neotron_api::Result<u64> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
}

/// Move the file offset (for the given file handle) to the end of the file
///
/// Some files do not support seeking and will produce an error.
extern "C" fn api_seek_end(_fd: neotron_api::file::Handle) -> neotron_api::Result<u64> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
}

/// Rename a file
extern "C" fn api_rename(
    _old_path: neotron_api::FfiString,
    _new_path: neotron_api::FfiString,
) -> neotron_api::Result<()> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
}

/// Perform a special I/O control operation.
///
/// # Audio Devices
///
/// * `0` - get output sample rate/format (0xN000_0000_<sample_rate_u32>) where N indicates the sample format
///     * N = 0 => Eight bit mono, one byte per sample
///     * N = 1 => Eight bit stereo, two byte per samples
///     * N = 2 => Sixteen bit mono, two byte per samples
///     * N = 3 => Sixteen bit stereo, four byte per samples
/// * `1` - set output sample rate/format
///     * As above
/// * `2` - get output sample space available
///     * Gets a value in bytes
extern "C" fn api_ioctl(
    fd: neotron_api::file::Handle,
    command: u64,
    value: u64,
) -> neotron_api::Result<u64> {
    let mut open_handles = OPEN_HANDLES.lock();
    let Some(h) = open_handles.get_mut(fd.value() as usize) else {
        return neotron_api::Result::Err(neotron_api::Error::BadHandle);
    };
    match h {
        OpenHandle::Audio => ioctl_audio(h, command, value),
        OpenHandle::Gfx(state) => ioctl_gfx(state, command, value),
        _ => neotron_api::Result::Err(neotron_api::Error::InvalidArg),
    }
}

/// Open a directory, given a path as a UTF-8 string.
extern "C" fn api_opendir(
    _path: neotron_api::FfiString,
) -> neotron_api::Result<neotron_api::dir::Handle> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
}

/// Close a previously opened directory.
extern "C" fn api_closedir(_dir: neotron_api::dir::Handle) -> neotron_api::Result<()> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
}

/// Read from an open directory
extern "C" fn api_readdir(
    _dir: neotron_api::dir::Handle,
) -> neotron_api::Result<neotron_api::dir::Entry> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
}

/// Get information about a file
extern "C" fn api_stat(
    _path: neotron_api::FfiString,
) -> neotron_api::Result<neotron_api::file::Stat> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
}

/// Get information about an open file
extern "C" fn api_fstat(
    fd: neotron_api::file::Handle,
) -> neotron_api::Result<neotron_api::file::Stat> {
    let mut open_handles = OPEN_HANDLES.lock();
    match open_handles.get_mut(fd.value() as usize) {
        Some(OpenHandle::File(f)) => {
            let stat = neotron_api::file::Stat {
                file_size: f.length() as u64,
                ctime: neotron_api::file::Time {
                    year_since_1970: 0,
                    zero_indexed_month: 0,
                    zero_indexed_day: 0,
                    hours: 0,
                    minutes: 0,
                    seconds: 0,
                },
                mtime: neotron_api::file::Time {
                    year_since_1970: 0,
                    zero_indexed_month: 0,
                    zero_indexed_day: 0,
                    hours: 0,
                    minutes: 0,
                    seconds: 0,
                },
                attr: neotron_api::file::Attributes::empty(),
            };
            neotron_api::Result::Ok(stat)
        }
        _ => neotron_api::Result::Err(neotron_api::Error::InvalidArg),
    }
}

/// Delete a file.
///
/// If the file is currently open this will give an error.
extern "C" fn api_deletefile(_path: neotron_api::FfiString) -> neotron_api::Result<()> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
}

/// Delete a directory
///
/// If the directory has anything in it, this will give an error.
extern "C" fn api_deletedir(_path: neotron_api::FfiString) -> neotron_api::Result<()> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
}

/// Change the current directory
///
/// Relative file paths are taken to be relative to the current directory.
///
/// Unlike on MS-DOS, there is only one current directory for the whole
/// system, not one per drive.
extern "C" fn api_chdir(_path: neotron_api::FfiString) -> neotron_api::Result<()> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
}

/// Change the current directory to the open directory
///
/// Relative file paths are taken to be relative to the current directory.
///
/// Unlike on MS-DOS, there is only one current directory for the whole
/// system, not one per drive.
extern "C" fn api_dchdir(_dir: neotron_api::dir::Handle) -> neotron_api::Result<()> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
}

/// Obtain the current working directory.
extern "C" fn api_pwd(_path: neotron_api::FfiBuffer) -> neotron_api::Result<usize> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
}

/// Allocate some memory
extern "C" fn api_malloc(
    _size: usize,
    _alignment: usize,
) -> neotron_api::Result<*mut core::ffi::c_void> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
}

/// Free some previously allocated memory
extern "C" fn api_free(_ptr: *mut core::ffi::c_void, _size: usize, _alignment: usize) {}

/// Handle audio-specific ioctls
fn ioctl_audio(_h: &mut OpenHandle, command: u64, value: u64) -> neotron_api::Result<u64> {
    let api = API.get();
    match command {
        0 => {
            // Getting sample rate
            let neotron_common_bios::FfiResult::Ok(config) = (api.audio_output_get_config)() else {
                return neotron_api::Result::Err(neotron_api::Error::DeviceSpecific);
            };
            let mut result: u64 = config.sample_rate_hz as u64;
            let nibble = match config.sample_format.make_safe() {
                Ok(neotron_common_bios::audio::SampleFormat::EightBitMono) => 0,
                Ok(neotron_common_bios::audio::SampleFormat::EightBitStereo) => 1,
                Ok(neotron_common_bios::audio::SampleFormat::SixteenBitMono) => 2,
                Ok(neotron_common_bios::audio::SampleFormat::SixteenBitStereo) => 3,
                _ => {
                    return neotron_api::Result::Err(neotron_api::Error::DeviceSpecific);
                }
            };
            result |= nibble << 60;
            neotron_api::Result::Ok(result)
        }
        1 => {
            // Setting sample rate
            let sample_rate = value as u32;
            let format = match value >> 60 {
                0 => neotron_common_bios::audio::SampleFormat::EightBitMono,
                1 => neotron_common_bios::audio::SampleFormat::EightBitStereo,
                2 => neotron_common_bios::audio::SampleFormat::SixteenBitMono,
                3 => neotron_common_bios::audio::SampleFormat::SixteenBitStereo,
                _ => {
                    return neotron_api::Result::Err(neotron_api::Error::InvalidArg);
                }
            };
            let config = neotron_common_bios::audio::Config {
                sample_format: format.make_ffi_safe(),
                sample_rate_hz: sample_rate,
            };
            match (api.audio_output_set_config)(config) {
                neotron_common_bios::FfiResult::Ok(_) => {
                    osprintln!("audio {}, {:?}", sample_rate, format);
                    neotron_api::Result::Ok(0)
                }
                neotron_common_bios::FfiResult::Err(_) => {
                    neotron_api::Result::Err(neotron_api::Error::DeviceSpecific)
                }
            }
        }
        2 => {
            // Setting sample space
            match (api.audio_output_get_space)() {
                neotron_common_bios::FfiResult::Ok(n) => neotron_api::Result::Ok(n as u64),
                neotron_common_bios::FfiResult::Err(_) => {
                    neotron_api::Result::Err(neotron_api::Error::DeviceSpecific)
                }
            }
        }
        _ => neotron_api::Result::Err(neotron_api::Error::InvalidArg),
    }
}

/// Clear the screen
///
/// The command the colour, which is taken modulo the number of on-screen colours.
pub const GFX_COMMAND_CLEAR_SCREEN: u64 = 0;

/// Plot a chunky pixel
///
/// The command contains [ x | y | mode | colour ].
///
/// * `x` is 16 bits and marks the horizontal position (0 is left)
/// * `y` is 16 bits and marks the vertical position (0 is top)
/// * `mode` is 8 bits and is currently ignored
/// * `colour` is 24 bits, and is taken modulo the number of on-screen colours
pub const GFX_COMMAND_CHUNKY_PLOT: u64 = 1;

/// Change graphics mode
///
/// The command contains the video mode in the upper 32 bits and a pointer to a
/// framebuffer in the lower 32 bits.
///
/// The framebuffer pointer must point to a 32-bit aligned region of memory
/// that is large enough for the selected mode. If you pass `null`, then the OS
/// will attempt to allocate a framebuffer for you.
pub const GFX_COMMAND_CHANGE_MODE: u64 = 2;

/// Move the cursor
///
/// The command contains [ x | y | <padding> ].
pub const GFX_COMMAND_MOVE_CURSOR: u64 = 3;

/// Draw a line
///
/// The command contains [ x | y | mode | colour ].
///
/// * `x` is 16 bits and marks the final horizontal position (0 is left)
/// * `y` is 16 bits and marks the final vertical position (0 is top)
/// * `mode` is 8 bits and is currently ignored
/// * `colour` is 24 bits, and is taken modulo the number of on-screen colours
///
/// The start position is the cursor position. The cursor is updated to the final position.
pub const GFX_COMMAND_DRAW_LINE: u64 = 4;

/// Set a palette entry
///
/// The command contains [ <padding> | II | RR | GG | BB ]
///
/// II, RR, GG and BB are 8-bit values where II is the index into the 256 long
/// palette, and RR, GG and BB are the 24-bit RGB colour for that index.
///
/// Use [`set_palette_value`] to construct a value.
pub const GFX_COMMAND_SET_PALETTE: u64 = 5;

/// Handle framebuffer-specific ioctls
fn ioctl_gfx(state: &mut GfxState, command: u64, value: u64) -> neotron_api::Result<u64> {
    let mut lock = crate::VGA_CONSOLE.lock();
    let Some(console) = lock.as_mut() else {
        // there is no graphics console
        return neotron_api::Result::Err(neotron_api::Error::NotFound);
    };
    match command {
        GFX_COMMAND_CLEAR_SCREEN => {
            let colour = (value & 0xFFFFFF) as u32;
            if let Err(e) = console.gfx_clear(colour) {
                Err(e).into()
            } else {
                Ok(0).into()
            }
        }
        GFX_COMMAND_CHUNKY_PLOT => {
            // the position on screen
            let x = (value >> 48) as u16;
            let y = (value >> 32) as u16;
            // whether to "set", "xor", "or", or "and" the pixel.
            // currently only "set" is supported (0)
            let _mode = (value >> 24) as u8;
            // the colour to use
            let colour = (value & 0xFFFFFF) as u32;
            if let Err(e) = console.gfx_plot(x, y, colour) {
                return neotron_api::Result::Err(e);
            }
            state.cursor_x = x;
            state.cursor_y = y;
            neotron_api::Result::Ok(0)
        }
        GFX_COMMAND_CHANGE_MODE => {
            let video_mode = (value >> 32) as u8;
            let Some(video_mode) = neotron_common_bios::video::Mode::try_from_u8(video_mode) else {
                return neotron_api::Result::Err(neotron_api::Error::InvalidArg);
            };
            let ptr = value as u32 as usize as *mut u32;
            // change the video mode
            if unsafe { console.change_mode(video_mode, ptr) }.is_err() {
                return neotron_api::Result::Err(neotron_api::Error::DeviceSpecific);
            };
            // reset our cursor on screen mode change
            state.cursor_x = 0;
            state.cursor_y = 0;
            neotron_api::Result::Ok(0)
        }
        GFX_COMMAND_MOVE_CURSOR => {
            let video_mode = console.get_mode();
            let new_x = (value >> 48) as u16;
            let new_y = (value >> 32) as u16;
            if new_x >= video_mode.horizontal_pixels() {
                return neotron_api::Result::Err(neotron_api::Error::InvalidArg);
            }
            if new_y >= video_mode.vertical_lines() {
                return neotron_api::Result::Err(neotron_api::Error::InvalidArg);
            }
            state.cursor_x = new_x;
            state.cursor_y = new_y;
            neotron_api::Result::Ok(0)
        }
        GFX_COMMAND_DRAW_LINE => {
            let new_x = (value >> 48) as u16;
            let new_y = (value >> 32) as u16;
            let colour = (value & 0xFFFFFF) as u32;
            if let Err(e) =
                console.gfx_draw_line(state.cursor_x, state.cursor_y, new_x, new_y, colour)
            {
                return neotron_api::Result::Err(e);
            };
            state.cursor_x = new_x;
            state.cursor_y = new_y;
            neotron_api::Result::Ok(0)
        }
        GFX_COMMAND_SET_PALETTE => {
            // the console doesn't care about the palette so I guess we'll still
            // do that here
            let index = (value >> 24) as u8;
            let rgb_packed = (value & 0xFFFFFF) as u32;
            let api = crate::API.get();
            let rgb_colour = neotron_common_bios::video::RGBColour::from_packed(rgb_packed);
            (api.video_set_palette)(index, rgb_colour);
            neotron_api::Result::Ok(0)
        }
        _ => neotron_api::Result::Err(neotron_api::Error::InvalidArg),
    }
}

// ===========================================================================
// End of file
// ===========================================================================
