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
        if let Err(e) = vertical(&handle, mode) {
            _ = writeln!(stdout, "Draw failure on vertical: {:?}", e);
        }
        if let Err(e) = horizontal(&handle, mode) {
            _ = writeln!(stdout, "Draw failure on horizontal: {:?}", e);
        }
        if let Err(e) = rolling(&handle, mode) {
            _ = writeln!(stdout, "Draw failure on rolling: {:?}", e);
        }
        if let Err(e) = grid(&handle, mode) {
            _ = writeln!(stdout, "Draw failure on grid: {:?}", e);
        }
    }

    0
}

/// plots a vertical colour stripe pattern.
fn vertical(
    handle: &neotron_sdk::File,
    mode: neotron_sdk::VideoMode,
) -> Result<(), neotron_sdk::Error> {
    for y in 0..mode.vertical_lines() {
        let mut colour = 0u32;
        for x in 0..mode.horizontal_pixels() {
            unsafe {
                handle.ioctl(
                    neotron_sdk::ioctls::gfx::COMMAND_CHUNKY_PLOT,
                    neotron_sdk::ioctls::gfx::chunky_plot_value(x, y, colour),
                )?;
            }

            colour = colour.wrapping_add(1) & 0xFFFFFF;
        }
    }

    wait_for_key();

    Ok(())
}

/// plots a horizontal colour stripe pattern.
fn horizontal(
    handle: &neotron_sdk::File,
    mode: neotron_sdk::VideoMode,
) -> Result<(), neotron_sdk::Error> {
    let mut colour = 0u32;
    for y in 0..mode.vertical_lines() {
        for x in 0..mode.horizontal_pixels() {
            unsafe {
                handle.ioctl(
                    neotron_sdk::ioctls::gfx::COMMAND_CHUNKY_PLOT,
                    neotron_sdk::ioctls::gfx::chunky_plot_value(x, y, colour),
                )?;
            }
        }
        colour = colour.wrapping_add(1) & 0xFFFFFF;
    }

    wait_for_key();

    Ok(())
}

/// plots a rolling stripe pattern.
fn rolling(
    handle: &neotron_sdk::File,
    mode: neotron_sdk::VideoMode,
) -> Result<(), neotron_sdk::Error> {
    for y in 0..mode.vertical_lines() {
        let mut colour = y as u32;
        for x in 0..mode.horizontal_pixels() {
            unsafe {
                handle.ioctl(
                    neotron_sdk::ioctls::gfx::COMMAND_CHUNKY_PLOT,
                    neotron_sdk::ioctls::gfx::chunky_plot_value(x, y, colour),
                )?;
            }
            colour = colour.wrapping_add(1) & 0xFFFFFF;
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

    for y in 0..mode.vertical_lines() {
        if (y % height) == 0 || (y == mode.vertical_lines() - 1) {
            // solid line
            for x in 0..mode.horizontal_pixels() {
                unsafe {
                    handle.ioctl(
                        neotron_sdk::ioctls::gfx::COMMAND_CHUNKY_PLOT,
                        neotron_sdk::ioctls::gfx::chunky_plot_value(x, y, 15),
                    )?;
                }
            }
        } else {
            // stripes
            for x in 0..mode.horizontal_pixels() {
                if (x % width) == 0 || (x == mode.horizontal_pixels() - 1) {
                    unsafe {
                        handle.ioctl(
                            neotron_sdk::ioctls::gfx::COMMAND_CHUNKY_PLOT,
                            neotron_sdk::ioctls::gfx::chunky_plot_value(x, y, 15),
                        )?;
                    }
                }
            }
        }
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
