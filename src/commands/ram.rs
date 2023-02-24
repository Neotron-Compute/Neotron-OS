//! Raw RAM read/write related commands for Neotron OS

use crate::{print, println, Ctx};

pub static HEXDUMP_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: hexdump,
        parameters: &[
            menu::Parameter::Mandatory {
                parameter_name: "address",
                help: Some("Start address"),
            },
            menu::Parameter::Optional {
                parameter_name: "length",
                help: Some("Number of bytes"),
            },
        ],
    },
    command: "hexdump",
    help: Some("Dump the contents of RAM as hex"),
};

pub static LOAD_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: load,
        parameters: &[
            menu::Parameter::Mandatory {
                parameter_name: "address",
                help: Some("Start address"),
            },
            menu::Parameter::Mandatory {
                parameter_name: "hex",
                help: Some("Bytes as hex string"),
            },
        ],
    },
    command: "load",
    help: Some("Load hex bytes into RAM from stdin"),
};

#[cfg(target_os = "none")]
pub static RUN_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: run,
        parameters: &[],
    },
    command: "run",
    help: Some("Jump to start of application area"),
};

fn parse_usize(input: &str) -> Result<usize, core::num::ParseIntError> {
    if let Some(digits) = input.strip_prefix("0x") {
        // Parse as hex
        usize::from_str_radix(digits, 16)
    } else {
        // Parse as decimal
        usize::from_str_radix(input, 10)
    }
}

/// Called when the "hexdump" command is executed.
///
/// If you ask for an address that generates a HardFault, the OS will crash. So
/// don't.
fn hexdump(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], _ctx: &mut Ctx) {
    const BYTES_PER_LINE: usize = 16;

    let Some(address_str) = args.get(0) else {
        println!("No address");
        return;
    };
    let Ok(address) = parse_usize(address_str) else {
        println!("Bad address");
        return;
    };
    let len_str = args.get(1).unwrap_or(&"16");
    let Ok(len) = parse_usize(len_str) else {
        println!("Bad length");
        return;
    };

    let mut ptr = address as *const u8;

    let mut this_line = 0;
    print!("{:08x}: ", address);
    for count in 0..len {
        if this_line == BYTES_PER_LINE {
            println!();
            print!("{:08x}: ", address + count);
            this_line = 1;
        } else {
            this_line += 1;
        }

        let b = unsafe { ptr.read_volatile() };
        print!("{:02x} ", b);
        ptr = unsafe { ptr.offset(1) };
    }
    println!();
}

/// Called when the "load" command is executed.
fn load(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], _ctx: &mut Ctx) {
    let Some(address_str) = args.get(0) else {
        println!("No address");
        return;
    };
    let Ok(address) = parse_usize(address_str) else {
        println!("Bad address");
        return;
    };
    let Some(mut hex_str) = args.get(1).cloned() else {
        println!("No hex");
        return;
    };

    let mut address = address as *mut u8;
    let mut count = 0;
    loop {
        let Some(hex_byte) = hex_str.get(0..2) else {
            println!("Bad hex from {:?}", hex_str);
            return;
        };
        hex_str = &hex_str[2..];
        let Ok(byte)  = u8::from_str_radix(hex_byte, 16) else {
            println!("Bad hex {:?}", hex_byte);
            return;
        };

        unsafe {
            address.write_volatile(byte);
            address = address.offset(1);
        }
        count += 1;

        println!("Loaded {} bytes", count);
    }
}

#[allow(unused)]
#[repr(C)]
pub struct Api {
    pub print: extern "C" fn(data: *const u8, len: usize),
}

static CALLBACK_TABLE: Api = Api { print: print_fn };

extern "C" fn print_fn(data: *const u8, len: usize) {
    let slice = unsafe { core::slice::from_raw_parts(data, len) };
    if let Ok(s) = core::str::from_utf8(slice) {
        print!("{}", s);
    } else {
        // Ignore App output - not UTF-8
    }
}

/// Called when the "run" command is executed.
#[cfg(target_os = "none")]
fn run(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    use core::convert::TryInto;
    const APPLICATION_START_ADDR: usize = 0x2000_1000;
    const APPLICATION_LEN: usize = 4096;
    // Application space starts 4K into Cortex-M SRAM
    let application_ram: &'static mut [u8] = unsafe {
        core::slice::from_raw_parts_mut(APPLICATION_START_ADDR as *mut u8, APPLICATION_LEN)
    };
    let start_word: [u8; 4] = (&application_ram[0..4]).try_into().unwrap();
    let start_ptr = usize::from_le_bytes(start_word) as *const ();
    let result = unsafe {
        let code: extern "C" fn(*const Api) -> u32 = ::core::mem::transmute(start_ptr);
        code(&CALLBACK_TABLE)
    };
    if result != 0 {
        println!("Got error code {}", result);
    }
}
