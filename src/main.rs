//! Neotron OS
//!
//! This is the common Neotron OS. Compile and link to run at a suitable location in Flash or RAM. The first four bytes must be a pointer
//! to the entry function. The OS has no static RAM (i.e. global variables) - all RAM allocation is provided by the BIOS.

#![no_std]
#![no_main]

extern crate panic_halt;

use neotron_common_bios::{Api, ApiByteSlice, Option, Result};

#[link_section = ".entry_point"]
#[no_mangle]
#[used]
/// The pointer the BIOS calls to start running this application.
pub static ENTRY_POINT: extern "C" fn(*const Api) -> i32 = nos_start;

pub extern "C" fn nos_start(bios: *const Api) -> i32 {
    let bios = unsafe { &*bios }; // Convert pointer to reference
    match (bios.serial_write)(0, ApiByteSlice::new(b"Hello, world!\r\n"), Option::None) {
        Result::Ok(_) => loop {},
        Result::Err(_e) => {
            let _ = (bios.serial_write)(0, ApiByteSlice::new(b"Error!\r\n"), Option::None);
            loop {}
        }
    }
}

// End of file
