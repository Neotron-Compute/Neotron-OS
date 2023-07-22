//! Program Loading and Execution

use embedded_sdmmc::File;

use crate::{
    fs::{BiosBlock, BiosTime},
    osprint, osprintln,
};

#[allow(unused)]
static CALLBACK_TABLE: neotron_api::Api = neotron_api::Api {
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

/// Ways in which loading a program can fail.
#[derive(Debug)]
pub enum Error {
    /// The file was too large for RAM.
    ProgramTooLarge,
    /// A filesystem error occurred
    Filesystem(embedded_sdmmc::Error<neotron_common_bios::Error>),
    /// An ELF error occurred
    Elf(neotron_loader::Error<embedded_sdmmc::Error<neotron_common_bios::Error>>),
    /// Tried to run when nothing was loaded
    NothingLoaded,
}

impl From<embedded_sdmmc::Error<neotron_common_bios::Error>> for Error {
    fn from(value: embedded_sdmmc::Error<neotron_common_bios::Error>) -> Self {
        Error::Filesystem(value)
    }
}

impl From<neotron_loader::Error<embedded_sdmmc::Error<neotron_common_bios::Error>>> for Error {
    fn from(
        value: neotron_loader::Error<embedded_sdmmc::Error<neotron_common_bios::Error>>,
    ) -> Self {
        Error::Elf(value)
    }
}

/// Something the ELF loader can use to get bytes off the disk
struct FileSource {
    mgr: core::cell::RefCell<embedded_sdmmc::VolumeManager<BiosBlock, BiosTime>>,
    volume: embedded_sdmmc::Volume,
    file: core::cell::RefCell<File>,
    buffer: core::cell::RefCell<[u8; Self::BUFFER_LEN]>,
    offset_cached: core::cell::Cell<Option<u32>>,
}

impl FileSource {
    const BUFFER_LEN: usize = 128;

    fn new(
        mgr: embedded_sdmmc::VolumeManager<BiosBlock, BiosTime>,
        volume: embedded_sdmmc::Volume,
        file: File,
    ) -> FileSource {
        FileSource {
            mgr: core::cell::RefCell::new(mgr),
            file: core::cell::RefCell::new(file),
            volume,
            buffer: core::cell::RefCell::new([0u8; 128]),
            offset_cached: core::cell::Cell::new(None),
        }
    }

    fn uncached_read(
        &self,
        offset: u32,
        out_buffer: &mut [u8],
    ) -> Result<(), embedded_sdmmc::Error<neotron_common_bios::Error>> {
        osprintln!("Reading from {}", offset);
        self.file.borrow_mut().seek_from_start(offset).unwrap();
        self.mgr
            .borrow_mut()
            .read(&self.volume, &mut self.file.borrow_mut(), out_buffer)?;
        Ok(())
    }
}

impl neotron_loader::traits::Source for &FileSource {
    type Error = embedded_sdmmc::Error<neotron_common_bios::Error>;

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

            osprintln!("Reading from {}", offset);
            self.file.borrow_mut().seek_from_start(offset).unwrap();
            self.mgr.borrow_mut().read(
                &self.volume,
                &mut self.file.borrow_mut(),
                self.buffer.borrow_mut().as_mut_slice(),
            )?;
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
        let official_tpa_start: Option<*mut u32> = Some((&mut _tpa_start) as *mut u32);

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
        unsafe {
            core::slice::from_raw_parts_mut(
                self.memory_bottom,
                self.memory_top.offset_from(self.memory_bottom) as usize,
            )
        }
    }

    /// Borrow the TPA region as a slice of bytes
    pub fn as_slice_u8(&mut self) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                self.memory_bottom as *mut u8,
                (self.memory_top.offset_from(self.memory_bottom) as usize)
                    * core::mem::size_of::<u32>(),
            )
        }
    }

    /// Loads a program from disk into the Transient Program Area.
    ///
    /// The program must be in the Neotron Executable format.
    pub fn load_program(&mut self, file_name: &str) -> Result<(), Error> {
        osprintln!("Loading /{} from Block Device 0", file_name);
        let bios_block = crate::fs::BiosBlock();
        let time = crate::fs::BiosTime();
        let mut mgr = embedded_sdmmc::VolumeManager::new(bios_block, time);
        // Open the first partition
        let mut volume = mgr.get_volume(embedded_sdmmc::VolumeIdx(0))?;
        let root_dir = mgr.open_root_dir(&volume)?;
        let file = mgr.open_file_in_dir(
            &mut volume,
            &root_dir,
            file_name,
            embedded_sdmmc::Mode::ReadOnly,
        )?;

        let source = FileSource::new(mgr, volume, file);
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

    /// Copy a program from memory into the Transient Program Area.
    ///
    /// The program must be in the Neotron Executable format.
    pub fn copy_program(&mut self, program: &[u8]) -> Result<(), Error> {
        let application_ram = self.as_slice_u8();
        if program.len() > application_ram.len() {
            return Err(Error::ProgramTooLarge);
        }
        let application_ram = &mut application_ram[0..program.len()];
        application_ram.copy_from_slice(program);
        Ok(())
    }

    /// Execute a program.
    ///
    /// If the program returns, you get `Ok(<exit_code>)`. The program returning
    /// an exit code that is non-zero is not considered a failure from the point
    /// of view of this API. You wanted to run a program, and the program was
    /// run.
    pub fn execute(&mut self) -> Result<i32, Error> {
        if self.last_entry == 0 {
            return Err(Error::NothingLoaded);
        }

        let result = unsafe {
            let code: extern "C" fn(*const neotron_api::Api) -> i32 =
                ::core::mem::transmute(self.last_entry as *const ());
            code(&CALLBACK_TABLE)
        };

        self.last_entry = 0;
        Ok(result)
    }
}

