//! # VGA Console
//!
//! Code for dealing with a VGA-style console, where there's a buffer of 16-bit
//! values, each corresponding to a glyph and some attributes.
//!
//! The input to this sub-system is a stream of bytes, which should be valid
//! UTF-8 and may contain ANSI escape sequences.
//!
//! The ANSI sequences are parsed using Alacritty's [vte
//! crate](https://crates.io/crates/vte). We then interpret the 'action' codes
//! as best we can. We also interpret commands like `\n` as a New Line (move the
//! cursor down one line), and `\r` as Carriage Return (move the cursor back to
//! Column 0). Note that we use 0-based row and column numbers internally, but
//! we understand ANSI sequences that use 1-based indidices.
//!
//! You can write to the VGA console because `core::fmt::Write` is implemented
//! for `VgaConsole`. Any text sent this way will be sent through the ANSI
//! decoder. Anything that's not an ANSI sequence will be converted to Code Page
//! 850 and then added to the 2D array of glyphs and attributes that is our text
//! buffer. We then assume that some other code somewhere else will take these
//! values and put them on a video screen somehow.

// ===========================================================================
// Modules and Imports
// ===========================================================================

use neotron_common_bios::video::{Attr, TextBackgroundColour, TextForegroundColour};

// ===========================================================================
// Global Variables
// ===========================================================================

// ===========================================================================
// Macros
// ===========================================================================

// ===========================================================================
// Public types
// ===========================================================================

/// Represents our simulation of a DEC-like ANSI video terminal.
pub struct VgaConsole {
    inner: ConsoleInner,
    parser: vte::Parser<16>,
}

impl VgaConsole {
    /// White on Black
    const DEFAULT_ATTR: Attr = Attr::new(
        TextForegroundColour::LIGHT_GRAY,
        TextBackgroundColour::BLACK,
        false,
    );

    pub fn new(addr: *mut u8, width: isize, height: isize) -> VgaConsole {
        VgaConsole {
            inner: ConsoleInner {
                addr,
                width,
                height,
                row: 0,
                col: 0,
                attr: Self::DEFAULT_ATTR,
                bright: false,
                reverse: false,
                cursor_wanted: false,
                cursor_holder: None,
                cursor_depth: 0,
            },
            parser: vte::Parser::new_with_size(),
        }
    }

    /// Change the video mode
    ///
    /// Non text modes are ignored.
    pub fn change_mode(&mut self, mode: neotron_common_bios::video::Mode) {
        if let (Some(height), Some(width)) = (mode.text_height(), mode.text_width()) {
            self.inner.height = height as isize;
            self.inner.width = width as isize;
            self.clear();
        }
    }

    /// Clear the screen.
    ///
    /// Every character on the screen is replaced with an space (U+0020).
    pub fn clear(&mut self) {
        self.inner.cursor_disable();
        self.inner.clear();
        self.inner.cursor_enable();
    }

    /// Set the default attribute for any future text.
    pub fn set_attr(&mut self, attr: Attr) {
        self.inner.set_attr(attr);
    }

    /// Write a UTF-8 byte string to the console.
    ///
    /// Is parsed for ANSI codes, and Unicode is converted to Code Page 850 for
    /// display on the VGA screen.
    pub fn write_bstr(&mut self, bstr: &[u8]) {
        self.inner.cursor_disable();
        for b in bstr {
            self.parser.advance(&mut self.inner, *b);
        }
        self.inner.cursor_enable();
    }
}

// ===========================================================================
// Private types
// ===========================================================================

/// Handles the inner details of where we are on screen.
///
/// Separate from the parser, so it can be passed to the `advance` method.
struct ConsoleInner {
    addr: *mut u8,
    width: isize,
    height: isize,
    row: isize,
    col: isize,
    attr: Attr,
    bright: bool,
    reverse: bool,
    cursor_wanted: bool,
    cursor_depth: u8,
    cursor_holder: Option<u8>,
}

impl ConsoleInner {
    const DEFAULT_ATTR: Attr = Attr::new(
        TextForegroundColour::LIGHT_GRAY,
        TextBackgroundColour::BLACK,
        false,
    );

    /// Replace the glyph at the current location with a cursor.
    fn cursor_enable(&mut self) {
        self.cursor_depth -= 1;
        if self.cursor_depth == 0 && self.cursor_wanted && self.cursor_holder.is_none() {
            // Remember what was where our cursor is (unless the cursor is off-screen, when we make something up)
            if self.row >= 0 && self.row < self.height && self.col >= 0 && self.col < self.width {
                let value = self.read();
                self.write_at(self.row, self.col, b'_', true);
                self.cursor_holder = Some(value);
            } else {
                self.cursor_holder = Some(b' ');
            }
        }
    }

    /// Replace the cursor at the current location with its previous contents.
    fn cursor_disable(&mut self) {
        if let Some(glyph) = self.cursor_holder.take() {
            if self.row >= 0 && self.row < self.height && self.col >= 0 && self.col < self.width {
                // cursor was on-screen, so restore it
                self.write(glyph);
            }
        }
        self.cursor_depth += 1;
    }

