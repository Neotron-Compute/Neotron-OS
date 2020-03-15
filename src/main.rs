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

/// The OS version string
static OS_VERSION: &str = concat!("Neotron OS, version ", env!("CARGO_PKG_VERSION"), "\0");

static mut API: Option<&'static common::Api> = None;

#[derive(Debug)]
struct VgaConsole {
    addr: *mut u8,
    width: u8,
    height: u8,
    row: u8,
    col: u8,
}

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

impl VgaConsole {
    const DEFAULT_ATTR: u8 = (2 << 3) | (1 << 0);

    fn move_char_right(&mut self) {
        self.col += 1;
        if self.col == self.width {
            self.col = 0;
            self.move_char_down();
        }
    }

    fn move_char_down(&mut self) {
        self.row += 1;
        if self.row == self.height {
            self.scroll_page();
            self.row = self.height - 1;
        }
    }

    fn read(&self) -> (u8, u8) {
        let offset =
            ((isize::from(self.row) * isize::from(self.width)) + isize::from(self.col)) * 2;
        let glyph = unsafe { core::ptr::read_volatile(self.addr.offset(offset)) };
        let attr = unsafe { core::ptr::read_volatile(self.addr.offset(offset + 1)) };
        (glyph, attr)
    }

    fn find_start_row(&mut self) {
        for row in 0..self.height {
            self.row = row;
            let g = self.read().0;
            if (g == b'\0') || (g == b' ') {
                // Found a line with nothing on it - start here!
                break;
            }
        }
    }

    fn write(&mut self, glyph: u8, attr: Option<u8>) {
        let offset =
            ((isize::from(self.row) * isize::from(self.width)) + isize::from(self.col)) * 2;
        unsafe { core::ptr::write_volatile(self.addr.offset(offset), glyph) };
        if let Some(a) = attr {
            unsafe { core::ptr::write_volatile(self.addr.offset(offset + 1), a) };
        }
    }

    fn scroll_page(&mut self) {
        unsafe {
            core::ptr::copy(
                self.addr.offset(isize::from(self.width * 2)),
                self.addr,
                usize::from(self.width) * usize::from(self.height - 1) * 2,
            );
            // Blank bottom line
            for col in 0..self.width {
                self.col = col;
                self.write(b' ', Some(Self::DEFAULT_ATTR));
            }
            self.col = 0;
        }
    }
}

impl core::fmt::Write for VgaConsole {
    fn write_str(&mut self, data: &str) -> core::fmt::Result {
        for ch in data.chars() {
            match ch {
                '\r' => {
                    self.col = 0;
                }
                '\n' => {
                    self.col = 0;
                    self.move_char_down();
                }
                b if b <= '\u{00FF}' => {
                    self.write(b as u8, None);
                    self.move_char_right();
                }
                _ => {
                    self.write(b'?', None);
                    self.move_char_right();
                }
            }
        }
        Ok(())
    }
}

#[no_mangle]
extern "C" fn entry_point(api: &'static common::Api) -> ! {
    unsafe {
        API = Some(api);
    }
    let mut addr: *mut u8 = core::ptr::null_mut();
    let mut width = 0;
    let mut height = 0;
    (api.video_memory_info_get)(&mut addr, &mut width, &mut height);
    let mut vga = VgaConsole {
        addr,
        width,
        height,
        row: 0,
        col: 0,
    };
    vga.find_start_row();
    writeln!(vga, "{}", OS_VERSION).unwrap();
    writeln!(vga, "BIOS Version: {}", (api.bios_version_get)()).unwrap();
    writeln!(vga, "BIOS API Version: {}", (api.api_version_get)()).unwrap();
    loop {
        for _ in 0..1_000_000 {
            let _ = (api.api_version_get)();
        }
        writeln!(vga, "tick...").unwrap();
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
