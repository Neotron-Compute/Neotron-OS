//! Screen-related commands for Neotron OS

use pc_keyboard::DecodedKey;

static SLIDES: [&[u8]; 8] = [
    include_bytes!("../slide_pico_vga.bmp"),
    include_bytes!("../slide_pico_audio.bmp"),
    include_bytes!("../slide_bios.bmp"),
    include_bytes!("../slide_os.bmp"),
    include_bytes!("../slide_oss.bmp"),
    include_bytes!("../slide_links.bmp"),
    include_bytes!("../slide_stars.bmp"),
    include_bytes!("../slide_px3.bmp"),
];

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

pub static DEMO_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: demo_cmd,
        parameters: &[],
    },
    command: "demo",
    help: Some("Run demo"),
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
        // draw a dummy non-zero data. In Chunky1 this is a checkerboard.
        let line_size_words = mode.line_size_bytes() / 4;
        for row in 0..mode.vertical_lines() as usize {
            let word = if (row % 2) == 0 {
                0x5555_5555
            } else {
                0xAAAA_AAAA
            };
            for col in 0..line_size_words {
                let idx = (row * line_size_words) + col;
                unsafe {
                    // Let's try stripes?
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
    'wait: loop {
        let keyin = crate::STD_INPUT.lock().get_raw();
        if let Some(DecodedKey::Unicode('Q') | DecodedKey::Unicode('q')) = keyin {
            break 'wait;
        }
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

/// Called when the "demo" command is executed.
fn demo_cmd(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], ctx: &mut Ctx) {
    let api = crate::API.get();
    let old_mode = (api.video_get_mode)();
    let old_ptr = (api.video_get_framebuffer)();
    let buffer = ctx.tpa.as_slice_u8();
    let buffer_ptr = buffer.as_mut_ptr() as *mut u32;
    let old_palette = [
        (api.video_get_palette)(0),
        (api.video_get_palette)(1),
        (api.video_get_palette)(2),
        (api.video_get_palette)(3),
    ];
    if let neotron_common_bios::FfiResult::Err(e) =
        unsafe { (api.video_set_mode)(Mode::from_u8(6), buffer_ptr) }
    {
        osprintln!("Couldn't set mode 6: {:?}", e);
        return;
    }

    'slides: for slide_bytes in SLIDES.iter().cycle().cloned() {
        if let Err(_e) = show_slide(slide_bytes, api, buffer) {
            break;
        }
        // Now wait for user input - Q to quit, ' ' to skip
        'wait: for _ in 0..300 {
            // 300 frames = 5 seconds
            (api.video_wait_for_line)(478);
            (api.video_wait_for_line)(479);
            let keyin = crate::STD_INPUT.lock().get_raw();
            if let Some(DecodedKey::Unicode('Q') | DecodedKey::Unicode('q')) = keyin {
                break 'slides;
            }
            if let Some(DecodedKey::Unicode(' ')) = keyin {
                break 'wait;
            }
        }
    }

    // Put it back as it was
    unsafe {
        (api.video_set_mode)(old_mode, old_ptr);
        for (idx, colour) in old_palette.iter().enumerate() {
            if let neotron_common_bios::FfiOption::Some(colour) = colour {
                (api.video_set_palette)(idx as u8, *colour);
            }
        }
    }
}

enum SlideError {
    Unspecified,
}

fn show_slide(
    data: &[u8],
    api: &neotron_common_bios::Api,
    buffer: &mut [u8],
) -> Result<(), SlideError> {
    use embedded_graphics::pixelcolor::RgbColor;

    let raw_bmp = tinybmp::RawBmp::from_slice(data).map_err(|_| SlideError::Unspecified)?;
    let header = raw_bmp.header();
    if header.image_size.width > 640 || header.image_size.height > 480 {
        return Err(SlideError::Unspecified);
    }

    // Program palette
    if let Some(table) = raw_bmp.color_table() {
        for entry in 0..4 {
            if let Some(rgb) = table.get(entry) {
                let rgb666 =
                    neotron_common_bios::video::RGBColour::from_rgb(rgb.r(), rgb.g(), rgb.b());
                (api.video_set_palette)(entry as u8, rgb666);
            }
        }
    }

    // Copy bitmap
    for px in raw_bmp.pixels() {
        let offset_px = (px.position.y * 640) + px.position.x;
        let offset_byte = (offset_px / 4) as usize;
        match offset_px % 4 {
            0 => {
                buffer[offset_byte] = (px.color << 6) as u8;
            }
            1 => {
                buffer[offset_byte] |= (px.color << 4) as u8;
            }
            2 => {
                buffer[offset_byte] |= (px.color << 2) as u8;
            }
            _ => {
                buffer[offset_byte] |= px.color as u8;
            }
        }
    }

    Ok(())
}

// End of file