    /// Move the cursor relative to the current location.
    ///
    /// Clamps to the visible screen.
    fn move_cursor_relative(&mut self, rows: isize, cols: isize) {
        self.row += rows;
        self.col += cols;
        if self.row < 0 {
            self.row = 0;
        }
        if self.col < 0 {
            self.col = 0;
        }
        if self.row >= self.height {
            self.row = self.height - 1;
        }
        if self.col >= self.width {
            self.col = self.width - 1;
        }
    }

    /// Move the cursor to the given location.
    ///
    /// Clamps to the visible screen.
    fn move_cursor_absolute(&mut self, rows: isize, cols: isize) {
        // move it
        self.row = rows;
        self.col = cols;
        // clamp it
        self.move_cursor_relative(0, 0);
    }

    /// Move the cursor to 0,0
    fn home(&mut self) {
        self.move_cursor_absolute(0, 0);
    }

    /// If we are currently positioned off-screen, scroll and fix that.
    ///
    /// We defer this so you can write the last char on the last line without
    /// causing it to scroll pre-emptively.
    fn scroll_as_required(&mut self) {
        assert!(self.row <= self.height);
        if self.col >= self.width {
            self.col = 0;
            self.row += 1;
        }
        if self.row == self.height {
            self.row -= 1;
            self.scroll_page();
        }
    }

    /// Blank the screen
    fn clear(&mut self) {
        for row in 0..self.height {
            for col in 0..self.width {
                self.write_at(row, col, b' ', false);
            }
        }
        self.home();
    }

    /// Set the default attribute for any future text.
    fn set_attr(&mut self, attr: Attr) {
        self.attr = attr;
    }

    /// Put a glyph at the current position on the screen.
    ///
    /// Don't do this if the cursor is enabled.
    fn write(&mut self, glyph: u8) {
        self.write_at(self.row, self.col, glyph, false);
    }

    /// Put a glyph at a given position on the screen.
    ///
    /// Don't do this if the cursor is enabled.
    fn write_at(&mut self, row: isize, col: isize, glyph: u8, is_cursor: bool) {
        assert!(row < self.height, "{} >= {}?", row, self.height);
        assert!(col < self.width, "{} => {}?", col, self.width);
        if !crate::IS_PANIC.load(core::sync::atomic::Ordering::Relaxed) && !is_cursor {
            assert!(self.cursor_holder.is_none());
        }

        let offset = ((row * self.width) + col) * 2;
        unsafe { core::ptr::write_volatile(self.addr.offset(offset), glyph) };
        let attr = if self.reverse {
            let new_fg = self.attr.bg().as_u8();
            let new_bg = self.attr.fg().as_u8();
            Attr::new(
                unsafe { TextForegroundColour::new_unchecked(new_fg) },
                unsafe { TextBackgroundColour::new_unchecked(new_bg & 0x07) },
                false,
            )
        } else {
            self.attr
        };

        unsafe { core::ptr::write_volatile(self.addr.offset(offset + 1), attr.as_u8()) };
    }

    /// Read a glyph at the current position
    ///
    /// Don't do this if the cursor is enabled.
    fn read(&mut self) -> u8 {
        self.read_at(self.row, self.col)
    }

    /// Read a glyph at the given position
    ///
    /// Don't do this if the cursor is enabled.
    fn read_at(&mut self, row: isize, col: isize) -> u8 {
        assert!(row < self.height, "{} >= {}?", row, self.height);
        assert!(col < self.width, "{} => {}?", col, self.width);
        if !crate::IS_PANIC.load(core::sync::atomic::Ordering::Relaxed) {
            assert!(self.cursor_holder.is_none());
        }
        let offset = ((row * self.width) + col) * 2;
        unsafe { core::ptr::read_volatile(self.addr.offset(offset)) }
    }

    /// Move everyone on screen up one line, losing the top line.
    ///
    /// The bottom line will be all space characters.
    fn scroll_page(&mut self) {
        let row_len_bytes = self.width * 2;
        unsafe {
            // Scroll rows[1..=height-1] to become rows[0..=height-2].
            core::ptr::copy(
                self.addr.offset(row_len_bytes),
                self.addr,
                (row_len_bytes * (self.height - 1)) as usize,
            );
        }
        // Blank the bottom line of the screen (rows[height-1]).
        for col in 0..self.width {
            self.write_at(self.height - 1, col, b' ', false);
        }
    }

