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
use core::sync::atomic::{AtomicBool, Ordering};
use neotron_common_bios as bios;

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

/// Note if we are panicking right now.
///
/// If so, don't panic if a serial write fails.
static IS_PANIC: AtomicBool = AtomicBool::new(false);

static OS_MENU: menu::Menu<Ctx> = menu::Menu {
    label: "root",
    items: &[
        &menu::Item {
            item_type: menu::ItemType::Callback {
                function: cmd_lshw,
                parameters: &[],
            },
            command: "lshw",
            help: Some("List all the hardware"),
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
        &menu::Item {
            item_type: menu::ItemType::Callback {
                function: cmd_config,
                parameters: &[
                    menu::Parameter::Optional {
                        parameter_name: "command",
                        help: Some("Which operation to perform (try help)"),
                    },
                    menu::Parameter::Optional {
                        parameter_name: "value",
                        help: Some("new value for the setting"),
                    },
                ],
            },
            command: "config",
            help: Some("Handle non-volatile OS configuration"),
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
        let is_panic = IS_PANIC.load(Ordering::SeqCst);
        let res = (api.serial_write)(
            // Which port
            self.0,
            // Data
            bios::ApiByteSlice::new(data.as_bytes()),
            // No timeout
            bios::Option::None,
        );
        if !is_panic {
            res.unwrap();
        }
        Ok(())
    }
}

struct Ctx {
    config: config::Config,
}

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
            println!("Configured VGA console {}x{}", width, height);
        }
    }

    if let Some((idx, serial_config)) = config.get_serial_console() {
        let _ignored = (api.serial_configure)(idx, serial_config);
        unsafe { SERIAL_CONSOLE = Some(SerialConsole(idx)) };
        println!("Configured Serial console on Serial {}", idx);
    }

    // Now we can call println!
    println!("Welcome to {}!", OS_VERSION);
    println!("Copyright © Jonathan 'theJPster' Pallant and the Neotron Developers, 2022");

    let ctx = Ctx { config };

    let mut keyboard = pc_keyboard::EventDecoder::new(
        pc_keyboard::layouts::AnyLayout::Uk105Key(pc_keyboard::layouts::Uk105Key),
        pc_keyboard::HandleControl::MapLettersToUnicode,
    );
    let mut buffer = [0u8; 256];
    let mut menu = menu::Runner::new(&OS_MENU, &mut buffer, ctx);

    loop {
        match (api.hid_get_event)() {
            bios::Result::Ok(bios::Option::Some(bios::hid::HidEvent::KeyPress(code))) => {
                let pckb_ev = pc_keyboard::KeyEvent {
                    code,
                    state: pc_keyboard::KeyState::Down,
                };
                if let Some(pc_keyboard::DecodedKey::Unicode(mut ch)) =
                    keyboard.process_keyevent(pckb_ev)
                {
                    if ch == '\n' {
                        ch = '\r';
                    }
                    let mut buffer = [0u8; 6];
                    let s = ch.encode_utf8(&mut buffer);
                    for b in s.as_bytes() {
                        menu.input_byte(*b);
                    }
                }
            }
            bios::Result::Ok(bios::Option::Some(bios::hid::HidEvent::KeyRelease(code))) => {
                let pckb_ev = pc_keyboard::KeyEvent {
                    code,
                    state: pc_keyboard::KeyState::Up,
                };
                if let Some(pc_keyboard::DecodedKey::Unicode(ch)) =
                    keyboard.process_keyevent(pckb_ev)
                {
                    let mut buffer = [0u8; 6];
                    let s = ch.encode_utf8(&mut buffer);
                    for b in s.as_bytes() {
                        menu.input_byte(*b);
                    }
                }
            }
            bios::Result::Ok(bios::Option::Some(bios::hid::HidEvent::MouseInput(_ignore))) => {}
            bios::Result::Ok(bios::Option::None) => {
                // Do nothing
            }
            bios::Result::Err(e) => {
                println!("Failed to get HID events: {:?}", e);
            }
        }
        (api.power_idle)();
    }
}

