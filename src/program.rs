//! Program Loading and Execution

use crate::{print, println};

#[allow(unused)]
static CALLBACK_TABLE: Api = Api { print: print_fn };

/// Ways in which loading a program can fail.
#[derive(Debug)]
pub enum Error {
    /// The file was too large for RAM.
    ProgramTooLarge,
    /// A filesystem error occurred
    Filesystem(embedded_sdmmc::Error<neotron_common_bios::Error>),
}

impl From<embedded_sdmmc::Error<neotron_common_bios::Error>> for Error {
    fn from(value: embedded_sdmmc::Error<neotron_common_bios::Error>) -> Self {
        Error::Filesystem(value)
    }
}

#[allow(unused)]
#[repr(C)]
/// The API we give to applications.
pub struct Api {
    pub print: extern "C" fn(data: *const u8, len: usize),
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
        let application_ram = self.as_slice_u32();
        let start_ptr = application_ram[0] as *const ();
        let result = unsafe {
            let code: extern "C" fn(*const Api) -> i32 = ::core::mem::transmute(start_ptr);
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

// ===========================================================================
// End of file
// ===========================================================================