    /// Convert a Unicode Scalar Value to a font glyph.
    ///
    /// Zero-width and modifier Unicode Scalar Values (e.g. `U+0301 COMBINING,
    /// ACCENT`) are not supported. Normalise your Unicode before calling
    /// this function.
    fn map_char_to_glyph(input: char) -> u8 {
        // This fixed table only works for the default font. When we support
        // changing font, we will need to plug-in a different table for each font.
        match input {
            '\u{0020}'..='\u{007E}' => input as u8,
            // 0x80 to 0x9F are the C1 control codes with no visual
            // representation
            '\u{00A0}' => 255, // NBSP
            '\u{00A1}' => 173, // ¡
            '\u{00A2}' => 189, // ¢
            '\u{00A3}' => 156, // £
            '\u{00A4}' => 207, // ¤
            '\u{00A5}' => 190, // ¥
            '\u{00A6}' => 221, // ¦
            '\u{00A7}' => 245, // §
            '\u{00A8}' => 249, // ¨
            '\u{00A9}' => 184, // ©
            '\u{00AA}' => 166, // ª
            '\u{00AB}' => 174, // «
            '\u{00AC}' => 170, // ¬
            '\u{00AD}' => 240, // - (Soft Hyphen)
            '\u{00AE}' => 169, // ®
            '\u{00AF}' => 238, // ¯
            '\u{00B0}' => 248, // °
            '\u{00B1}' => 241, // ±
            '\u{00B2}' => 253, // ²
            '\u{00B3}' => 252, // ³
            '\u{00B4}' => 239, // ´
            '\u{00B5}' => 230, // µ
            '\u{00B6}' => 244, // ¶
            '\u{00B7}' => 250, // ·
            '\u{00B8}' => 247, // ¸
            '\u{00B9}' => 251, // ¹
            '\u{00BA}' => 167, // º
            '\u{00BB}' => 175, // »
            '\u{00BC}' => 172, // ¼
            '\u{00BD}' => 171, // ½
            '\u{00BE}' => 243, // ¾
            '\u{00BF}' => 168, // ¿
            '\u{00C0}' => 183, // À
            '\u{00C1}' => 181, // Á
            '\u{00C2}' => 182, // Â
            '\u{00C3}' => 199, // Ã
            '\u{00C4}' => 142, // Ä
            '\u{00C5}' => 143, // Å
            '\u{00C6}' => 146, // Æ
            '\u{00C7}' => 128, // Ç
            '\u{00C8}' => 212, // È
            '\u{00C9}' => 144, // É
            '\u{00CA}' => 210, // Ê
            '\u{00CB}' => 211, // Ë
            '\u{00CC}' => 222, // Ì
            '\u{00CD}' => 214, // Í
            '\u{00CE}' => 215, // Î
            '\u{00CF}' => 216, // Ï
            '\u{00D0}' => 209, // Ð
            '\u{00D1}' => 165, // Ñ
            '\u{00D2}' => 227, // Ò
            '\u{00D3}' => 224, // Ó
            '\u{00D4}' => 226, // Ô
            '\u{00D5}' => 229, // Õ
            '\u{00D6}' => 153, // Ö
            '\u{00D7}' => 158, // ×
            '\u{00D8}' => 157, // Ø
            '\u{00D9}' => 235, // Ù
            '\u{00DA}' => 233, // Ú
            '\u{00DB}' => 234, // Û
            '\u{00DC}' => 154, // Ü
            '\u{00DD}' => 237, // Ý
            '\u{00DE}' => 232, // Þ
            '\u{00DF}' => 225, // ß
            '\u{00E0}' => 133, // à
            '\u{00E1}' => 160, // á
            '\u{00E2}' => 131, // â
            '\u{00E3}' => 198, // ã
            '\u{00E4}' => 132, // ä
            '\u{00E5}' => 134, // å
            '\u{00E6}' => 145, // æ
            '\u{00E7}' => 135, // ç
            '\u{00E8}' => 138, // è
            '\u{00E9}' => 130, // é
            '\u{00EA}' => 136, // ê
            '\u{00EB}' => 137, // ë
            '\u{00EC}' => 141, // ì
            '\u{00ED}' => 161, // í
            '\u{00EE}' => 140, // î
            '\u{00EF}' => 139, // ï
            '\u{00F0}' => 208, // ð
            '\u{00F1}' => 164, // ñ
            '\u{00F2}' => 149, // ò
            '\u{00F3}' => 162, // ó
            '\u{00F4}' => 147, // ô
            '\u{00F5}' => 228, // õ
            '\u{00F6}' => 148, // ö
            '\u{00F7}' => 246, // ÷
            '\u{00F8}' => 155, // ø
            '\u{00F9}' => 151, // ù
            '\u{00FA}' => 163, // ú
            '\u{00FB}' => 150, // û
            '\u{00FC}' => 129, // ü
            '\u{00FD}' => 236, // ý
            '\u{00FE}' => 231, // þ
            '\u{00FF}' => 152, // ÿ
            '\u{0131}' => 213, // ı
            '\u{0192}' => 159, // ƒ
            '\u{2017}' => 242, // ‗
            '\u{2022}' => 7,   // •
            '\u{203C}' => 19,  // ‼
            '\u{2190}' => 27,  // ←
            '\u{2191}' => 24,  // ↑
            '\u{2192}' => 26,  // →
            '\u{2193}' => 25,  // ↓
            '\u{2194}' => 29,  // ↔
            '\u{2195}' => 18,  // ↕
            '\u{21A8}' => 23,  // ↨
            '\u{221F}' => 28,  // ∟
            '\u{2302}' => 127, // ⌂
            '\u{2500}' => 196, // ─
            '\u{2502}' => 179, // │
            '\u{250C}' => 218, // ┌
            '\u{2510}' => 191, // ┐
            '\u{2514}' => 192, // └
            '\u{2518}' => 217, // ┘
            '\u{251C}' => 195, // ├
            '\u{2524}' => 180, // ┤
            '\u{252C}' => 194, // ┬
            '\u{2534}' => 193, // ┴
            '\u{253C}' => 197, // ┼
            '\u{2550}' => 205, // ═
            '\u{2551}' => 186, // ║
            '\u{2554}' => 201, // ╔
            '\u{2557}' => 187, // ╗
            '\u{255A}' => 200, // ╚
            '\u{255D}' => 188, // ╝
            '\u{2560}' => 204, // ╠
            '\u{2563}' => 185, // ╣
            '\u{2566}' => 203, // ╦
            '\u{2569}' => 202, // ╩
            '\u{256C}' => 206, // ╬
            '\u{2580}' => 223, // ▀
            '\u{2584}' => 220, // ▄
            '\u{2588}' => 219, // █
            '\u{2591}' => 176, // ░
            '\u{2592}' => 177, // ▒
            '\u{2593}' => 178, // ▓
            '\u{25A0}' => 254, // ■
            '\u{25AC}' => 22,  // ▬
            '\u{25B2}' => 30,  // ▲
            '\u{25BA}' => 16,  // ►
            '\u{25BC}' => 31,  // ▼
            '\u{25C4}' => 17,  // ◄
            '\u{25CB}' => 9,   // ○
            '\u{25D8}' => 8,   // ◘
            '\u{25D9}' => 10,  // ◙
            '\u{263A}' => 1,   // ☺
            '\u{263B}' => 2,   // ☻
            '\u{263C}' => 15,  // ☼
            '\u{2640}' => 12,  // ♀
            '\u{2642}' => 11,  // ♂
            '\u{2660}' => 6,   // ♠
            '\u{2663}' => 5,   // ♣
            '\u{2665}' => 3,   // ♥
            '\u{2666}' => 4,   // ♦
            '\u{266A}' => 13,  // ♪
            '\u{266B}' => 14,  // ♫
            _ => b'?',
        }
    }
}

