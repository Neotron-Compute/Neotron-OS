//! # The Neotron Operating System
//!
//! This OS is intended to be loaded by a Neotron BIOS.
//!
//! Copyright (c) The Neotron Developers, 2022
//!
//! Licence: GPL v3 or higher (see ../LICENCE.md)

#![cfg_attr(not(test), no_std)]

// ===========================================================================
// Modules and Imports
// ===========================================================================

use core::sync::atomic::{AtomicBool, Ordering};
use neotron_common_bios as bios;

mod commands;
mod config;
mod fs;
mod program;
mod vgaconsole;

pub use config::Config as OsConfig;

// ===========================================================================
// Global Variables
// ===========================================================================

/// The OS version string
const OS_VERSION: &str = concat!("Neotron OS, v", env!("OS_VERSION"));

/// Used to convert between POSIX epoch (for `chrono`) and Neotron epoch (for BIOS APIs).
const SECONDS_BETWEEN_UNIX_AND_NEOTRON_EPOCH: i64 = 946684800;

/// We store the API object supplied by the BIOS here
static API: Api = Api::new();

/// We store our VGA console here.
static mut VGA_CONSOLE: Option<vgaconsole::VgaConsole> = None;

/// We store our VGA console here.
static mut SERIAL_CONSOLE: Option<SerialConsole> = None;

/// Note if we are panicking right now.
///
/// If so, don't panic if a serial write fails.
static IS_PANIC: AtomicBool = AtomicBool::new(false);

/// Our keyboard controller
static mut STD_INPUT: StdInput = StdInput::new();

struct StdInput {
    keyboard: pc_keyboard::EventDecoder<pc_keyboard::layouts::AnyLayout>,
    buffer: heapless::spsc::Queue<u8, 16>,
}

impl StdInput {
    const fn new() -> StdInput {
        StdInput {
            keyboard: pc_keyboard::EventDecoder::new(
                pc_keyboard::layouts::AnyLayout::Uk105Key(pc_keyboard::layouts::Uk105Key),
                pc_keyboard::HandleControl::MapLettersToUnicode,
            ),
            buffer: heapless::spsc::Queue::new(),
        }
    }

    fn get_buffered_data(&mut self, buffer: &mut [u8]) -> usize {
        // If there is some data, get it.
        let mut count = 0;
        for slot in buffer.iter_mut() {
            if let Some(n) = self.buffer.dequeue() {
                *slot = n;
                count += 1;
            }
        }
        count
    }

    /// Gets a raw event from the keyboard
    fn get_raw(&mut self) -> Option<pc_keyboard::DecodedKey> {
        let api = API.get();
        match (api.hid_get_event)() {
            bios::ApiResult::Ok(bios::FfiOption::Some(bios::hid::HidEvent::KeyPress(code))) => {
                let pckb_ev = pc_keyboard::KeyEvent {
                    code,
                    state: pc_keyboard::KeyState::Down,
                };
                self.keyboard.process_keyevent(pckb_ev)
            }
            bios::ApiResult::Ok(bios::FfiOption::Some(bios::hid::HidEvent::KeyRelease(code))) => {
                let pckb_ev = pc_keyboard::KeyEvent {
                    code,
                    state: pc_keyboard::KeyState::Up,
                };
                self.keyboard.process_keyevent(pckb_ev)
            }
            bios::ApiResult::Ok(bios::FfiOption::Some(bios::hid::HidEvent::MouseInput(
                _ignore,
            ))) => None,
            bios::ApiResult::Ok(bios::FfiOption::None) => {
                // Do nothing
                None
            }
            bios::ApiResult::Err(_e) => None,
        }
    }

    /// Gets some input bytes, as UTF-8.
    ///
    /// The data you get might be cut in the middle of a UTF-8 character.
    fn get_data(&mut self, buffer: &mut [u8]) -> usize {
        let count = self.get_buffered_data(buffer);
        if buffer.len() == 0 || count > 0 {
            return count;
        }

        // Nothing buffered - ask the keyboard for something
        let decoded_key = self.get_raw();

        match decoded_key {
            Some(pc_keyboard::DecodedKey::Unicode(mut ch)) => {
                if ch == '\n' {
                    ch = '\r';
                }
                let mut buffer = [0u8; 6];
                let s = ch.encode_utf8(&mut buffer);
                for b in s.as_bytes() {
                    // This will always fit
                    self.buffer.enqueue(*b).unwrap();
                }
            }
            Some(pc_keyboard::DecodedKey::RawKey(pc_keyboard::KeyCode::ArrowRight)) => {
                // Load the ANSI sequence for a right arrow
                for b in b"\x1b[0;77b" {
                    // This will always fit
                    self.buffer.enqueue(*b).unwrap();
                }
            }
            _ => {
                // Drop anything else
            }
        }

        // if let Some((uart_dev, _serial_conf)) = menu.context.config.get_serial_console() {
        //     while !self.buffer.is_full() {
        //         let mut buffer = [0u8];
        //         let wrapper = neotron_common_bios::FfiBuffer::new(&mut buffer);
        //         match (api.serial_read)(uart_dev, wrapper, neotron_common_bios::FfiOption::None) {
        //             neotron_common_bios::ApiResult::Ok(n) if n >= 0 => {
        //                 self.buffer.enqueue(buffer[0]).unwrap();
        //             }
        //             _ => {
        //                 break;
        //             }
        //         }
        //     }
        // }

        self.get_buffered_data(buffer)
    }
}