/// Application API to print things to the console.
#[allow(unused)]
extern "C" fn print_fn(data: *const u8, len: usize) {
    let slice = unsafe { core::slice::from_raw_parts(data, len) };
    if let Ok(s) = core::str::from_utf8(slice) {
        osprint!("{}", s);
    } else {
        // Ignore App output - not UTF-8
    }
}

/// Open a file, given a path as UTF-8 string.
///
/// If the file does not exist, or is already open, it returns an error.
///
/// Path may be relative to current directory, or it may be an absolute
/// path.
extern "C" fn api_open(
    _path: neotron_api::FfiString,
    _flags: neotron_api::file::Flags,
) -> neotron_api::Result<neotron_api::file::Handle> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
}

/// Close a previously opened file.
extern "C" fn api_close(_fd: neotron_api::file::Handle) -> neotron_api::Result<()> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
}

/// Write to an open file handle, blocking until everything is written.
///
/// Some files do not support writing and will produce an error.
extern "C" fn api_write(
    fd: neotron_api::file::Handle,
    buffer: neotron_api::FfiByteSlice,
) -> neotron_api::Result<()> {
    if fd == neotron_api::file::Handle::new_stdout() {
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
    } else {
        neotron_api::Result::Err(neotron_api::Error::BadHandle)
    }
}

/// Read from an open file, returning how much was actually read.
///
/// If you hit the end of the file, you might get less data than you asked for.
extern "C" fn api_read(
    fd: neotron_api::file::Handle,
    mut buffer: neotron_api::FfiBuffer,
) -> neotron_api::Result<usize> {
    if fd == neotron_api::file::Handle::new_stdin() {
        if let Some(buffer) = buffer.as_mut_slice() {
            let count = { crate::STD_INPUT.lock().get_data(buffer) };
            Ok(count).into()
        } else {
            neotron_api::Result::Err(neotron_api::Error::DeviceSpecific)
        }
    } else {
        neotron_api::Result::Err(neotron_api::Error::BadHandle)
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
extern "C" fn api_ioctl(
    _fd: neotron_api::file::Handle,
    _command: u64,
    _value: u64,
) -> neotron_api::Result<u64> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
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
    _fd: neotron_api::file::Handle,
) -> neotron_api::Result<neotron_api::file::Stat> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
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

// ===========================================================================
// End of file
// ===========================================================================