impl core::fmt::Write for VgaConsole {
    /// Write a UTF-8 string slice to the console.
    ///
    /// Is parsed for ANSI codes, and Unicode is converted to Code Page 850 for
    /// display on the VGA screen.
    fn write_str(&mut self, data: &str) -> core::fmt::Result {
        self.inner.cursor_disable();
        assert!(self.inner.cursor_holder.is_none());
        for b in data.bytes() {
            self.parser.advance(&mut self.inner, b);
        }
        self.inner.cursor_enable();
        Ok(())
    }
}

impl vte::Perform for ConsoleInner {
    /// Draw a character to the screen and update states.
    fn print(&mut self, ch: char) {
        self.scroll_as_required();
        self.write(Self::map_char_to_glyph(ch));
        self.col += 1;
    }

    /// Execute a C0 or C1 control function.
    fn execute(&mut self, byte: u8) {
        self.scroll_as_required();
        match byte {
            0x08 => {
                // This is a backspace, so we go back one character (if we
                // can). We expect the caller to provide "\u{0008} \u{0008}"
                // to actually erase the char then move the cursor over it.
                if self.col > 0 {
                    self.col -= 1;
                }
            }
            b'\r' => {
                self.col = 0;
            }
            b'\n' => {
                self.col = 0;
                self.row += 1;
            }
            _ => {
                // ignore unknown C0 or C1 control code
            }
        }
        // We may now be off-screen, but that's OK because we will scroll before
        // we print the next thing.
    }

