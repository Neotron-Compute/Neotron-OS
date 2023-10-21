//! Hardware related commands for Neotron OS

use crate::{bios, osprintln, Ctx, API};

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

// End of file