// ===========================================================================
// Macros
// ===========================================================================

/// Prints to the screen
#[macro_export]
macro_rules! osprint {
    ($($arg:tt)*) => {
        if let Some(ref mut console) = unsafe { &mut $crate::VGA_CONSOLE } {
            #[allow(unused)]
            use core::fmt::Write as _;
            write!(console, $($arg)*).unwrap();
        }
        if let Some(ref mut console) = unsafe { &mut $crate::SERIAL_CONSOLE } {
            #[allow(unused)]
            use core::fmt::Write as _;
            write!(console, $($arg)*).unwrap();
        }
    };
}

/// Prints to the screen and puts a new-line on the end
#[macro_export]
macro_rules! osprintln {
    () => ($crate::osprint!("\n"));
    ($($arg:tt)*) => {
        $crate::osprint!($($arg)*);
        $crate::osprint!("\n");
    };
}

// ===========================================================================
// Local types
// ===========================================================================

/// Represents the API supplied by the BIOS
struct Api {
    bios: core::sync::atomic::AtomicPtr<bios::Api>,
}

impl Api {
    /// Create a new object with a null pointer for the BIOS API.
    const fn new() -> Api {
        Api {
            bios: core::sync::atomic::AtomicPtr::new(core::ptr::null_mut()),
        }
    }

    /// Change the stored BIOS API pointer.
    ///
    /// The pointed-at object must have static lifetime.
    unsafe fn store(&self, api: *const bios::Api) {
        self.bios
            .store(api as *mut bios::Api, core::sync::atomic::Ordering::SeqCst)
    }

    /// Get the BIOS API as a reference.
    ///
    /// Will panic if the stored pointer is null.
    fn get(&self) -> &'static bios::Api {
        let ptr = self.bios.load(core::sync::atomic::Ordering::SeqCst) as *const bios::Api;
        let api_ref = unsafe { ptr.as_ref() }.expect("BIOS API should be non-null");
        api_ref
    }

    /// Get the current time
    fn get_time(&self) -> chrono::NaiveDateTime {
        let api = self.get();
        let bios_time = (api.time_clock_get)();
        let secs = i64::from(bios_time.secs) + SECONDS_BETWEEN_UNIX_AND_NEOTRON_EPOCH;
        let nsecs = bios_time.nsecs;
        chrono::NaiveDateTime::from_timestamp_opt(secs, nsecs).unwrap()
    }

    /// Set the current time
    fn set_time(&self, timestamp: chrono::NaiveDateTime) {
        let api = self.get();
        let nanos = timestamp.timestamp_nanos();
        let bios_time = bios::Time {
            secs: ((nanos / 1_000_000_000) - SECONDS_BETWEEN_UNIX_AND_NEOTRON_EPOCH) as u32,
            nsecs: (nanos % 1_000_000_000) as u32,
        };
        (api.time_clock_set)(bios_time);
    }
}

/// Represents the serial port we can use as a text input/output device.
struct SerialConsole(u8);

impl SerialConsole {
    fn write_bstr(&mut self, data: &[u8]) -> core::fmt::Result {
        let api = API.get();
        let is_panic = IS_PANIC.load(Ordering::Relaxed);
        let res = (api.serial_write)(
            // Which port
            self.0,
            // Data
            bios::FfiByteSlice::new(data),
            // No timeout
            bios::FfiOption::None,
        );
        if !is_panic {
            res.unwrap();
        }
        Ok(())
    }
}

impl core::fmt::Write for SerialConsole {
    fn write_str(&mut self, data: &str) -> core::fmt::Result {
        let api = API.get();
        let is_panic = IS_PANIC.load(Ordering::Relaxed);
        let res = (api.serial_write)(
            // Which port
            self.0,
            // Data
            bios::FfiByteSlice::new(data.as_bytes()),
            // No timeout
            bios::FfiOption::None,
        );
        if !is_panic {
            res.unwrap();
        }
        Ok(())
    }
}

