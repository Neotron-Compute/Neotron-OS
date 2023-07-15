//! Raw RAM read/write related commands for Neotron OS

use crate::{osprint, osprintln, Ctx};

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

pub static RUN_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: run,
        parameters: &[],
    },
    command: "run",
    help: Some("Jump to start of application area"),
};

pub static LOAD_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: loadf,
        parameters: &[
            menu::Parameter::Mandatory {
                parameter_name: "address",
                help: Some("The address to load from"),
            },
            menu::Parameter::Mandatory {
                parameter_name: "length",
                help: Some("The number of bytes to load"),
            },
        ],
    },
    command: "loadf",
    help: Some("Copy a program from ROM/RAM into the application area"),
};

fn parse_usize(input: &str) -> Result<usize, core::num::ParseIntError> {
    if let Some(digits) = input.strip_prefix("0x") {
        // Parse as hex
        usize::from_str_radix(digits, 16)
    } else {
        // Parse as decimal
        input.parse::<usize>()
    }
}

/// Called when the "hexdump" command is executed.
///
/// If you ask for an address that generates a HardFault, the OS will crash. So
/// don't.
fn hexdump(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], _ctx: &mut Ctx) {
    const BYTES_PER_LINE: usize = 16;

    let Some(address_str) = args.first() else {
        osprintln!("No address");
        return;
    };
    let Ok(address) = parse_usize(address_str) else {
        osprintln!("Bad address");
        return;
    };
    let len_str = args.get(1).unwrap_or(&"16");
    let Ok(len) = parse_usize(len_str) else {
        osprintln!("Bad length");
        return;
    };

    let mut ptr = address as *const u8;

    let mut this_line = 0;
    osprint!("{:08x}: ", address);
    for count in 0..len {
        if this_line == BYTES_PER_LINE {
            osprintln!();
            osprint!("{:08x}: ", address + count);
            this_line = 1;
        } else {
            this_line += 1;
        }

        let b = unsafe { ptr.read_volatile() };
        osprint!("{:02x} ", b);
        ptr = unsafe { ptr.offset(1) };
    }
    osprintln!();
}

/// Called when the "run" command is executed.
fn run(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], ctx: &mut Ctx) {
    match ctx.tpa.execute() {
        Ok(0) => {
            osprintln!();
        }
        Ok(n) => {
            osprintln!("\nError Code: {}", n);
        }
        Err(e) => {
            osprintln!("\nFailed to execute: {:?}", e);
        }
    }
}

/// Called when the "loadf" command is executed.
///
/// If you ask for an address that generates a HardFault, the OS will crash. So
/// don't.
fn loadf(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], ctx: &mut Ctx) {
    let Some(address_str) = args.first() else {
        osprintln!("No address");
        return;
    };
    let Ok(address) = parse_usize(address_str) else {
        osprintln!("Bad address");
        return;
    };
    let Some(len_str) = args.get(1) else {
        osprintln!("No length");
        return;
    };
    let Ok(len) = parse_usize(len_str) else {
        osprintln!("Bad length");
        return;
    };

    let src_slice = unsafe { core::slice::from_raw_parts(address as *const u8, len) };

    match ctx.tpa.copy_program(src_slice) {
        Ok(_) => {
            osprintln!("Ok");
        }
        Err(e) => {
            osprintln!("Error: {:?}", e);
        }
    }
}
