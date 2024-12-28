//! Logic for the vidtest utility

#![no_std]
#![deny(missing_docs)]

use core::{cell::UnsafeCell, fmt::Write};

// Big enough for Mode 5 (640x480 @ 16 colours)
struct RacyBuffer {
    inner: UnsafeCell<[u32; 640 * 480 / 8]>,
}

unsafe impl Sync for RacyBuffer {}

static FRAMEBUFFER: RacyBuffer = RacyBuffer {
    inner: UnsafeCell::new([0; 640 * 480 / 8]),
};

/// Entry point to the program
pub fn main() -> i32 {
    let mut stdout = neotron_sdk::stdout();
    let Some(new_mode) = neotron_sdk::arg(0) else {
        _ = writeln!(stdout, "Must supply a mode argument, like 7 for Mode 7");
        return -1;
    };

    let Ok(mode_num) = new_mode.parse::<u8>() else {
        _ = writeln!(stdout, "Invalid integer {:?}", new_mode);
        return -1;
    };
    let Some(mode) = neotron_sdk::VideoMode::try_from_u8(mode_num) else {
        _ = writeln!(stdout, "Invalid mode {:?}", new_mode);
        return -1;
    };

    let free_space = core::mem::size_of_val(&FRAMEBUFFER);
    if mode.frame_size_bytes() > free_space {
        _ = writeln!(
            stdout,
            "Mode requires {} bytes, we have {} bytes",
            mode.frame_size_bytes(),
            free_space
        );
        return -1;
    }

    let handle: Result<_, _> = neotron_sdk::File::open(
        neotron_sdk::path::Path::new("GFX:").unwrap(),
        neotron_sdk::Flags::WRITE,
    );
    if let Ok(handle) = handle {
        if let Err(e) = unsafe {
            handle.ioctl(
                neotron_sdk::ioctls::gfx::COMMAND_CHANGE_MODE,
                neotron_sdk::ioctls::gfx::change_mode_value(
                    mode,
                    FRAMEBUFFER.inner.get() as *mut u32,
                ),
            )
        } {
            _ = writeln!(stdout, "Mode change failure: {:?}", e);
            return -1;
        }
        if let Err(e) = grid(&handle, mode) {
            _ = writeln!(stdout, "Draw failure on grid: {:?}", e);
        }
        if let Err(e) = stripes(&handle, mode) {
            _ = writeln!(stdout, "Draw failure on stripes: {:?}", e);
        }
        if let Err(e) = radial(&handle, mode) {
            _ = writeln!(stdout, "Draw failure on radial: {:?}", e);
        }
    }

    0
}

/// plots some horizontal stripes, with all the colours
fn stripes(
    handle: &neotron_sdk::File,
    mode: neotron_sdk::VideoMode,
) -> Result<(), neotron_sdk::Error> {
    let bpp = match mode.format() {
        neotron_sdk::VideoFormat::Chunky8 => 8,
        neotron_sdk::VideoFormat::Chunky4 => 4,
        neotron_sdk::VideoFormat::Chunky2 => 2,
        neotron_sdk::VideoFormat::Chunky1 => 1,
        _ => return Err(neotron_sdk::Error::InvalidArg),
    };
    let colours = 1 << bpp;
    let height = mode.vertical_lines();
    let stripe_height = (height / (2 * colours)).max(1);
    let mut colour_iter = (0..colours).cycle();
    let mut stripe_so_far = 0;
    let mut colour = colour_iter.next().unwrap();
    for y in 0..mode.vertical_lines() {
        unsafe {
            handle.ioctl(
                neotron_sdk::ioctls::gfx::COMMAND_MOVE_CURSOR,
                neotron_sdk::ioctls::gfx::move_cursor_value(0, y),
            )
        }?;
        unsafe {
            handle.ioctl(
                neotron_sdk::ioctls::gfx::COMMAND_DRAW_LINE,
                neotron_sdk::ioctls::gfx::draw_line_value(
                    mode.horizontal_pixels() - 1,
                    y,
                    colour as u32,
                ),
            )
        }?;
        stripe_so_far += 1;
        if stripe_so_far == stripe_height {
            stripe_so_far = 0;
            colour = colour_iter.next().unwrap();
        }
    }

    wait_for_key();

    Ok(())
}