    /// A final character has arrived for a CSI sequence
    ///
    /// The `ignore` flag indicates that either more than two intermediates arrived
    /// or the number of parameters exceeded the maximum supported length,
    /// and subsequent characters were ignored.
    fn csi_dispatch(
        &mut self,
        params: &vte::Params,
        intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        // Just in case you want a single parameter, here it is
        let mut first = *params.iter().next().and_then(|s| s.first()).unwrap_or(&1) as isize;
        let mut second = *params.iter().nth(1).and_then(|s| s.first()).unwrap_or(&1) as isize;

        match action {
            'm' => {
                // Select Graphic Rendition
                for p in params.iter() {
                    let Some(p) = p.first() else {
                        // Can't handle sub-params, i.e. params with more than one value
                        return;
                    };
                    match *p {
                        0 => {
                            // Reset, or normal
                            self.attr = Self::DEFAULT_ATTR;
                            self.bright = false;
                            self.reverse = false;
                        }
                        1 => {
                            // Bold intensity
                            self.bright = true;
                        }
                        7 => {
                            // Reverse video
                            self.reverse = true;
                        }
                        22 => {
                            // Normal intensity
                            self.bright = false;
                        }
                        // Foreground
                        30 => {
                            self.attr.set_fg(TextForegroundColour::BLACK);
                        }
                        31 => {
                            self.attr.set_fg(TextForegroundColour::RED);
                        }
                        32 => {
                            self.attr.set_fg(TextForegroundColour::GREEN);
                        }
                        33 => {
                            self.attr.set_fg(TextForegroundColour::BROWN);
                        }
                        34 => {
                            self.attr.set_fg(TextForegroundColour::BLUE);
                        }
                        35 => {
                            self.attr.set_fg(TextForegroundColour::MAGENTA);
                        }
                        36 => {
                            self.attr.set_fg(TextForegroundColour::CYAN);
                        }
                        37 | 39 => {
                            self.attr.set_fg(TextForegroundColour::LIGHT_GRAY);
                        }
                        // Background
                        40 => {
                            self.attr.set_bg(TextBackgroundColour::BLACK);
                        }
                        41 => {
                            self.attr.set_bg(TextBackgroundColour::RED);
                        }
                        42 => {
                            self.attr.set_bg(TextBackgroundColour::GREEN);
                        }
                        43 => {
                            self.attr.set_bg(TextBackgroundColour::BROWN);
                        }
                        44 => {
                            self.attr.set_bg(TextBackgroundColour::BLUE);
                        }
                        45 => {
                            self.attr.set_bg(TextBackgroundColour::MAGENTA);
                        }
                        46 => {
                            self.attr.set_bg(TextBackgroundColour::CYAN);
                        }
                        47 | 49 => {
                            self.attr.set_bg(TextBackgroundColour::LIGHT_GRAY);
                        }
                        _ => {
                            // Ignore unknown code
                        }
                    }
                }
                // Now check if we're bright, and make it brighter. We do this
                // last, because they might set the colour first and set the
                // bright bit afterwards.
                if self.bright {
                    match self.attr.fg() {
                        TextForegroundColour::BLACK => {
                            self.attr.set_fg(TextForegroundColour::DARK_GRAY);
                        }
                        TextForegroundColour::RED => {
                            self.attr.set_fg(TextForegroundColour::LIGHT_RED);
                        }
                        TextForegroundColour::GREEN => {
                            self.attr.set_fg(TextForegroundColour::LIGHT_GREEN);
                        }
                        TextForegroundColour::BROWN => {
                            self.attr.set_fg(TextForegroundColour::YELLOW);
                        }
                        TextForegroundColour::BLUE => {
                            self.attr.set_fg(TextForegroundColour::LIGHT_BLUE);
                        }
                        TextForegroundColour::MAGENTA => {
                            self.attr.set_fg(TextForegroundColour::PINK);
                        }
                        TextForegroundColour::CYAN => {
                            self.attr.set_fg(TextForegroundColour::LIGHT_CYAN);
                        }
                        TextForegroundColour::LIGHT_GRAY => {
                            self.attr.set_fg(TextForegroundColour::WHITE);
                        }
                        _ => {
                            // Do nothing
                        }
                    }
                }
            }
            'A' => {
                // Cursor Up
                if first == 0 {
                    first = 1;
                }
                self.move_cursor_relative(-first, 0);
            }
            'B' => {
                // Cursor Down
                if first == 0 {
                    first = 1;
                }
                self.move_cursor_relative(first, 0);
            }
            'C' => {
                // Cursor Forward
                if first == 0 {
                    first = 1;
                }
                self.move_cursor_relative(0, first);
            }
            'D' => {
                // Cursor Back
                if first == 0 {
                    first = 1;
                }
                self.move_cursor_relative(0, -first);
            }
            'E' => {
                // Cursor next line
                if first == 0 {
                    first = 1;
                }
                self.move_cursor_absolute(self.row + first, 0);
            }
            'F' => {
                // Cursor previous line
                if first == 0 {
                    first = 1;
                }
                self.move_cursor_absolute(self.row - first, 0);
            }
            'G' => {
                // Cursor horizontal absolute
                if first == 0 {
                    first = 1;
                }
                // We are zero-indexed, ANSI is 1-indexed
                self.move_cursor_absolute(self.row, first - 1);
            }
            'H' | 'f' => {
                // Cursor Position (or Horizontal Vertical Position)
                if first == 0 {
                    first = 1;
                }
                if second == 0 {
                    second = 1;
                }
                // We are zero-indexed, ANSI is 1-indexed
                self.move_cursor_absolute(first - 1, second - 1);
            }
            'J' => {
                // Erase in Display
                match first {
                    0 => {
                        // Erase the cursor through the end of the display
                        for row in 0..self.height {
                            for col in 0..self.width {
                                if row > self.row || (row == self.row && col >= self.col) {
                                    self.write_at(row, col, b' ', false);
                                }
                            }
                        }
                    }
                    1 => {
                        // Erase from the beginning of the display through the cursor
                        for row in 0..self.height {
                            for col in 0..self.width {
                                if row < self.row || (row == self.row && col <= self.col) {
                                    self.write_at(row, col, b' ', false);
                                }
                            }
                        }
                    }
                    2 => {
                        // Erase the complete display
                        for row in 0..self.height {
                            for col in 0..self.width {
                                self.write_at(row, col, b' ', false);
                            }
                        }
                    }
                    _ => {
                        // Ignore it
                    }
                }
            }
            'K' => {
                // Erase in Line
                match first {
                    0 => {
                        // Erase the cursor through the end of the line
                        for col in self.col..self.width {
                            self.write_at(self.row, col, b' ', false);
                        }
                    }
                    1 => {
                        // Erase from the beginning of the line through the cursor
                        for col in 0..=self.col {
                            self.write_at(self.row, col, b' ', false);
                        }
                    }
                    2 => {
                        // Erase the complete line
                        for col in 0..self.width {
                            self.write_at(self.row, col, b' ', false);
                        }
                    }
                    _ => {
                        // Ignore it
                    }
                }
            }
            'n' if first == 6 => {
                // Device Status Report - todo.
                //
                // We should send "\u{001b}[<rows>;<cols>R" where <rows> and
                // <cols> are integers for 1-indexed rows and columns
                // respectively. But for that we need an input buffer to put bytes into.
            }
            'h' if intermediates.first().cloned() == Some(b'?') => {
                // DEC special code for Cursor On. It'll be activated whenever
                // we finish what we're printing.
                self.cursor_wanted = true;
            }
            'l' if intermediates.first().cloned() == Some(b'?') => {
                // DEC special code for Cursor Off.
                self.cursor_wanted = false;
            }
            _ => {
                // Unknown code - ignore it
            }
        }
    }
}

