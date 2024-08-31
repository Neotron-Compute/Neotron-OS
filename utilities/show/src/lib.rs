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
        writeln!(stdout, "Bye!").unwrap();
    }
}

// End of file
