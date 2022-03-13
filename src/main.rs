//! # The Neotron Operating System
//!
//! This OS is intended to be loaded by a Neotron BIOS.
//!
//! Copyright (c) The Neotron Developers, 2020
//!
//! Licence: GPL v3 or higher (see ../LICENCE.md)

#![no_std]
#![no_main]

// Imports
use core::fmt::Write;
use neotron_common_bios as bios;
use serde::{Deserialize, Serialize};

// ===========================================================================
// Global Variables
// ===========================================================================

/// This tells the BIOS how to start the OS. This must be the first four bytes
/// of our portion of Flash.
#[link_section = ".entry_point"]
#[used]
pub static ENTRY_POINT_ADDR: extern "C" fn(&'static bios::Api) -> ! = main;

/// The OS version string
const OS_VERSION: &str = concat!("Neotron OS, version ", env!("CARGO_PKG_VERSION"), "-2");

/// We store the API object supplied by the BIOS here
static mut API: Option<&'static bios::Api> = None;

/// We store our VGA console here.
static mut VGA_CONSOLE: Option<VgaConsole> = None;

/// We store our VGA console here.
static mut SERIAL_CONSOLE: Option<SerialConsole> = None;

// ===========================================================================
// Macros
// ===========================================================================

/// Prints to the screen
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        if let Some(ref mut console) = unsafe { &mut VGA_CONSOLE } {
            write!(console, $($arg)*).unwrap();
        }
        if let Some(ref mut console) = unsafe { &mut SERIAL_CONSOLE } {
            write!(console, $($arg)*).unwrap();
        }
    };
}

/// Prints to the screen and puts a new-line on the end
#[macro_export]
macro_rules! println {
    () => (print!("\n"));
    ($($arg:tt)*) => {
        print!($($arg)*);
        print!("\n");
    };
}

// ===========================================================================
// Local types
// ===========================================================================

#[derive(Debug)]
struct VgaConsole {
    addr: *mut u8,
    width: u8,
    height: u8,
    row: u8,
    col: u8,
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

/// Represents the serial port we can use as a text input/output device.
struct SerialConsole(u8);

impl core::fmt::Write for SerialConsole {
    fn write_str(&mut self, data: &str) -> core::fmt::Result {
        if let Some(api) = unsafe { API } {
            (api.serial_write)(
                // Which port
                self.0,
                // Data
                bios::ApiByteSlice::new(data.as_bytes()),
                // No timeout
                bios::Option::None,
            )
            .unwrap();
        }
        Ok(())
    }
}

/// Represents our configuration information that we ask the BIOS to serialise
#[derive(Debug, Serialize, Deserialize)]
struct Config {
    vga_console_on: bool,
    serial_console_on: bool,
    serial_baud: u32,
}

impl Config {
    fn load() -> Result<Config, &'static str> {
        if let Some(api) = unsafe { API } {
            let mut buffer = [0u8; 64];
            match (api.configuration_get)(bios::ApiBuffer::new(&mut buffer)) {
                bios::Result::Ok(n) => {
                    postcard::from_bytes(&buffer[0..n]).map_err(|_e| "Failed to parse config")
                }
                bios::Result::Err(_e) => Err("Failed to load config"),
            }
        } else {
            Err("No API available?!")
        }
    }

    fn save(&self) -> Result<(), &'static str> {
        if let Some(api) = unsafe { API } {
            let mut buffer = [0u8; 64];
            let slice =
                postcard::to_slice(self, &mut buffer).map_err(|_e| "Failed to parse config")?;
            (api.configuration_set)(bios::ApiByteSlice::new(slice));
            Ok(())
        } else {
            Err("No API available?!")
        }
    }

    /// Should this system use the VGA console?
    fn has_vga_console(&self) -> bool {
        self.vga_console_on
    }

    /// Should this system use the UART console?
    fn has_serial_console(&self) -> Option<(u8, bios::serial::Config)> {
        if self.serial_console_on {
            Some((
                0,
                bios::serial::Config {
                    data_rate_bps: self.serial_baud,
                    data_bits: bios::serial::DataBits::Eight,
                    stop_bits: bios::serial::StopBits::One,
                    parity: bios::serial::Parity::None,
                    handshaking: bios::serial::Handshaking::None,
                },
            ))
        } else {
            None
        }
    }
}

impl core::default::Default for Config {
    fn default() -> Config {
        Config {
            vga_console_on: true,
            serial_console_on: false,
            serial_baud: 115200,
        }
    }
}

// ===========================================================================
// Private functions
// ===========================================================================

/// Initialise our global variables - the BIOS will not have done this for us
/// (as it doesn't know where they are).
unsafe fn start_up_init() {
    extern "C" {

        // These symbols come from `link.x`
        static mut __sbss: u32;
        static mut __ebss: u32;

        static mut __sdata: u32;
        static mut __edata: u32;
        static __sidata: u32;
    }

    r0::zero_bss(&mut __sbss, &mut __ebss);
    r0::init_data(&mut __sdata, &mut __edata, &__sidata);
}

// ===========================================================================
// Public functions / impl for public types
// ===========================================================================

/// This is the function the BIOS calls. This is because we store the address
/// of this function in the ENTRY_POINT_ADDR variable.
extern "C" fn main(api: &'static bios::Api) -> ! {
    unsafe {
        start_up_init();
        API = Some(api);
    }

    let config = Config::load().unwrap_or_else(|_| Config::default());

    if config.has_vga_console() {
        let mut addr: *mut u8 = core::ptr::null_mut();
        let mut width = 0;
        let mut height = 0;
        (api.video_memory_info_get)(&mut addr, &mut width, &mut height);
        if !addr.is_null() {
            let mut vga = VgaConsole {
                addr,
                width,
                height,
                row: 0,
                col: 0,
            };
            vga.find_start_row();
            unsafe {
                VGA_CONSOLE = Some(vga);
            }
            println!("Configured VGA console");
        }
    }

    if let Some((idx, serial_config)) = config.has_serial_console() {
        let _ignored = (api.serial_configure)(idx, serial_config);
        unsafe { SERIAL_CONSOLE = Some(SerialConsole(idx)) };
        println!("Configured Serial console on Serial {}", idx);
    }

    // Now we can call println!
    println!("Welcome to {}!", OS_VERSION);
    panic!("Testing a panic...");
}

/// Called when we have a panic.
#[inline(never)]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("PANIC!\n{:#?}", info);
    use core::sync::atomic::{self, Ordering};
    loop {
        atomic::compiler_fence(Ordering::SeqCst);
    }
}

// ===========================================================================
// End of file
// ===========================================================================
