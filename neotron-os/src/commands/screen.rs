//! Screen-related commands for Neotron OS

use crate::{bios::video::Mode, osprint, osprintln, Ctx};

pub static CLS_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: cls_cmd,
        parameters: &[],
    },
    command: "cls",
    help: Some("Clear the screen"),
};

pub static MODE_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: mode_cmd,
        parameters: &[menu::Parameter::Optional {
            parameter_name: "new_mode",
            help: Some("The new text mode to change to"),
        }],
    },
    command: "mode",
    help: Some("List/change video mode"),
};

/// Called when the "cls" command is executed.
fn cls_cmd(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    // Reset SGR, go home, clear screen,
    osprint!("\u{001b}[0m\u{001b}[1;1H\u{001b}[2J");
}

/// Called when the "mode" command is executed
fn mode_cmd(_menu: &menu::Menu<Ctx>, item: &menu::Item<Ctx>, args: &[&str], _ctx: &mut Ctx) {
    let Some(new_mode) = menu::argument_finder(item, args, "new_mode").unwrap() else {
        print_modes();
        return;
    };
    let Ok(mode_num) = new_mode.parse::<u8>() else {
        osprintln!("Invalid integer {:?}", new_mode);
        return;
    };
    let Some(mode) = Mode::try_from_u8(mode_num) else {
        osprintln!("Invalid mode {:?}", new_mode);
        return;
    };
    let mut lock = crate::VGA_CONSOLE.lock();
    let Some(vga_console) = lock.as_mut() else {
        osprintln!("No VGA console.");
        return;
    };
    // Passing a null pointer is OK here because the BIOS will either allocate
    // space, or give us a blank screen.
    match unsafe { vga_console.change_mode(mode, core::ptr::null_mut()) } {
        Ok(_) => {
            osprintln!("Changed to mode {}", mode_num);
        }
        Err(e) => {
            osprintln!("Failed to set mode {}: BIOS said {:?}", mode_num, e);
        }
    }
}

/// Print out all supported video modes
fn print_modes() {
    let api = crate::API.get();
    let current_mode = (api.video_get_mode)();
    let mut any_mode = false;

    let formats = [
        ("T16 ", neotron_common_bios::video::Format::Text8x16),
        ("T8  ", neotron_common_bios::video::Format::Text8x8),
        ("C32 ", neotron_common_bios::video::Format::Chunky32),
        ("C16 ", neotron_common_bios::video::Format::Chunky16),
        ("C8  ", neotron_common_bios::video::Format::Chunky8),
        ("C4  ", neotron_common_bios::video::Format::Chunky4),
        ("C2  ", neotron_common_bios::video::Format::Chunky2),
        ("C1  ", neotron_common_bios::video::Format::Chunky1),
    ];

    osprint!("        ");
    for (name, _) in formats {
        osprint!("{}", name);
    }
    osprintln!();

    for scaling in [
        neotron_common_bios::video::Scaling::None,
        neotron_common_bios::video::Scaling::DoubleHeight,
        neotron_common_bios::video::Scaling::DoubleWidth,
        neotron_common_bios::video::Scaling::DoubleWidthAndHeight,
    ] {
        for timing in [
            neotron_common_bios::video::Timing::T640x480,
            neotron_common_bios::video::Timing::T640x400,
            neotron_common_bios::video::Timing::T800x600,
        ] {
            // check if any formats work for this timing mode
            let mut any_format = false;
            for (_, format) in formats {
                let m = neotron_common_bios::video::Mode::new_with_scaling(timing, format, scaling);
                let is_supported = (api.video_is_valid_mode)(m);
                if is_supported {
                    any_format = true;
                    break;
                }
            }
            // if there's a valid format, print the line (otherwise skip it for brevity)
            if any_format {
                let basic_mode = neotron_common_bios::video::Mode::new_with_scaling(
                    timing,
                    neotron_common_bios::video::Format::Chunky1,
                    scaling,
                );
                osprint!(
                    "{:03}x{:03}:",
                    basic_mode.horizontal_pixels(),
                    basic_mode.vertical_lines()
                );
                for (_, format) in formats {
                    let m =
                        neotron_common_bios::video::Mode::new_with_scaling(timing, format, scaling);
                    let is_supported = (api.video_is_valid_mode)(m);
                    if is_supported {
                        osprint!(
                            "{:03}{}",
                            m.as_u8(),
                            if current_mode == m { "<" } else { " " }
                        );
                        any_mode = true;
                    } else {
                        osprint!("--- ");
                    }
                }
                osprintln!();
            }
        }
    }

    if !any_mode {
        osprintln!("No valid modes found");
    }
}

// End of file
