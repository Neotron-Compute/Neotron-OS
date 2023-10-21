//! Hardware related commands for Neotron OS

use crate::{bios, osprintln, Ctx, API};

use super::{parse_u8, parse_usize};

pub static LSBLK_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: lsblk,
        parameters: &[],
    },
    command: "lsblk",
    help: Some("List all the Block Devices"),
};

pub static LSBUS_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: lsbus,
        parameters: &[],
    },
    command: "lsbus",
    help: Some("List all the Neotron Bus devices"),
};

pub static LSI2C_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: lsi2c,
        parameters: &[],
    },
    command: "lsi2c",
    help: Some("List all the BIOS I2C devices"),
};

pub static LSMEM_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: lsmem,
        parameters: &[],
    },
    command: "lsmem",
    help: Some("List all the BIOS Memory regions"),
};

pub static LSUART_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: lsuart,
        parameters: &[],
    },
    command: "lsuart",
    help: Some("List all the BIOS UARTs"),
};

pub static SHUTDOWN_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: shutdown,
        parameters: &[
            menu::Parameter::Named {
                parameter_name: "reboot",
                help: Some("Reboot after shutting down"),
            },
            menu::Parameter::Named {
                parameter_name: "bootloader",
                help: Some("Reboot into the bootloader after shutting down"),
            },
        ],
    },
    command: "shutdown",
    help: Some("Shutdown the system"),
};

pub static I2C_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: i2c,
        parameters: &[
            menu::Parameter::Mandatory {
                parameter_name: "bus_idx",
                help: Some("I2C bus index"),
            },
            menu::Parameter::Mandatory {
                parameter_name: "dev_addr",
                help: Some("7-bit I2C device address"),
            },
            menu::Parameter::Mandatory {
                parameter_name: "tx_bytes",
                help: Some("Hex string to transmit"),
            },
            menu::Parameter::Mandatory {
                parameter_name: "rx_count",
                help: Some("How many bytes to receive"),
            },
        ],
    },
    command: "i2c",
    help: Some("Do an I2C transaction on a bus"),
};

/// Called when the "lsblk" command is executed.
fn lsblk(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    let api = API.get();
    let mut found = false;

    osprintln!("Block Devices:");
    for dev_idx in 0..=255u8 {
        if let bios::FfiOption::Some(device_info) = (api.block_dev_get_info)(dev_idx) {
            let (bsize, bunits, dsize, dunits) =
                match device_info.num_blocks * u64::from(device_info.block_size) {
                    x if x < (1024 * 1024 * 1024) => {
                        // Under 1 GiB, give it in 10s of MiB
                        (10 * x / (1024 * 1024), "MiB", x / 100_000, "MB")
                    }
                    x => {
                        // Anything else in GiB
                        (10 * x / (1024 * 1024 * 1024), "GiB", x / 100_000_000, "GB")
                    }
                };
            osprintln!("Device {}:", dev_idx);
            osprintln!("\t      Name: {}", device_info.name);
            osprintln!("\t      Type: {:?}", device_info.device_type);
            osprintln!("\tBlock size: {}", device_info.block_size);
            osprintln!("\tNum Blocks: {}", device_info.num_blocks);
            osprintln!(
                "\t Card Size: {}.{} {} ({}.{} {})",
                bsize / 10,
                bsize % 10,
                bunits,
                dsize / 10,
                dsize % 10,
                dunits
            );
            osprintln!("\t Ejectable: {}", device_info.ejectable);
            osprintln!("\t Removable: {}", device_info.removable);
            osprintln!("\t Read Only: {}", device_info.read_only);
            osprintln!(
                "\t     Media: {}",
                if device_info.media_present {
                    "Present"
                } else {
                    "Missing"
                }
            );
            found = true;
        }
    }
    if !found {
        osprintln!("\tNone");
    }
}

/// Called when the "lsbus" command is executed.
fn lsbus(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    let api = API.get();
    let mut found = false;
    osprintln!("Neotron Bus Devices:");
    for dev_idx in 0..=255u8 {
        if let bios::FfiOption::Some(device_info) = (api.bus_get_info)(dev_idx) {
            let kind = match device_info.kind {
                bios::bus::PeripheralKind::Slot => "Slot",
                bios::bus::PeripheralKind::SdCard => "SdCard",
                bios::bus::PeripheralKind::Reserved => "Reserved",
            };
            osprintln!("\t{}: {} ({})", dev_idx, device_info.name, kind);
            found = true;
        }
    }
    if !found {
        osprintln!("\tNone");
    }
}

/// Called when the "lsi2c" command is executed.
fn lsi2c(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    let api = API.get();
    let mut found = false;
    osprintln!("I2C Buses:");
    for dev_idx in 0..=255u8 {
        if let bios::FfiOption::Some(device_info) = (api.i2c_bus_get_info)(dev_idx) {
            osprintln!("\t{}: {}", dev_idx, device_info.name);
            found = true;
        }
    }
    if !found {
        osprintln!("\tNone");
    }
}