// ===========================================================================
// Private functions
// ===========================================================================

// None

// ===========================================================================
// Public functions
// ===========================================================================

// None

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::VgaConsole;
    const WIDTH: usize = 12;
    const HEIGHT: usize = 7;

    /// Convert a text buffer into a string we can compare against.
    ///
    /// Each glyph and attribute is printed like "xx yy", separated by "|" for
    /// each column, and "\n" for each row.
    fn print_buffer(buffer: &[u8]) -> String {
        use std::fmt::Write;
        let mut output = String::new();
        let mut pos = 0;
        for _r in 0..HEIGHT {
            for _c in 0..WIDTH {
                write!(output, "{:02x} {:02x}|", buffer[pos], buffer[pos + 1]).unwrap();
                pos += 2;
            }
            writeln!(output).unwrap();
        }
        output
    }

    #[test]
    fn basic_print() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        console.write_bstr(b"Hello\n");
        assert_eq!(
            print_buffer(&buffer),
            "\
        48 07|65 07|6c 07|6c 07|6f 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 0);
    }

    #[test]
    fn cr_overprint() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        console.write_bstr(b"0\r1\n");
        // Second row
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 0);
        // The '1' has replaced the 0
        assert_eq!(
            print_buffer(&buffer),
            "\
        31 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        // We are on the second row
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 0);
    }

    #[test]
    fn scroll() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        console.write_bstr(b"0\n");
        console.write_bstr(b"1\n");
        for _ in 0..HEIGHT - 1 {
            console.write_bstr(b"\n");
        }
        // We are now off the bottom of the screen
        assert_eq!(console.inner.row, HEIGHT as isize);
        assert_eq!(console.inner.col, 0);
        // And the '1' is on the top row
        assert_eq!(
            print_buffer(&buffer),
            "\
        31 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|\n"
        );
    }

    #[test]
    fn home1() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        // Print 0 and replace it with a 1
        console.write_bstr(b"0\n\x1b[0;0H1\n");
        // We are on the second row
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 0);
        // And the '1' has replaced the '0'
        assert_eq!(
            print_buffer(&buffer),
            "\
        31 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
    }

    #[test]
    fn home2() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        // Print 0 and replace it with a 1
        console.write_bstr(b"0\n\x1b[1;1H1\n");
        // And the '1' has replaced the '0'
        assert_eq!(
            print_buffer(&buffer),
            "\
        31 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
    }

    #[test]
    fn home3() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        // Print 0 and replace it with a 1
        console.write_bstr(b"0\n\x1b[H1\n");
        // The '1' has replaced the '0'
        assert_eq!(
            print_buffer(&buffer),
            "\
        31 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        // We are on the second row
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 0);
    }

    #[test]
    fn movecursor() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        // Print 0 and replace it with a 1
        console.write_bstr(b"\x1b[2;2H1");
        assert_eq!(
            print_buffer(&buffer),
            "\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|31 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        // We are on the second row
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 2);
    }

    #[test]
    fn sgr_reset() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        console.write_bstr(b"\x1b[0m1");
        assert_eq!(
            print_buffer(&buffer),
            "\
        31 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 0);
        assert_eq!(console.inner.col, 1);
    }

    #[test]
    fn sgr_backgrounds() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        // +-------+-----+-----+-----+-----+-----+-----+-----+
        // + BLINK | BG2 | BG1 | BG0 | FG3 | FG2 | FG1 | FG0 |
        // +-------+-----+-----+-----+-----+-----+-----+-----+
        let colour_map = [
            "40", // Light Gray on Black
            "41", // Light Gray on Red
            "42", // Light Gray on Green
            "43", // Light Gray on Yellow
            "44", // Light Gray on Blue
            "45", // Light Gray on Magenta
            "46", // Light Gray on Cyan
            "47", // Light Gray on White
        ];

        for ansi in colour_map.iter() {
            console.write_bstr(b"\x1b[");
            console.write_bstr(ansi.as_bytes());
            console.write_bstr(b"m1");
        }

        assert_eq!(
            print_buffer(&buffer),
            "\
        31 07|31 47|31 27|31 67|31 17|31 57|31 37|31 77|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
    }

    #[test]
    fn sgr_foregrounds() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        // +-------+-----+-----+-----+-----+-----+-----+-----+
        // + BLINK | BG2 | BG1 | BG0 | FG3 | FG2 | FG1 | FG0 |
        // +-------+-----+-----+-----+-----+-----+-----+-----+
        let colour_map = [
            "30", // Black on Black
            "31", // Red on Black
            "32", // Green on Black
            "33", // Yellow on Black
            "34", // Blue on Black
            "35", // Magenta on Black
            "36", // Cyan on Black
            "37", // White on Black
        ];

        for ansi in colour_map.iter() {
            console.write_bstr(b"\x1b[");
            console.write_bstr(ansi.as_bytes());
            console.write_bstr(b"m1");
        }

        assert_eq!(
            print_buffer(&buffer),
            "\
        31 00|31 04|31 02|31 06|31 01|31 05|31 03|31 07|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
    }

    #[test]
    fn sgr_bold() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        // +-------+-----+-----+-----+-----+-----+-----+-----+
        // + BLINK | BG2 | BG1 | BG0 | FG3 | FG2 | FG1 | FG0 |
        // +-------+-----+-----+-----+-----+-----+-----+-----+
        let colour_map = [
            "30", // Bright Black on Black
            "31", // Bright Red on Black
            "32", // Bright Green on Black
            "33", // Bright Yellow on Black
            "34", // Bright Blue on Black
            "35", // Bright Magenta on Black
            "36", // Bright Cyan on Black
            "37", // Bright White on Black
        ];

        console.write_bstr(b"\x1b[1m");

        for ansi in colour_map.iter() {
            console.write_bstr(b"\x1b[");
            console.write_bstr(ansi.as_bytes());
            console.write_bstr(b"m1");
        }

        assert_eq!(
            print_buffer(&buffer),
            "\
        31 08|31 0c|31 0a|31 0e|31 09|31 0d|31 0b|31 0f|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
    }

    #[test]
    fn sgr_all_three() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        // +-------+-----+-----+-----+-----+-----+-----+-----+
        // + BLINK | BG2 | BG1 | BG0 | FG3 | FG2 | FG1 | FG0 |
        // +-------+-----+-----+-----+-----+-----+-----+-----+
        let colour_map = [
            "1;40;37", // Bright White on Black
            "0",       // Default
            "33;44",   // Brown on Blue
        ];

        for ansi in colour_map.iter() {
            console.write_bstr(b"\x1b[");
            console.write_bstr(ansi.as_bytes());
            console.write_bstr(b"m1");
        }

        assert_eq!(
            print_buffer(&buffer),
            "\
        31 0f|31 07|31 16|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
    }

    #[test]
    fn cursor_up() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        // Go home, print 0\n then go up a line and replace the 0 with a 1
        console.write_bstr(b"\x1b[H0\n\x1b[A1");
        assert_eq!(
            print_buffer(&buffer),
            "\
        31 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 0);
        assert_eq!(console.inner.col, 1);
        // Go home, print 0\n then go up a line and replace the 0 with a 2
        console.write_bstr(b"\x1b[H0\n\x1b[0A2");
        assert_eq!(
            print_buffer(&buffer),
            "\
        32 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 0);
        assert_eq!(console.inner.col, 1);
        // Go home, print 0\n then go up a line and replace the 0 with a 3
        console.write_bstr(b"\x1b[H0\n\x1b[1A3");
        assert_eq!(
            print_buffer(&buffer),
            "\
        33 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 0);
        assert_eq!(console.inner.col, 1);
        // Go home then print 40\n50\n60\n7 then go up two lines and replace the 0 of 50 with a 8
        console.write_bstr(b"\x1b[H40\n50\n60\n7\x1b[2A8");
        assert_eq!(
            print_buffer(&buffer),
            "\
        34 07|30 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        35 07|38 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        36 07|30 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        37 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 2);
    }

    #[test]
    fn cursor_down() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        // Go home, go down 1 line, and print 0
        console.write_bstr(b"\x1b[H\x1b[B0");
        assert_eq!(
            print_buffer(&buffer),
            "\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        30 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 1);
        // go down 1 line, and print 1
        console.write_bstr(b"\x1b[0B1");
        assert_eq!(
            print_buffer(&buffer),
            "\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        30 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|31 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 2);
        assert_eq!(console.inner.col, 2);
        // go down 1 line, and print 2
        console.write_bstr(b"\x1b[1B2");
        assert_eq!(
            print_buffer(&buffer),
            "\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        30 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|31 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|32 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 3);
        assert_eq!(console.inner.col, 3);
        // go down 2 lines, and print 3
        console.write_bstr(b"\x1b[2B3");
        assert_eq!(
            print_buffer(&buffer),
            "\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        30 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|31 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|32 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|33 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 5);
        assert_eq!(console.inner.col, 4);
    }

    #[test]
    fn cursor_forward() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        // Print .0.1.2..3
        console.write_bstr(b"\x1b[C0");
        console.write_bstr(b"\x1b[0C1");
        console.write_bstr(b"\x1b[1C2");
        console.write_bstr(b"\x1b[2C3");
        assert_eq!(
            print_buffer(&buffer),
            "\
        00 00|30 07|00 00|31 07|00 00|32 07|00 00|00 00|33 07|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
    }

    #[test]
    fn cursor_backwards() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        // Print 123 then replace the 3 with a 4
        console.write_bstr(b"123\x1b[D4");
        assert_eq!(
            print_buffer(&buffer),
            "\
        31 07|32 07|34 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        // Replace the 4 with a 5
        console.write_bstr(b"\x1b[0D5");
        assert_eq!(
            print_buffer(&buffer),
            "\
        31 07|32 07|35 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        // Replace the 5 with a 6
        console.write_bstr(b"\x1b[1D6");
        assert_eq!(
            print_buffer(&buffer),
            "\
        31 07|32 07|36 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        // Replace the 2 with a 7
        console.write_bstr(b"\x1b[2D7");
        assert_eq!(
            print_buffer(&buffer),
            "\
        31 07|37 07|36 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
    }

    #[test]
    fn cursor_next_line() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        // Go home, print xxx, go down 1 line, and print 0
        console.write_bstr(b"\x1b[Hxxx\x1b[E0");
        // We should have returned to col 0 for the '0' so are in col 1
        assert_eq!(
            print_buffer(&buffer),
            "\
        78 07|78 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        30 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 1);
        // go down 1 line, and print 1
        console.write_bstr(b"xxx\x1b[0E1");
        assert_eq!(
            print_buffer(&buffer),
            "\
        78 07|78 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        30 07|78 07|78 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        31 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 2);
        assert_eq!(console.inner.col, 1);
        // go down 1 line, and print 2
        console.write_bstr(b"xxx\x1b[1E2");
        assert_eq!(
            print_buffer(&buffer),
            "\
        78 07|78 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        30 07|78 07|78 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        31 07|78 07|78 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        32 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 3);
        assert_eq!(console.inner.col, 1);
        // go down 2 lines, and print 3
        console.write_bstr(b"xxx\x1b[2E3");
        assert_eq!(
            print_buffer(&buffer),
            "\
        78 07|78 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        30 07|78 07|78 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        31 07|78 07|78 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        32 07|78 07|78 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        33 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 5);
        assert_eq!(console.inner.col, 1);
    }

    #[test]
    fn cursor_previous_line() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        // Print xx, xx, 11, 22, 33, 456 on the first five lines
        // Then go back and replace 4 with 7, 3 with 8, 2 with 9 and the first x with 0
        console.write_bstr(b"xx\nxx\n11\n22\n33\n456\x1b[F7\x1b[0F8\x1b[1F9\x1b[2F0");
        // We should be back up on the top row
        assert_eq!(
            print_buffer(&buffer),
            "\
        30 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        78 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        39 07|31 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        38 07|32 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        37 07|33 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        34 07|35 07|36 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 0);
        assert_eq!(console.inner.col, 1);
    }

    #[test]
    fn cursor_horizontal_absolute() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        // Print 12345 the replace the 3 with a 9
        console.write_bstr(b"12345\x1b[3G9");
        assert_eq!(
            print_buffer(&buffer),
            "\
        31 07|32 07|39 07|34 07|35 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 0);
        assert_eq!(console.inner.col, 3);
    }

    #[test]
    fn cursor_position() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        // In row;col form.
        console.write_bstr(b"xxx\x1b[H0\x1b[;3H1\x1b[2;H2\x1b[3;4H3");
        // the 4 should be in the right-hand column, and the 5 should wrap
        // around and start on the next row.
        console.write_bstr(format!("\x1b[4;{}H45", WIDTH).as_bytes());
        assert_eq!(
            print_buffer(&buffer),
            "\
        30 07|78 07|31 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        32 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|33 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|34 07|\n\
        35 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 4);
        assert_eq!(console.inner.col, 1);
    }

    #[test]
    fn erase_in_display_cursor_to_end() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        console.write_bstr(b"xxx\nxxx\n\x1b[2;2H");
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 1);
        console.write_bstr(b"\x1b[0J");
        assert_eq!(
            print_buffer(&buffer),
            "\
        78 07|78 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        78 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|\n\
        20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|\n\
        20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|\n\
        20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|\n\
        20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|\n\
        20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|\n"
        );
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 1);
    }

    #[test]
    fn erase_in_display_start_to_cursor() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        console.write_bstr(b"xxx\nxxx\n\x1b[2;2H");
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 1);
        console.write_bstr(b"\x1b[1J");
        assert_eq!(
            print_buffer(&buffer),
            "\
        20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|\n\
        20 07|20 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 1);
    }

    #[test]
    fn erase_in_display_entire_screen() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        console.write_bstr(b"xxx\nxxx\n\x1b[2;2H");
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 1);
        console.write_bstr(b"\x1b[2J");
        assert_eq!(
            print_buffer(&buffer),
            "\
        20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|\n\
        20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|\n\
        20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|\n\
        20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|\n\
        20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|\n\
        20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|\n\
        20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|\n"
        );
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 1);
    }

    #[test]
    fn erase_in_line_cursor_to_end() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        console.write_bstr(b"xxx\nxxx\n\x1b[2;2H");
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 1);
        console.write_bstr(b"\x1b[0K");
        assert_eq!(
            print_buffer(&buffer),
            "\
        78 07|78 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        78 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 1);
    }

    #[test]
    fn erase_in_line_start_to_cursor() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        console.write_bstr(b"xxx\nxxx\n\x1b[2;2H");
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 1);
        console.write_bstr(b"\x1b[1K");
        assert_eq!(
            print_buffer(&buffer),
            "\
        78 07|78 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        20 07|20 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 1);
    }

    #[test]
    fn erase_in_line_entire_line() {
        let mut buffer = [0u8; WIDTH * HEIGHT * 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as isize, HEIGHT as isize);
        console.write_bstr(b"xxx\nxxx\n\x1b[2;2H");
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 1);
        console.write_bstr(b"\x1b[2K");
        assert_eq!(
            print_buffer(&buffer),
            "\
        78 07|78 07|78 07|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|20 07|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n\
        00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|00 00|\n"
        );
        assert_eq!(console.inner.row, 1);
        assert_eq!(console.inner.col, 1);
    }
}

// ===========================================================================
// End of file
// ===========================================================================
