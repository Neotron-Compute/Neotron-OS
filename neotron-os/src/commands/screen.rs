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
    help: Some("List/change video mode"),
};

pub static GFX_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: gfx_cmd,
        parameters: &[
            menu::Parameter::Mandatory {
                parameter_name: "new_mode",
                help: Some("The new gfx mode to try"),
            },
            menu::Parameter::Optional {
                parameter_name: "filename",
                help: Some("a file to display"),
            },
        ],
    },
    command: "gfx",
    help: Some("Test a graphics mode"),
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
        if (api.video_mode_needs_vram)(mode) {
            // The OS currently has no VRAM for text modes
            osprintln!("That mode requires more VRAM than the BIOS has.");
            return;
        }
        // # Safety
        //
        // It's always OK to pass NULl to this API.
        match unsafe { (api.video_set_mode)(mode, core::ptr::null_mut()) } {
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

/// Called when the "gfx" command is executed
fn gfx_cmd(_menu: &menu::Menu<Ctx>, item: &menu::Item<Ctx>, args: &[&str], ctx: &mut Ctx) {
    let Some(new_mode) = menu::argument_finder(item, args, "new_mode").unwrap() else {
        osprintln!("Missing arg");
        return;
    };
    let file_name = menu::argument_finder(item, args, "filename").unwrap();
    let Ok(mode_num) = new_mode.parse::<u8>() else {
        osprintln!("Invalid integer {:?}", new_mode);
        return;
    };
    let Some(mode) = Mode::try_from_u8(mode_num) else {
        osprintln!("Invalid mode {:?}", new_mode);
        return;
    };
    let api = crate::API.get();
    let old_mode = (api.video_get_mode)();
    let old_ptr = (api.video_get_framebuffer)();

    let buffer = ctx.tpa.as_slice_u8();
    let buffer_ptr = buffer.as_mut_ptr() as *mut u32;
    if let Some(file_name) = file_name {
        let Ok(file) = crate::FILESYSTEM.open_file(file_name, embedded_sdmmc::Mode::ReadOnly)
        else {
            osprintln!("No such file.");
            return;
        };
        let _ = file.read(buffer);
    } else {
        let (odd_pattern, even_pattern) = match mode.format() {
            // This is alternating hearts and diamonds
            Format::Text8x16 | Format::Text8x8 => (
                u32::from_le_bytes(*b"\x03\x0F\x04\x70"),
                u32::from_le_bytes(*b"\x04\x70\x03\x0F"),
            ),
            // Can't do a checkerboard here - so stripes will do
            Format::Chunky32 => (0x0000_0000, 0x0000_0001),
            // These should produce black/white checkerboard, in the default
            // palette
            Format::Chunky16 => (0x0000_FFFF, 0xFFFF_0000),
            Format::Chunky8 => (0x000F_000F, 0x0F00_0F00),
            Format::Chunky4 => (0x0F0F_0F0F, 0xF0F0_F0F0),
            Format::Chunky2 => (0x3333_3333, 0xCCCC_CCCC),
            Format::Chunky1 => (0x5555_5555, 0xAAAA_AAAA),
            _ => todo!(),
        };
        // draw a dummy non-zero data. In Chunky1 this is a checkerboard.
        let line_size_words = mode.line_size_bytes() / 4;
        for row in 0..mode.vertical_lines() as usize {
            let word = if (row % 2) == 0 {
                even_pattern
            } else {
                odd_pattern
            };
            for col in 0..line_size_words {
                let idx = (row * line_size_words) + col;
                unsafe {
                    buffer_ptr.add(idx).write_volatile(word);
                }
            }
        }
    }

    if let neotron_common_bios::FfiResult::Err(e) =
        unsafe { (api.video_set_mode)(mode, buffer_ptr) }
    {
        osprintln!("Couldn't set mode {}: {:?}", mode_num, e);
    }

    // Now wait for user input
    while crate::STD_INPUT.lock().get_raw().is_none() {
        // spin
    }

    // Put it back as it was
    unsafe {
        (api.video_set_mode)(old_mode, old_ptr);
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
