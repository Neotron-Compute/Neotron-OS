//! Configuration related commands for Neotron OS

use crate::{config, println, Ctx};

/// Called when the "config" command is executed.
pub fn command(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], ctx: &mut Ctx) {
    let command = args.get(0).cloned().unwrap_or("print");
    match command {
        "reset" => match config::Config::load() {
            Ok(new_config) => {
                ctx.config = new_config;
                println!("Loaded OK.");
            }
            Err(e) => {
                println!("Error loading; {}", e);
            }
        },
        "save" => match ctx.config.save() {
            Ok(_) => {
                println!("Saved OK.");
            }
            Err(e) => {
                println!("Error saving: {}", e);
            }
        },
        "vga" => match args.get(1).cloned() {
            Some("on") => {
                ctx.config.set_vga_console(true);
                println!("VGA now on");
            }
            Some("off") => {
                ctx.config.set_vga_console(false);
                println!("VGA now off");
            }
            _ => {
                println!("Give on or off as argument");
            }
        },
        "serial" => match (args.get(1).cloned(), args.get(1).map(|s| s.parse::<u32>())) {
            (_, Some(Ok(baud))) => {
                println!("Turning serial console on at {} bps", baud);
                ctx.config.set_serial_console_on(baud);
            }
            (Some("off"), _) => {
                println!("Turning serial console off");
                ctx.config.set_serial_console_off();
            }
            _ => {
                println!("Give off or an integer as argument");
            }
        },
        "print" => {
            println!("VGA   : {}", ctx.config.get_vga_console());
            match ctx.config.get_serial_console() {
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
