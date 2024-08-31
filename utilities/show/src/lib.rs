//! Main content for the bitmap show demo

#![no_std]
#![deny(missing_docs)]
#![deny(unsafe_code)]

use core::fmt::Write;

/// Represents the bitmap show application
pub struct App {}

impl App {
    /// Make a new bitmap show application.
    pub const fn new() -> App {
        App {}
    }

    /// Run the flames demo
    pub fn play(&mut self) {
        let mut stdout = neotron_sdk::stdout();
        let Some(bitmap_name) = neotron_sdk::arg(0) else {
            writeln!(stdout, "Need a bitmap name!").unwrap();
            return;
        };
        writeln!(stdout, "Loading {}", bitmap_name).unwrap();

        // 1. open `GFX$mode=5:` - the OS will allocate framebuffer from the TPA
        // 2. open the given file
        // 3. load the given file into our static buffer
        //    (or we can malloc the right amount of memory from the OS)
        // 4. parse the bitmap and transfer the pixels into the framebuffer
        //    by writing bytes to the GFX$ file we opened, seeking as required.
        // 5. close the file
        // 6. wait for a keypress
        // 7. close the GFX$ file - the OS will release the framebuffer

        // Bonus features - load the file first and work out what framebuffer
        // mode we need to display it, instead of always using Mode 5 (which
        // doesn't work on Neotron Pico with RP2040).

        // OS features:
        //
        // - need to be able to open GFX$ with a mode, and need
        //   to have it allocate out of the TPA (and release on close).
        // - need to be able to malloc out of the TPA and have any open
        //   allocations cleaned up on program exit (free'ing memory is a nice
        //   to have...)
        //
    }
}

// End of file
