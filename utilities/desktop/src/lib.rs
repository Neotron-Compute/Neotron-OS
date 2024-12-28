//! Logic for the vidtest utility

#![no_std]
#![deny(missing_docs)]

use core::{cell::UnsafeCell, fmt::Write};

// Big enough for our Mode 5 (640 x 480 @ 4bpp) logo
struct RacyBuffer {
    inner: UnsafeCell<[u8; 320 * 480]>,
}

unsafe impl Sync for RacyBuffer {}

static FRAMEBUFFER: RacyBuffer = RacyBuffer {
    inner: UnsafeCell::new(*include_bytes!("desktop.raw")),
};

/// Entry point to the program
pub fn main() -> i32 {
    let mut stdout = neotron_sdk::stdout();

    let Some(mode) = neotron_sdk::VideoMode::try_from_u8(5) else {
        _ = writeln!(stdout, "Invalid mode");
        return -1;
    };

    let handle: Result<_, _> = neotron_sdk::File::open(
        neotron_sdk::path::Path::new("GFX:").unwrap(),
        neotron_sdk::Flags::WRITE,
    );
    let Ok(handle) = handle else {
        return -1;
    };
    unsafe {
        // point at our canned data
        if handle
            .ioctl(
                neotron_sdk::ioctls::gfx::COMMAND_CHANGE_MODE,
                neotron_sdk::ioctls::gfx::change_mode_value(
                    mode,
                    FRAMEBUFFER.inner.get() as *mut u32,
                ),
            )
            .is_err()
        {
            return -2;
        }
    }

    for (index, entry) in PALETTE.iter().enumerate() {
        unsafe {
            // load the palette
            if handle
                .ioctl(
                    neotron_sdk::ioctls::gfx::COMMAND_SET_PALETTE,
                    neotron_sdk::ioctls::gfx::set_palette_value(
                        index as u8,
                        entry.0,
                        entry.1,
                        entry.2,
                    ),
                )
                .is_err()
            {
                return -2;
            }
        }
    }

    wait_for_key();

    0
}

fn wait_for_key() {
    let stdin = neotron_sdk::stdin();
    let mut buffer = [0u8; 1];
    while let Ok(0) = stdin.read(&mut buffer) {
        // spin
    }
}

static PALETTE: [(u8, u8, u8); 16] = [
    (0, 0, 0),
    (136, 0, 0),
    (255, 0, 0),
    (0, 136, 0),
    (0, 255, 0),
    (136, 136, 0),
    (255, 255, 0),
    (0, 0, 136),
    (0, 0, 255),
    (0, 136, 136),
    (0, 255, 255),
    (136, 136, 136),
    (187, 187, 187),
    (255, 255, 255),
    (0, 0, 0),
    (0, 0, 0),
];

// End of file