/// Called when the "lshw" command is executed.
fn cmd_lshw(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _context: &mut Ctx) {
    let api = API.get();
    let mut found = false;

    println!("Memory regions:");
    for region_idx in 0..=255u8 {
        if let bios::Option::Some(region) = (api.memory_get_region)(region_idx) {
            println!("  {}: {}", region_idx, region);
            found = true;
        }
    }
    if !found {
        println!("  None");
    }

    println!();
    found = false;

    println!("Serial Devices:");
    for dev_idx in 0..=255u8 {
        if let bios::Option::Some(device_info) = (api.serial_get_info)(dev_idx) {
            println!("  {}: {:?}", dev_idx, device_info);
            found = true;
        }
    }
    if !found {
        println!("  None");
    }

    println!();
    found = false;

    println!("Block Devices:");
    for dev_idx in 0..=255u8 {
        if let bios::Option::Some(device_info) = (api.block_dev_get_info)(dev_idx) {
            println!("  {}: {:?}", dev_idx, device_info);
            found = true;
        }
    }
    if !found {
        println!("  None");
    }

    println!();
    found = false;

    println!("I2C Buses:");
    for dev_idx in 0..=255u8 {
        if let bios::Option::Some(device_info) = (api.i2c_bus_get_info)(dev_idx) {
            println!("  {}: {:?}", dev_idx, device_info);
            found = true;
        }
    }
    if !found {
        println!("  None");
    }

    println!();
    found = false;

    println!("Neotron Bus Devices:");
    for dev_idx in 0..=255u8 {
        if let bios::Option::Some(device_info) = (api.bus_get_info)(dev_idx) {
            println!("  {}: {:?}", dev_idx, device_info);
            found = true;
        }
    }
    if !found {
        println!("  None");
    }

    println!();
    found = false;

    println!("Audio Mixers:");
    for dev_idx in 0..=255u8 {
        if let bios::Result::Ok(device_info) = (api.audio_mixer_channel_get_info)(dev_idx) {
            println!("  {}: {:?}", dev_idx, device_info);
            found = true;
        }
    }
    if !found {
        println!("  None");
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

/// Called when the "config" command is executed.
fn cmd_config(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], context: &mut Ctx) {
    let command = args.get(0).cloned().unwrap_or("print");
    match command {
        "reset" => match config::Config::load() {
            Ok(new_config) => {
                context.config = new_config;
                println!("Loaded OK.");
            }
            Err(e) => {
                println!("Error loading; {}", e);
            }
        },
        "save" => match context.config.save() {
            Ok(_) => {
                println!("Saved OK.");
            }
            Err(e) => {
                println!("Error saving: {}", e);
            }
        },
        "vga" => match args.get(1).cloned() {
            Some("on") => {
                context.config.set_vga_console(true);
                println!("VGA now on");
            }
            Some("off") => {
                context.config.set_vga_console(false);
                println!("VGA now off");
            }
            _ => {
                println!("Give on or off as argument");
            }
        },
        "serial" => match (args.get(1).cloned(), args.get(1).map(|s| s.parse::<u32>())) {
            (_, Some(Ok(baud))) => {
                println!("Turning serial console on at {} bps", baud);
                context.config.set_serial_console_on(baud);
            }
            (Some("off"), _) => {
                println!("Turning serial console off");
                context.config.set_serial_console_off();
            }
            _ => {
                println!("Give off or an integer as argument");
            }
        },
        "print" => {
            println!("VGA   : {}", context.config.get_vga_console());
            match context.config.get_serial_console() {
                None => {
                    println!("Serial: off");
                }
                Some((_port, config)) => {
                    println!("Serial: {} bps", config.data_rate_bps);
                }
            }
        }
        _ => {
            println!("config print - print the config");
            println!("config help - print this help text");
            println!("config reset - load config from BIOS store");
            println!("config save - save config to BIOS store");
            println!("config vga on - turn VGA on");
            println!("config vga off - turn VGA off");
            println!("config serial off - turn serial console off");
            println!("config serial <baud> - turn serial console on with given baud rate");
        }
    }
}

/// Called when we have a panic.
#[inline(never)]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    IS_PANIC.store(true, Ordering::SeqCst);
    println!("PANIC!\n{:#?}", info);
    let api = API.get();
    loop {
        (api.power_idle)();
    }
}

// ===========================================================================
// End of file
// ===========================================================================
