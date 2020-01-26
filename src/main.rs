//! # The Neotron Operating System
//!
//! This OS is intended to be loaded by a Neotron BIOS.
#![no_std]
#![no_main]

use core::fmt::Write;
use neotron_common_bios as common;

#[link_section = ".entry_point"]
#[no_mangle]
#[used]
pub static ENTRY_POINT: extern "C" fn(&'static common::Api) -> ! = entry_point;

static mut API: Option<&'static common::Api> = None;

struct SerialConsole;

impl core::fmt::Write for SerialConsole {
    fn write_str(&mut self, data: &str) -> core::fmt::Result {
        if let Some(api) = unsafe { API } {
            let _res = (api.serial_write)(
                0,
                common::ApiByteSlice::new(data.as_bytes()),
                common::Option::None,
            );
        }
        Ok(())
    }
}

#[no_mangle]
extern "C" fn entry_point(api: &'static common::Api) -> ! {
    unsafe {
        API = Some(api);
    }
    writeln!(SerialConsole, "Neotron OS.").unwrap();
    writeln!(SerialConsole, "BIOS Version: {}", (api.bios_version_get)()).unwrap();
    writeln!(
        SerialConsole,
        "BIOS API Version: {}",
        (api.api_version_get)()
    )
    .unwrap();
    loop {
        for _ in 0..80_000_000 {
            let _ = (api.api_version_get)();
        }
        writeln!(SerialConsole, "tick...").unwrap();
    }
}

#[inline(never)]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    use core::sync::atomic::{self, Ordering};
    loop {
        atomic::compiler_fence(Ordering::SeqCst);
    }
}
