//! Program Loading and Execution

use crate::{print, println};

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
    /// Start Address didn't look right
    BadAddress(u32),
}

impl From<embedded_sdmmc::Error<neotron_common_bios::Error>> for Error {
    fn from(value: embedded_sdmmc::Error<neotron_common_bios::Error>) -> Self {
        Error::Filesystem(value)
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
        println!("Loading /{} from Block Device 0", file_name);
        let bios_block = crate::fs::BiosBlock();
        let time = crate::fs::BiosTime();
        let mut mgr = embedded_sdmmc::VolumeManager::new(bios_block, time);
        // Open the first partition
        let mut volume = mgr.get_volume(embedded_sdmmc::VolumeIdx(0))?;
        let root_dir = mgr.open_root_dir(&volume)?;
        let mut file = mgr.open_file_in_dir(
            &mut volume,
            &root_dir,
            file_name,
            embedded_sdmmc::Mode::ReadOnly,
        )?;
        // Application space starts 4K into Cortex-M SRAM
        let application_ram = self.as_slice_u8();
        if file.length() as usize > application_ram.len() {
            return Err(Error::ProgramTooLarge);
        };
        let application_ram = &mut application_ram[0..file.length() as usize];
        mgr.read(&volume, &mut file, application_ram)?;
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
        // Read start-ptr as a 32-bit value
        let application_ram = self.as_slice_u32();
        let start_addr = application_ram[0];
        // But now we want RAM as u8 values, as start_ptr will be an odd number
        // because it's a Thumb2 address and that's a u16 aligned value, plus 1
        // to indicate Thumb2 mode.
        let application_ram = self.as_slice_u8();
        print!("Start address 0x{:08x}:", start_addr);
        // Does this start pointer look OK?
        if (start_addr & 1) != 1 {
            println!("not thumb2 func");
            return Err(Error::BadAddress(start_addr));
        }
        if !application_ram
            .as_ptr_range()
            .contains(&(start_addr as *const u8))
        {
            println!("out of bounds");
            return Err(Error::BadAddress(start_addr));
        }
        println!("OK!");
        drop(application_ram);
        let result = unsafe {
            let code: extern "C" fn(*const neotron_api::Api) -> i32 =
                ::core::mem::transmute(start_addr as *const ());
            code(&CALLBACK_TABLE)
        };
        Ok(result)
    }
}

/// Application API to print things to the console.
#[allow(unused)]
extern "C" fn print_fn(data: *const u8, len: usize) {
    let slice = unsafe { core::slice::from_raw_parts(data, len) };
    if let Ok(s) = core::str::from_utf8(slice) {
        print!("{}", s);
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
        if let Some(ref mut console) = unsafe { &mut crate::VGA_CONSOLE } {
            console.write_bstr(buffer.as_slice());
        }
        if let Some(ref mut console) = unsafe { &mut crate::SERIAL_CONSOLE } {
            if let Err(_e) = console.write_bstr(buffer.as_slice()) {
                return neotron_api::Result::Err(neotron_api::Error::DeviceSpecific);
            }
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
    _fd: neotron_api::file::Handle,
    _buffer: neotron_api::FfiBuffer,
) -> neotron_api::Result<usize> {
    neotron_api::Result::Err(neotron_api::Error::Unimplemented)
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