/// plots a grid
fn grid(
    handle: &neotron_sdk::File,
    mode: neotron_sdk::VideoMode,
) -> Result<(), neotron_sdk::Error> {
    unsafe { handle.ioctl(neotron_sdk::ioctls::gfx::COMMAND_CLEAR_SCREEN, 0) }?;

    let width = mode.horizontal_pixels() / 10;
    let height = mode.vertical_lines() / 10;
    let y_max = mode.vertical_lines() - 1;
    let x_max = mode.horizontal_pixels() - 1;
    let colour = 15;

    // Horizontal
    for y in (0..mode.vertical_lines())
        .filter(|&y| (y % height) == 0 || (y == mode.vertical_lines() - 1))
    {
        unsafe {
            handle.ioctl(
                neotron_sdk::ioctls::gfx::COMMAND_MOVE_CURSOR,
                neotron_sdk::ioctls::gfx::move_cursor_value(0, y),
            )
        }?;
        unsafe {
            handle.ioctl(
                neotron_sdk::ioctls::gfx::COMMAND_DRAW_LINE,
                neotron_sdk::ioctls::gfx::draw_line_value(x_max, y, colour),
            )
        }?;
    }

    // Vertical
    for x in (0..mode.horizontal_pixels())
        .filter(|&x| (x % width) == 0 || (x == mode.horizontal_pixels() - 1))
    {
        unsafe {
            handle.ioctl(
                neotron_sdk::ioctls::gfx::COMMAND_MOVE_CURSOR,
                neotron_sdk::ioctls::gfx::move_cursor_value(x, 0),
            )
        }?;
        unsafe {
            handle.ioctl(
                neotron_sdk::ioctls::gfx::COMMAND_DRAW_LINE,
                neotron_sdk::ioctls::gfx::draw_line_value(x, y_max, colour),
            )
        }?;
    }

    wait_for_key();

    Ok(())
}

/// plots a radial pattern
fn radial(
    handle: &neotron_sdk::File,
    mode: neotron_sdk::VideoMode,
) -> Result<(), neotron_sdk::Error> {
    unsafe { handle.ioctl(neotron_sdk::ioctls::gfx::COMMAND_CLEAR_SCREEN, 0) }?;

    let mut colour = 1;

    for x in (0..mode.horizontal_pixels()).step_by(16) {
        // plot from 0,0 to the bottom edge
        unsafe {
            handle.ioctl(
                neotron_sdk::ioctls::gfx::COMMAND_MOVE_CURSOR,
                neotron_sdk::ioctls::gfx::move_cursor_value(0, 0),
            )
        }?;
        unsafe {
            handle.ioctl(
                neotron_sdk::ioctls::gfx::COMMAND_DRAW_LINE,
                neotron_sdk::ioctls::gfx::draw_line_value(x, mode.vertical_lines() - 1, colour),
            )
        }?;
        colour += 1;
    }

    // do the diagonal
    unsafe {
        handle.ioctl(
            neotron_sdk::ioctls::gfx::COMMAND_MOVE_CURSOR,
            neotron_sdk::ioctls::gfx::move_cursor_value(0, 0),
        )
    }?;
    unsafe {
        handle.ioctl(
            neotron_sdk::ioctls::gfx::COMMAND_DRAW_LINE,
            neotron_sdk::ioctls::gfx::draw_line_value(
                mode.horizontal_pixels() - 1,
                mode.vertical_lines() - 1,
                colour,
            ),
        )
    }?;
    colour += 1;

    for y in (0..mode.vertical_lines()).step_by(16).rev() {
        // plot from 0,0 to the right hand edge
        unsafe {
            handle.ioctl(
                neotron_sdk::ioctls::gfx::COMMAND_MOVE_CURSOR,
                neotron_sdk::ioctls::gfx::move_cursor_value(0, 0),
            )
        }?;
        unsafe {
            handle.ioctl(
                neotron_sdk::ioctls::gfx::COMMAND_DRAW_LINE,
                neotron_sdk::ioctls::gfx::draw_line_value(mode.horizontal_pixels() - 1, y, colour),
            )
        }?;
        colour += 1;
    }

    wait_for_key();

    Ok(())
}

fn wait_for_key() {
    let stdin = neotron_sdk::stdin();
    let mut buffer = [0u8; 1];
    while let Ok(0) = stdin.read(&mut buffer) {
        // spin
    }
}

// End of file
