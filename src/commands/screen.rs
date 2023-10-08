//! Screen-related commands for Neotron OS

use crate::{
    bios::{
        video::{Format, Mode},
        ApiResult,
    },
    osprint, osprintln, Ctx,
};

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
    help: Some("List possible video modes"),
};

/// Called when the "cls" command is executed.
fn cls_cmd(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    // Reset SGR, go home, clear screen,
    osprint!("\u{001b}[0m\u{001b}[1;1H\u{001b}[2J");
}

/// Called when the "mode" command is executed
fn mode_cmd(_menu: &menu::Menu<Ctx>, item: &menu::Item<Ctx>, args: &[&str], _ctx: &mut Ctx) {
    if let Some(new_mode) = menu::argument_finder(item, args, "new_mode").unwrap() {
        let Ok(mode_num) = new_mode.parse::<u8>() else {
            osprintln!("Invalid integer {:?}", new_mode);
            return;
        };
        let Some(mode) = Mode::try_from_u8(mode_num) else {
            osprintln!("Invalid mode {:?}", new_mode);
            return;
        };
        let has_vga = {
            let mut guard = crate::VGA_CONSOLE.lock();
            guard.as_mut().is_some()
        };
        if !has_vga {
            osprintln!("No VGA console.");
            return;
        }
        let api = crate::API.get();
        match mode.format() {
            Format::Text8x16 => {}
            Format::Text8x8 => {}
            _ => {
                osprintln!("Not a text mode?");
                return;
            }
        }
        match (api.video_set_mode)(mode) {
            ApiResult::Ok(_) => {
                let mut guard = crate::VGA_CONSOLE.lock();
                if let Some(console) = guard.as_mut() {
                    console.change_mode(mode);
                }
                osprintln!("Now in mode {}", mode.as_u8());
            }
            ApiResult::Err(e) => {
                osprintln!("Failed to change mode: {:?}", e);
            }
        }
    } else {
        print_modes();
    }
}

/// Print out all supported video modes
fn print_modes() {
    let api = crate::API.get();
    let current_mode = (api.video_get_mode)();
    let mut any_mode = false;
    for mode_no in 0..255 {
        // Note (unsafe): we'll test if it's right before we try and use it
        let Some(m) = Mode::try_from_u8(mode_no) else {
            continue;
        };
        let is_supported = (api.video_is_valid_mode)(m);
        if is_supported {
            any_mode = true;
            let is_current = if current_mode == m { "*" } else { " " };
            let text_rows = m.text_height();
            let text_cols = m.text_width();
            let f = m.format();
            let width = m.horizontal_pixels();
            let height = m.vertical_lines();
            let hz = m.frame_rate_hz();
            if let (Some(text_rows), Some(text_cols)) = (text_rows, text_cols) {
                // It's a text mode
                osprintln!("{mode_no:3}{is_current}: {width} x {height} @ {hz} Hz {f} ({text_cols} x {text_rows})");
            } else {
                // It's a framebuffer mode
                let f = m.format();
                osprintln!("{mode_no:3}{is_current}: {width} x {height} @ {hz} Hz {f}");
            }
        }
    }
    if !any_mode {
        osprintln!("No valid modes found");
    }
}

// End of file
