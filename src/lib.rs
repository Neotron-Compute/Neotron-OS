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

mod charmap;
mod config;
mod vgaconsole;

// ===========================================================================
// Global Variables
// ===========================================================================

/// The OS version string
const OS_VERSION: &str = concat!("Neotron OS, version ", env!("OS_VERSION"));

/// We store the API object supplied by the BIOS here
static API: Api = Api::new();

/// We store our VGA console here.
static mut VGA_CONSOLE: Option<vgaconsole::VgaConsole> = None;

/// We store our VGA console here.
static mut SERIAL_CONSOLE: Option<SerialConsole> = None;

static OS_MENU: menu::Menu<Ctx> = menu::Menu {
    label: "root",
    items: &[
        &menu::Item {
            item_type: menu::ItemType::Callback {
                function: cmd_mem,
                parameters: &[],
            },
            command: "mem",
            help: Some("Show memory regions"),
        },
        &menu::Item {
            item_type: menu::ItemType::Callback {
                function: cmd_clear,
                parameters: &[],
            },
            command: "clear",
            help: Some("Clear the screen"),
        },
        &menu::Item {
            item_type: menu::ItemType::Callback {
                function: cmd_fill,
                parameters: &[],
            },
            command: "fill",
            help: Some("Fill the screen with characters"),
        },
    ],
    entry: None,
    exit: None,
};

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

impl Api {
    const fn new() -> Api {
        Api {
            bios: core::sync::atomic::AtomicPtr::new(core::ptr::null_mut()),
        }
    }

    fn store(&self, api: *const bios::Api) {
        self.bios
            .store(api as *mut bios::Api, core::sync::atomic::Ordering::SeqCst)
    }

    fn get(&self) -> &'static bios::Api {
        unsafe { &*(self.bios.load(core::sync::atomic::Ordering::SeqCst) as *const bios::Api) }
    }
}

struct Api {
    bios: core::sync::atomic::AtomicPtr<bios::Api>,
}

/// Represents the serial port we can use as a text input/output device.
struct SerialConsole(u8);

impl core::fmt::Write for SerialConsole {
    fn write_str(&mut self, data: &str) -> core::fmt::Result {
        let api = API.get();
        (api.serial_write)(
            // Which port
            self.0,
            // Data
            bios::ApiByteSlice::new(data.as_bytes()),
            // No timeout
            bios::Option::None,
        )
        .unwrap();
        Ok(())
    }
}

struct Ctx;

impl core::fmt::Write for Ctx {
    fn write_str(&mut self, data: &str) -> core::fmt::Result {
        print!("{}", data);
        Ok(())
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
pub extern "C" fn main(api: *const bios::Api) -> ! {
    unsafe {
        start_up_init();
        API.store(api);
    }

    let api = API.get();
    if (api.api_version_get)() != neotron_common_bios::API_VERSION {
        panic!("API mismatch!");
    }

    let config = config::Config::load().unwrap_or_default();

    if config.has_vga_console() {
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

    let mut found;

    println!("Serial Ports:");
    found = false;
    for device_idx in 0..=255 {
        if let bios::Option::Some(serial) = (api.serial_get_info)(device_idx) {
            println!("Serial Device {}: {:?}", device_idx, serial);
            found = true;
        } else {
            // Ran out of serial devices (we assume they are consecutive)
            break;
        }
    }
    if !found {
        println!("None.");
    }

    let mut keyboard = charmap::UKEnglish::new();
    let mut buffer = [0u8; 256];
    let mut menu = menu::Runner::new(&OS_MENU, &mut buffer, Ctx);

    loop {
        if let neotron_common_bios::Result::Ok(neotron_common_bios::Option::Some(ev)) =
            (api.hid_get_event)()
        {
            if let Some(charmap::Keypress::Unicode(ch)) = keyboard.handle_event(ev) {
                let mut buffer = [0u8; 6];
                let s = ch.encode_utf8(&mut buffer);
                for b in s.as_bytes() {
                    menu.input_byte(*b);
                }
            }
        }
        (api.power_idle)();
    }
}

/// Called when the "mem" command is executed.
fn cmd_mem(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _context: &mut Ctx) {
    println!("Memory Regions:");
    let mut found = false;
    let api = API.get();
    for region_idx in 0..=255 {
        if let bios::Option::Some(region) = (api.memory_get_region)(region_idx) {
            println!("Region {}: {}", region_idx, region);
            found = true;
        } else {
            // Ran out of regions (we assume they are consecutive)
            break;
        }
    }
    if !found {
        println!("None.");
    }
}

/// Called when the "clear" command is executed.
fn cmd_clear(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _context: &mut Ctx) {
    if let Some(ref mut console) = unsafe { &mut VGA_CONSOLE } {
        console.clear();
    }
}

/// Called when the "fill" command is executed.
fn cmd_fill(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _context: &mut Ctx) {
    if let Some(ref mut console) = unsafe { &mut VGA_CONSOLE } {
        console.clear();
    }
    let api = API.get();
    let mode = (api.video_get_mode)();
    let (Some(width), Some(height)) = (mode.text_width(), mode.text_height()) else {
        println!("Unable to get console size");
        return;
    };
    // A range of printable ASCII compatible characters
    let mut char_cycle = (' '..='~').cycle();
    // Scroll two screen fulls
    for _row in 0..height * 2 {
        for _col in 0..width {
            print!("{}", char_cycle.next().unwrap());
        }
    }
}

/// Called when we have a panic.
#[inline(never)]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    println!("PANIC!\n{:#?}", info);
    let api = API.get();
    loop {
        (api.power_idle)();
    }
}

// ===========================================================================
// End of file
// ===========================================================================
