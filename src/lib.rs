//! # The Neotron Operating System
//!
//! This OS is intended to be loaded by a Neotron BIOS.
//!
//! Copyright (c) The Neotron Developers, 2022
//!
//! Licence: GPL v3 or higher (see ../LICENCE.md)

#![no_std]

// Imports
use core::fmt::Write;
use neotron_common_bios as bios;
use serde::{Deserialize, Serialize};

// ===========================================================================
// Global Variables
// ===========================================================================

/// The OS version string
const OS_VERSION: &str = concat!("Neotron OS, version ", env!("OS_VERSION"));

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
    width: isize,
    height: isize,
    row: isize,
    col: isize,
}

impl VgaConsole {
    /// White on Black
    const DEFAULT_ATTR: u8 = (15 << 3) | 0;

    fn move_char_right(&mut self) {
        self.col += 1;
    }

    fn move_char_down(&mut self) {
        self.row += 1;
    }

    fn scroll_as_required(&mut self) {
        if self.col == self.width {
            self.col = 0;
            self.row += 1;
        }
        if self.row == self.height {
            self.row = self.height - 1;
            self.scroll_page();
        }
    }

    fn clear(&mut self) {
        for row in 0..self.height {
            for col in 0..self.width {
                self.row = row;
                self.col = col;
                self.write(b' ', Some(Self::DEFAULT_ATTR));
            }
        }
        self.row = 0;
        self.col = 0;
    }

    fn write(&mut self, glyph: u8, attr: Option<u8>) {
        let offset = ((self.row * self.width) + self.col) * 2;
        unsafe { core::ptr::write_volatile(self.addr.offset(offset), glyph) };
        if let Some(a) = attr {
            unsafe { core::ptr::write_volatile(self.addr.offset(offset + 1), a) };
        }
    }

    fn scroll_page(&mut self) {
        let row_len_bytes = self.width * 2;
        unsafe {
            core::ptr::copy(
                self.addr.offset(row_len_bytes),
                self.addr,
                (row_len_bytes * (self.height - 1)) as usize,
            );
            // Blank bottom line
            for col in 0..self.width {
                self.col = col;
                self.write(b' ', Some(Self::DEFAULT_ATTR));
            }
            self.col = 0;
        }
    }