pub struct Ctx {
    config: config::Config,
    tpa: program::TransientProgramArea,
}

impl core::fmt::Write for Ctx {
    fn write_str(&mut self, data: &str) -> core::fmt::Result {
        osprint!("{}", data);
        Ok(())
    }
}

// ===========================================================================
// Private functions
// ===========================================================================

/// Initialise our global variables - the BIOS will not have done this for us
/// (as it doesn't know where they are).
#[cfg(all(target_os = "none", not(feature = "lib-mode")))]
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

#[cfg(any(not(target_os = "none"), feature = "lib-mode"))]
unsafe fn start_up_init() {
    // Nothing to do
}

// ===========================================================================
// Public functions / impl for public types
// ===========================================================================

/// This is the function the BIOS calls. This is because we store the address
/// of this function in the ENTRY_POINT_ADDR variable.
#[no_mangle]
pub extern "C" fn os_main(api: &bios::Api) -> ! {
    unsafe {
        start_up_init();
        API.store(api);
    }

    let api = API.get();
    if (api.api_version_get)() != neotron_common_bios::API_VERSION {
        panic!("API mismatch!");
    }

    let config = config::Config::load().unwrap_or_default();

    if config.get_vga_console() {
        // Try and set 80x30 mode for maximum compatibility
        (api.video_set_mode)(bios::video::Mode::new(
            bios::video::Timing::T640x480,
            bios::video::Format::Text8x16,
        ));
        // Work with whatever we get
        let mode = (api.video_get_mode)();
        let (width, height) = (mode.text_width(), mode.text_height());

        if let (Some(width), Some(height)) = (width, height) {
            let mut vga = vgaconsole::VgaConsole::new(
                (api.video_get_framebuffer)(),
                width as isize,
                height as isize,
            );
            vga.clear();
            unsafe {
                VGA_CONSOLE = Some(vga);
            }
            osprintln!("\u{001b}[0mConfigured VGA console {}x{}", width, height);
        }
    }

    if let Some((idx, serial_config)) = config.get_serial_console() {
        let _ignored = (api.serial_configure)(idx, serial_config);
        unsafe { SERIAL_CONSOLE = Some(SerialConsole(idx)) };
        osprintln!("Configured Serial console on Serial {}", idx);
    }

    // Now we can call osprintln!
    osprintln!("\u{001b}[44;33;1m{}\u{001b}[0m", OS_VERSION);
    osprintln!("\u{001b}[41;37;1mCopyright © Jonathan 'theJPster' Pallant and the Neotron Developers, 2022\u{001b}[0m");

    let (tpa_start, tpa_size) = match (api.memory_get_region)(0) {
        bios::FfiOption::None => {
            panic!("No TPA offered by BIOS!");
        }
        bios::FfiOption::Some(tpa) => {
            if tpa.length < 256 {
                panic!("TPA not large enough");
            }
            let offset = tpa.start.align_offset(4);
            (
                unsafe { tpa.start.add(offset) as *mut u32 },
                tpa.length - offset,
            )
        }
    };

    let mut ctx = Ctx {
        config,
        tpa: unsafe {
            // We have to trust the values given to us by the BIOS. If it lies, we will crash.
            program::TransientProgramArea::new(tpa_start, tpa_size)
        },
    };

    osprintln!(
        "\u{001b}[7mTPA: {} bytes @ {:p}\u{001b}[0m",
        ctx.tpa.as_slice_u8().len(),
        ctx.tpa.as_slice_u8().as_ptr()
    );

    // Show the cursor
    osprint!("\u{001b}[?25h");

    let mut buffer = [0u8; 256];
    let mut menu = menu::Runner::new(&commands::OS_MENU, &mut buffer, ctx);

    loop {
        let mut buffer = [0u8; 16];
        let count = unsafe { STD_INPUT.get_data(&mut buffer) };
        for b in &buffer[0..count] {
            menu.input_byte(*b);
        }
        (api.power_idle)();
    }
}

/// Called when we have a panic.
#[inline(never)]
#[panic_handler]
#[cfg(not(any(feature = "lib-mode", test)))]
fn panic(info: &core::panic::PanicInfo) -> ! {
    IS_PANIC.store(true, Ordering::Relaxed);
    osprintln!("PANIC!\n{:#?}", info);
    let api = API.get();
    loop {
        (api.power_idle)();
    }
}

// ===========================================================================
// End of file
// ===========================================================================
