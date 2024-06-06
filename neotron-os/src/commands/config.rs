//! Configuration related commands for Neotron OS

use crate::{bios, config, osprintln, Ctx};

pub static COMMAND_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: command,
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
};

/// Called when the "config" command is executed.
fn command(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], ctx: &mut Ctx) {
    let command = args.first().cloned().unwrap_or("print");
    match command {
        "reset" => match config::Config::load() {
            Ok(new_config) => {
                ctx.config = new_config;
                osprintln!("Loaded OK.");
            }
            Err(e) => {
                osprintln!("Error loading; {}", e);
            }
        },
        "save" => match ctx.config.save() {
            Ok(_) => {
                osprintln!("Saved OK.");
            }
            Err(e) => {
                osprintln!("Error saving: {}", e);
            }
        },
        "vga" => match args.get(1).cloned() {
            Some("off") => {
                ctx.config.set_vga_console(None);
                osprintln!("VGA now off");
            }
            Some(mode_str) => {
                let Some(mode) = mode_str
                    .parse::<u8>()
                    .ok()
                    .and_then(bios::video::Mode::try_from_u8)
                    .filter(|m| m.is_text_mode())
                else {
                    osprintln!("Not a valid text mode");
                    return;
                };
                ctx.config.set_vga_console(Some(mode));
                osprintln!("VGA set to mode {}", mode.as_u8());
            }
            _ => {
                osprintln!("Give integer or off as argument");
            }
        },
        "serial" => match (args.get(1).cloned(), args.get(1).map(|s| s.parse::<u32>())) {
            (_, Some(Ok(baud))) => {
                osprintln!("Turning serial console on at {} bps", baud);
                ctx.config.set_serial_console_on(baud);
            }
            (Some("off"), _) => {
                osprintln!("Turning serial console off");
                ctx.config.set_serial_console_off();
            }
            _ => {
                osprintln!("Give off or an integer as argument");
            }
        },
        "print" => {
            match ctx.config.get_vga_console() {
                Some(m) => {
                    osprintln!("VGA   : Mode {}", m.as_u8());
                }
                None => {
                    osprintln!("VGA   : off");
                }
            };
            match ctx.config.get_serial_console() {
                None => {
                    osprintln!("Serial: off");
                }
                Some((_port, config)) => {
                    osprintln!("Serial: {} bps", config.data_rate_bps);
                }
            }
        }
        _ => {
            osprintln!("config print - print the config");
            osprintln!("config help - print this help text");
            osprintln!("config reset - load config from BIOS store");
            osprintln!("config save - save config to BIOS store");
            osprintln!("config vga <n> - enable VGA in Mode <n>");
            osprintln!("config vga off - turn VGA off");
            osprintln!("config serial off - turn serial console off");
            osprintln!("config serial <baud> - turn serial console on with given baud rate");
        }
    }
}

// End of file