    /// Convert a Unicode Scalar Value to a font glyph.
    ///
    /// Zero-width and modifier Unicode Scalar Values (e.g. `U+0301 COMBINING,
    /// ACCENT`) are not supported. Normalise your Unicode before calling
    /// this function.
    fn map_char_to_glyph(input: char) -> u8 {
        // This fixed table only works for the default font. When we support
        // changing font, we will need to plug-in a different table for each font.
        match input {
            '\u{0000}'..='\u{007F}' => input as u8,
            '\u{00A0}' => 255, // NBSP
            '\u{00A1}' => 173, // ¡
            '\u{00A2}' => 189, // ¢
            '\u{00A3}' => 156, // £
            '\u{00A4}' => 207, // ¤
            '\u{00A5}' => 190, // ¥
            '\u{00A6}' => 221, // ¦
            '\u{00A7}' => 245, // §
            '\u{00A8}' => 249, // ¨
            '\u{00A9}' => 184, // ©
            '\u{00AA}' => 166, // ª
            '\u{00AB}' => 174, // «
            '\u{00AC}' => 170, // ¬
            '\u{00AD}' => 240, // SHY
            '\u{00AE}' => 169, // ®
            '\u{00AF}' => 238, // ¯
            '\u{00B0}' => 248, // °
            '\u{00B1}' => 241, // ±
            '\u{00B2}' => 253, // ²
            '\u{00B3}' => 252, // ³
            '\u{00B4}' => 239, // ´
            '\u{00B5}' => 230, // µ
            '\u{00B6}' => 244, // ¶
            '\u{00B7}' => 250, // ·
            '\u{00B8}' => 247, // ¸
            '\u{00B9}' => 251, // ¹
            '\u{00BA}' => 167, // º
            '\u{00BB}' => 175, // »
            '\u{00BC}' => 172, // ¼
            '\u{00BD}' => 171, // ½
            '\u{00BE}' => 243, // ¾
            '\u{00BF}' => 168, // ¿
            '\u{00C0}' => 183, // À
            '\u{00C1}' => 181, // Á
            '\u{00C2}' => 182, // Â
            '\u{00C3}' => 199, // Ã
            '\u{00C4}' => 142, // Ä
            '\u{00C5}' => 143, // Å
            '\u{00C6}' => 146, // Æ
            '\u{00C7}' => 128, // Ç
            '\u{00C8}' => 212, // È
            '\u{00C9}' => 144, // É
            '\u{00CA}' => 210, // Ê
            '\u{00CB}' => 211, // Ë
            '\u{00CC}' => 222, // Ì
            '\u{00CD}' => 214, // Í
            '\u{00CE}' => 215, // Î
            '\u{00CF}' => 216, // Ï
            '\u{00D0}' => 209, // Ð
            '\u{00D1}' => 165, // Ñ
            '\u{00D2}' => 227, // Ò
            '\u{00D3}' => 224, // Ó
            '\u{00D4}' => 226, // Ô
            '\u{00D5}' => 229, // Õ
            '\u{00D6}' => 153, // Ö
            '\u{00D7}' => 158, // ×
            '\u{00D8}' => 157, // Ø
            '\u{00D9}' => 235, // Ù
            '\u{00DA}' => 233, // Ú
            '\u{00DB}' => 234, // Û
            '\u{00DC}' => 154, // Ü
            '\u{00DD}' => 237, // Ý
            '\u{00DE}' => 232, // Þ
            '\u{00DF}' => 225, // ß
            '\u{00E0}' => 133, // à
            '\u{00E1}' => 160, // á
            '\u{00E2}' => 131, // â
            '\u{00E3}' => 198, // ã
            '\u{00E4}' => 132, // ä
            '\u{00E5}' => 134, // å
            '\u{00E6}' => 145, // æ
            '\u{00E7}' => 135, // ç
            '\u{00E8}' => 138, // è
            '\u{00E9}' => 130, // é
            '\u{00EA}' => 136, // ê
            '\u{00EB}' => 137, // ë
            '\u{00EC}' => 141, // ì
            '\u{00ED}' => 161, // í
            '\u{00EE}' => 140, // î
            '\u{00EF}' => 139, // ï
            '\u{00F0}' => 208, // ð
            '\u{00F1}' => 164, // ñ
            '\u{00F2}' => 149, // ò
            '\u{00F3}' => 162, // ó
            '\u{00F4}' => 147, // ô
            '\u{00F5}' => 228, // õ
            '\u{00F6}' => 148, // ö
            '\u{00F7}' => 246, // ÷
            '\u{00F8}' => 155, // ø
            '\u{00F9}' => 151, // ù
            '\u{00FA}' => 163, // ú
            '\u{00FB}' => 150, // û
            '\u{00FC}' => 129, // ü
            '\u{00FD}' => 236, // ý
            '\u{00FE}' => 231, // þ
            '\u{00FF}' => 152, // ÿ
            '\u{0131}' => 213, // ı
            '\u{0192}' => 159, // ƒ
            '\u{2017}' => 242, // ‗
            '\u{2500}' => 196, // ─
            '\u{2502}' => 179, // │
            '\u{250C}' => 218, // ┌
            '\u{2510}' => 191, // ┐
            '\u{2514}' => 192, // └
            '\u{2518}' => 217, // ┘
            '\u{251C}' => 195, // ├
            '\u{2524}' => 180, // ┤
            '\u{252C}' => 194, // ┬
            '\u{2534}' => 193, // ┴
            '\u{253C}' => 197, // ┼
            '\u{2550}' => 205, // ═
            '\u{2551}' => 186, // ║
            '\u{2554}' => 201, // ╔
            '\u{2557}' => 187, // ╗
            '\u{255A}' => 200, // ╚
            '\u{255D}' => 188, // ╝
            '\u{2560}' => 204, // ╠
            '\u{2563}' => 185, // ╣
            '\u{2566}' => 203, // ╦
            '\u{2569}' => 202, // ╩
            '\u{256C}' => 206, // ╬
            '\u{2580}' => 223, // ▀
            '\u{2584}' => 220, // ▄
            '\u{2588}' => 219, // █
            '\u{2591}' => 176, // ░
            '\u{2592}' => 177, // ▒
            '\u{2593}' => 178, // ▓
            '\u{25A0}' => 254, // ■
            _ => b'?',
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
                _ => {
                    self.scroll_as_required();
                    self.write(Self::map_char_to_glyph(ch), None);
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
#[cfg(target_os = "none")]
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

#[cfg(not(target_os = "none"))]
unsafe fn start_up_init() {
    // Nothing to do
}

// ===========================================================================
// Public functions / impl for public types
// ===========================================================================

/// This is the function the BIOS calls. This is because we store the address
/// of this function in the ENTRY_POINT_ADDR variable.
#[no_mangle]
pub extern "C" fn main(api: &'static bios::Api) -> ! {
    unsafe {
        start_up_init();
        if (api.api_version_get)() != neotron_common_bios::API_VERSION {
            panic!("API mismatch!");
        }
        API = Some(api);
    }

    let config = Config::load().unwrap_or_else(|_| Config::default());

    if config.has_vga_console() {
        // Try and set 80x50 mode for that authentic Windows NT bootloader feel
        (api.video_set_mode)(bios::video::Mode::new(
            bios::video::Timing::T640x400,
            bios::video::Format::Text8x8,
        ));
        // Work with whatever we get
        let mode = (api.video_get_mode)();
        let (width, height) = (mode.text_width(), mode.text_height());

        if let (Some(width), Some(height)) = (width, height) {
            let mut vga = VgaConsole {
                addr: (api.video_get_framebuffer)(),
                width: width as isize,
                height: height as isize,
                row: 0,
                col: 0,
            };
            vga.clear();
            unsafe {
                VGA_CONSOLE = Some(vga);
            }
            println!("Configured VGA console {}x{}", width, height);
        }
    }

    if let Some((idx, serial_config)) = config.has_serial_console() {
        let _ignored = (api.serial_configure)(idx, serial_config);
        unsafe { SERIAL_CONSOLE = Some(SerialConsole(idx)) };
        println!("Configured Serial console on Serial {}", idx);
    }

    // Now we can call println!
    println!("Welcome to {}!", OS_VERSION);
    println!("Copyright © Jonathan 'theJPster' Pallant and the Neotron Developers, 2022");

    for region_idx in 0..=255 {
        match (api.memory_get_region)(region_idx) {
            bios::Result::Ok(region) => {
                println!("Region {}: {}", region_idx, region);
            }
            _ => {
                // Ran out of regions (we assume they are consecutive)
                break;
            }
        }
    }

    // Some text, to force the console to scroll.
    for i in 0..50 {
        for _x in 0..50 - i {
            print!(".");
        }
        println!("{}", i);
        (api.delay)(neotron_common_bios::Timeout::new_ms(250));
    }

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
