//! Hardware related commands for Neotron OS

use crate::{bios, osprintln, Ctx, API};

pub static LSHW_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: lshw,
        parameters: &[],
    },
    command: "lshw",
    help: Some("List all the BIOS hardware"),
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

/// Called when the "lshw" command is executed.
fn lshw(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    let api = API.get();
    let mut found = false;

    osprintln!("Memory regions:");
    for region_idx in 0..=255u8 {
        if let bios::FfiOption::Some(region) = (api.memory_get_region)(region_idx) {
            osprintln!("  {}: {}", region_idx, region);
            found = true;
        }
    }
    if !found {
        osprintln!("  None");
    }

    found = false;

    osprintln!("Serial Devices:");
    for dev_idx in 0..=255u8 {
        if let bios::FfiOption::Some(device_info) = (api.serial_get_info)(dev_idx) {
            osprintln!(
                "  {}: {} {:?}",
                dev_idx,
                device_info.name,
                device_info.device_type
            );
            found = true;
        }
    }
    if !found {
        osprintln!("  None");
    }

    found = false;

    osprintln!("Block Devices:");
    for dev_idx in 0..=255u8 {
        if let bios::FfiOption::Some(device_info) = (api.block_dev_get_info)(dev_idx) {
            osprintln!(
                "  {}: {} {:?} bs={} size={} MiB",
                dev_idx,
                device_info.name,
                device_info.device_type,
                device_info.block_size,
                (device_info.num_blocks * u64::from(device_info.block_size)) / (1024 * 1024)
            );
            found = true;
        }
    }
    if !found {
        osprintln!("  None");
    }

    found = false;

    osprintln!("I2C Buses:");
    for dev_idx in 0..=255u8 {
        if let bios::FfiOption::Some(device_info) = (api.i2c_bus_get_info)(dev_idx) {
            osprintln!("  {}: {:?}", dev_idx, device_info);
            found = true;
        }
    }
    if !found {
        osprintln!("  None");
    }

    found = false;

    osprintln!("Neotron Bus Devices:");
    for dev_idx in 0..=255u8 {
        if let bios::FfiOption::Some(device_info) = (api.bus_get_info)(dev_idx) {
            osprintln!("  {}: {:?}", dev_idx, device_info);
            found = true;
        }
    }
    if !found {
        osprintln!("  None");
    }

    found = false;

    osprintln!("Audio Mixers:");
    for dev_idx in 0..=255u8 {
        if let bios::FfiOption::Some(device_info) = (api.audio_mixer_channel_get_info)(dev_idx) {
            let dir = match device_info.direction {
                bios::audio::Direction::Input => "In",
                bios::audio::Direction::Output => "Out",
                bios::audio::Direction::Loopback => "Loop",
            };
            osprintln!(
                "  {}: {:08} ({}) {}/{}",
                dev_idx,
                device_info.name,
                dir,
                device_info.current_level,
                device_info.max_level
            );
            found = true;
        }
    }
    if !found {
        osprintln!("  None");
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