/// Called when the "lsmem" command is executed.
fn lsmem(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    let api = API.get();
    let mut found = false;
    osprintln!("Memory regions:");
    for region_idx in 0..=255u8 {
        if let bios::FfiOption::Some(region) = (api.memory_get_region)(region_idx) {
            osprintln!("\t{}: {}", region_idx, region);
            found = true;
        }
    }
    if !found {
        osprintln!("\tNone");
    }
}

/// Called when the "lsuart" command is executed.
fn lsuart(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    let api = API.get();
    let mut found = false;
    osprintln!("UART Devices:");
    for dev_idx in 0..=255u8 {
        if let bios::FfiOption::Some(device_info) = (api.serial_get_info)(dev_idx) {
            let device_type = match device_info.device_type {
                bios::serial::DeviceType::Rs232 => "RS232",
                bios::serial::DeviceType::TtlUart => "TTL",
                bios::serial::DeviceType::UsbCdc => "USB",
                bios::serial::DeviceType::Midi => "MIDI",
            };
            osprintln!("\t{}: {} ({})", dev_idx, device_info.name, device_type);
            found = true;
        }
    }
    if !found {
        osprintln!("\tNone");
    }
}

/// Called when the "shutdown" command is executed.
fn shutdown(_menu: &menu::Menu<Ctx>, item: &menu::Item<Ctx>, args: &[&str], _ctx: &mut Ctx) {
    let api = API.get();
    if let Ok(Some(_)) = menu::argument_finder(item, args, "reboot") {
        osprintln!("Rebooting...");
        (api.power_control)(bios::PowerMode::Reset);
    } else if let Ok(Some(_)) = menu::argument_finder(item, args, "bootloader") {
        osprintln!("Rebooting into bootloader...");
        (api.power_control)(bios::PowerMode::Bootloader);
    } else {
        osprintln!("Shutting down...");
        (api.power_control)(bios::PowerMode::Off);
    }
}

/// Called when the "i2c" command is executed.
fn i2c(_menu: &menu::Menu<Ctx>, item: &menu::Item<Ctx>, args: &[&str], _ctx: &mut Ctx) {
    let bus_idx = menu::argument_finder(item, args, "bus_idx").unwrap();
    let dev_addr = menu::argument_finder(item, args, "dev_addr").unwrap();
    let tx_bytes = menu::argument_finder(item, args, "tx_bytes").unwrap();
    let rx_count = menu::argument_finder(item, args, "rx_count").unwrap();

    let (Some(bus_idx), Some(dev_addr), Some(tx_bytes), Some(rx_count)) =
        (bus_idx, dev_addr, tx_bytes, rx_count)
    else {
        osprintln!("Missing arguments.");
        return;
    };

    let mut tx_buffer: heapless::Vec<u8, 16> = heapless::Vec::new();

    for hex_pair in tx_bytes.as_bytes().chunks(2) {
        let Some(top) = hex_digit(hex_pair[0]) else {
            osprintln!("Bad hex.");
            return;
        };
        let Some(bottom) = hex_digit(hex_pair[1]) else {
            osprintln!("Bad hex.");
            return;
        };
        let byte = top << 4 | bottom;
        let Ok(_) = tx_buffer.push(byte) else {
            osprintln!("Too much hex.");
            return;
        };
    }

    let Ok(bus_idx) = parse_u8(bus_idx) else {
        osprintln!("Bad bus_idx");
        return;
    };

    let Ok(dev_addr) = parse_u8(dev_addr) else {
        osprintln!("Bad dev_addr");
        return;
    };

    let Ok(rx_count) = parse_usize(rx_count) else {
        osprintln!("Bad rx count.");
        return;
    };

    let mut rx_buf = [0u8; 16];

    let Some(rx_buf) = rx_buf.get_mut(0..rx_count) else {
        osprintln!("Too much rx.");
        return;
    };

    let api = API.get();

    match (api.i2c_write_read)(
        bus_idx,
        dev_addr,
        tx_buffer.as_slice().into(),
        bios::FfiByteSlice::empty(),
        rx_buf.into(),
    ) {
        bios::FfiResult::Ok(_) => {
            osprintln!("Ok, got {:x?}", rx_buf);
        }
        bios::FfiResult::Err(e) => {
            osprintln!("Failed: {:?}", e);
        }
    }
}

/// Convert an ASCII hex digit into a number
fn hex_digit(input: u8) -> Option<u8> {
    match input {
        b'0' => Some(0),
        b'1' => Some(1),
        b'2' => Some(2),
        b'3' => Some(3),
        b'4' => Some(4),
        b'5' => Some(5),
        b'6' => Some(6),
        b'7' => Some(7),
        b'8' => Some(8),
        b'9' => Some(9),
        b'a' | b'A' => Some(10),
        b'b' | b'B' => Some(11),
        b'c' | b'C' => Some(12),
        b'd' | b'D' => Some(13),
        b'e' | b'E' => Some(14),
        b'f' | b'F' => Some(15),
        _ => None,
    }
}

// End of file
