//! Game logic for the flames demo

#![no_std]
#![deny(missing_docs)]
#![deny(unsafe_code)]

use core::fmt::Write;

/// Represents the Flames application
pub struct App {
    seed: usize,
    width: usize,
    height: usize,
    buffer: [u8; Self::FLAME_BUFFER_LEN],
    stdout: neotron_sdk::File,
    stdin: neotron_sdk::File,
    bold: bool,
    colour: u8,
}

impl App {
    const MAX_WIDTH: usize = 80;
    const MAX_HEIGHT: usize = 60;
    const SIZE: usize = Self::MAX_WIDTH * Self::MAX_HEIGHT;
    const FLAME_BUFFER_LEN: usize = Self::SIZE + Self::MAX_WIDTH + 1;

    /// Make a new flames application.
    ///
    /// You can give the screen size in characters.
    pub const fn new(width: u8, height: u8) -> App {
        let width = if width as usize > Self::MAX_WIDTH {
            Self::MAX_WIDTH
        } else {
            width as usize
        };
        let height = if height as usize > Self::MAX_HEIGHT {
            Self::MAX_HEIGHT
        } else {
            height as usize
        };
        App {
            seed: 123456789,
            width,
            height,
            buffer: [0u8; Self::FLAME_BUFFER_LEN],
            stdout: neotron_sdk::stdout(),
            stdin: neotron_sdk::stdin(),
            bold: false,
            colour: 37,
        }
    }

    /// Run the flames demo
    pub fn play(&mut self) {
        neotron_sdk::console::cursor_off(&mut self.stdout);
        neotron_sdk::console::clear_screen(&mut self.stdout);
        loop {
            self.draw_fire();
            let mut buffer = [0u8; 1];
            if let Ok(1) = self.stdin.read(&mut buffer) {
                break;
            }
            neotron_sdk::delay(core::time::Duration::from_millis(17));
        }
        writeln!(self.stdout, "Bye!").unwrap();
        neotron_sdk::console::cursor_on(&mut self.stdout);
    }

    /// Draws a flame effect.
    /// Based on https://gist.github.com/msimpson/1096950.
    fn draw_fire(&mut self) {
        const CHARS: [char; 10] = [' ', '`', ':', '^', '*', 'x', '░', '▒', '▓', '█'];
        const COLOURS: [(bool, u8); 16] = [
            (true, 37),
            (true, 37),
            (true, 37),
            (true, 37),
            (true, 37),
            (true, 33),
            (true, 33),
            (true, 33),
            (true, 33),
            (true, 33),
            (true, 31),
            (true, 31),
            (true, 31),
            (true, 31),
            (true, 31),
            (false, 35),
        ];
        neotron_sdk::console::move_cursor(
            &mut self.stdout,
            neotron_sdk::console::Position::origin(),
        );
        // Seed the fire just off-screen
        for _i in 0..5 {
            let idx = (self.width * self.height) + self.random_up_to(self.width - 1);
            self.buffer[idx] = 100;
        }
        // Cascade the flames
        for idx in 0..(self.width * (self.height + 1)) {
            self.buffer[idx] = ((u32::from(self.buffer[idx])
                + u32::from(self.buffer[idx + 1])
                + u32::from(self.buffer[idx + self.width])
                + u32::from(self.buffer[idx + self.width + 1]))
                / 4) as u8;
            let glyph = CHARS
                .get(self.buffer[idx] as usize)
                .unwrap_or(CHARS.last().unwrap());
            let colour = COLOURS
                .get(self.buffer[idx] as usize)
                .unwrap_or(COLOURS.last().unwrap());
            // Only draw what is on screen
            if idx < (self.width * self.height) {
                self.set_colour(colour.0, colour.1);
                write!(self.stdout, "{}", glyph).unwrap();
            }
        }
    }

    /// Set the colour of any future text
    fn set_colour(&mut self, bold: bool, colour: u8) {
        if self.bold != bold {
            self.bold = bold;
            if bold {
                write!(self.stdout, "\u{001b}[1m").unwrap();
            } else {
                write!(self.stdout, "\u{001b}[22m").unwrap();
            }
        }
        if self.colour != colour {
            self.colour = colour;
            write!(self.stdout, "\u{001b}[{}m", colour).unwrap();
        }
    }

    /// Generates a number in the range [0, limit)
    fn random_up_to(&mut self, limit: usize) -> usize {
        let buckets = ::core::usize::MAX / limit;
        let upper_edge = buckets * limit;
        loop {
            let attempt = self.random();
            if attempt < upper_edge {
                return attempt / buckets;
            }
        }
    }

    /// Generate a random 32-bit number
    fn random(&mut self) -> usize {
        self.seed = (self.seed.wrapping_mul(1103515245)).wrapping_add(12345);
        self.seed
    }
}

// End of file
