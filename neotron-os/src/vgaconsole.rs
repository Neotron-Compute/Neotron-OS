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

use crate::bios::video::{Attr, Mode, TextBackgroundColour, TextForegroundColour};

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
        TextForegroundColour::LightGray,
        TextBackgroundColour::Black,
        false,
    );

    pub fn new(addr: *mut u32, width_chars: u16, height_chars: u16) -> VgaConsole {
        VgaConsole {
            inner: ConsoleInner {
                addr,
                width_chars,
                height_chars,
                mode: FramebufferMode::Text,
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
    /// The `fb_ptr` is given to the BIOS. It can be null, or it must point to a
    /// region big enough to handle the chosen graphics mode.
    pub unsafe fn change_mode(
        &mut self,
        mode: Mode,
        fb_ptr: *mut u32,
    ) -> Result<(), neotron_api::Error> {
        // TODO: support bitmap text rendering whilst in graphics mode

        // Change mode with the BIOS and return the result
        let api = crate::API.get();
        if let neotron_common_bios::FfiResult::Err(_e) = (api.video_set_mode)(mode, fb_ptr) {
            // BIOS says no
            return Err(neotron_api::Error::DeviceSpecific);
        }
        // get whatever buffer the BIOS chose to use
        self.inner.addr = (api.video_get_framebuffer)();
        match mode.format() {
            neotron_common_bios::video::Format::Text8x16
            | neotron_common_bios::video::Format::Text8x8 => {
                self.inner.mode = FramebufferMode::Text;
                // set up the console for this mode
                self.inner.height_chars = mode.text_height().unwrap();
                self.inner.width_chars = mode.text_width().unwrap();
                self.clear();
            }
            neotron_common_bios::video::Format::Chunky1 => {
                self.inner.mode = FramebufferMode::Graphics {
                    width: mode.horizontal_pixels(),
                    height: mode.vertical_lines(),
                    format: FramebufferFormat::Chunky1,
                    stride: mode.line_size_bytes(),
                };
            }
            neotron_common_bios::video::Format::Chunky2 => {
                self.inner.mode = FramebufferMode::Graphics {
                    width: mode.horizontal_pixels(),
                    height: mode.vertical_lines(),
                    format: FramebufferFormat::Chunky2,
                    stride: mode.line_size_bytes(),
                };
            }
            neotron_common_bios::video::Format::Chunky4 => {
                self.inner.mode = FramebufferMode::Graphics {
                    width: mode.horizontal_pixels(),
                    height: mode.vertical_lines(),
                    format: FramebufferFormat::Chunky4,
                    stride: mode.line_size_bytes(),
                };
            }
            neotron_common_bios::video::Format::Chunky8 => {
                self.inner.mode = FramebufferMode::Graphics {
                    width: mode.horizontal_pixels(),
                    height: mode.vertical_lines(),
                    format: FramebufferFormat::Chunky8,
                    stride: mode.line_size_bytes(),
                };
            }
            _ => {
                return Err(neotron_api::Error::Unimplemented);
            }
        }

        Ok(())
    }

    /// Clear the screen.
    ///
    /// In text mode, every character on the screen is replaced with an space (U+0020).
    pub fn clear(&mut self) {
        self.inner.clear();
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

    /// Get the current video mode
    pub fn get_mode(&self) -> Mode {
        let api = crate::API.get();
        (api.video_get_mode)()
    }

    /// Get the framebuffer pointer
    fn get_fb(&self) -> *mut u32 {
        let api = crate::API.get();
        (api.video_get_framebuffer)()
    }

    /// Clear the bitmap
    ///
    /// Returns an error if we're not in bitmap mode
    pub fn gfx_clear(&mut self, colour: u32) -> Result<(), neotron_api::Error> {
        let FramebufferMode::Graphics {
            height,
            format,
            stride,
            ..
        } = &self.inner.mode
        else {
            return Err(neotron_api::Error::InvalidArg);
        };
        let fb_ptr = self.get_fb();
        let pixel_byte = match format {
            FramebufferFormat::Chunky8 => colour as u8,
            FramebufferFormat::Chunky4 => {
                let nibble = (colour as u8) & 0x0F;
                nibble << 4 | nibble
            }
            FramebufferFormat::Chunky2 => {
                let pair = (colour as u8) & 0x03;
                pair << 6 | pair << 4 | pair << 2 | pair
            }
            FramebufferFormat::Chunky1 => {
                let bit = (colour as u8) & 0x01;
                if bit != 0 {
                    0xFF
                } else {
                    0x00
                }
            }
        };
        for y in 0..*height {
            let line_start = unsafe { fb_ptr.byte_add(*stride * (y as usize)) } as *mut u8;
            unsafe {
                line_start.write_bytes(pixel_byte, *stride);
            }
        }
        Ok(())
    }

    /// Draw a line
    ///
    /// Draw a line between `x0, y0` and `x1, y1` in `colour`
    ///
    /// Returns an error if any of the points are off-screen.
    pub fn gfx_draw_line(
        &mut self,
        x0: u16,
        y0: u16,
        x1: u16,
        y1: u16,
        colour: u32,
    ) -> Result<(), neotron_api::Error> {
        let FramebufferMode::Graphics {
            width,
            height,
            format,
            stride,
        } = &self.inner.mode
        else {
            return Err(neotron_api::Error::InvalidArg);
        };
        let fb_ptr = self.get_fb();
        if x0 >= *width {
            return Err(neotron_api::Error::InvalidArg);
        }
        if y1 >= *height {
            return Err(neotron_api::Error::InvalidArg);
        }
        if x1 >= *width {
            return Err(neotron_api::Error::InvalidArg);
        }
        if y1 >= *height {
            return Err(neotron_api::Error::InvalidArg);
        }
        let plot_line_func = match format {
            FramebufferFormat::Chunky8 => plot_line::<8>,
            FramebufferFormat::Chunky4 => plot_line::<4>,
            FramebufferFormat::Chunky2 => plot_line::<2>,
            FramebufferFormat::Chunky1 => plot_line::<1>,
        };
        unsafe {
            plot_line_func(
                fb_ptr as *mut u8,
                *stride,
                x0 as i16,
                y0 as i16,
                x1 as i16,
                y1 as i16,
                colour,
            )
        }
        Ok(())
    }

    /// Plot a single pixel at `x, y` in `colour`
    pub fn gfx_plot(&mut self, x: u16, y: u16, colour: u32) -> Result<(), neotron_api::Error> {
        let FramebufferMode::Graphics {
            width,
            height,
            format,
            stride,
        } = &self.inner.mode
        else {
            return Err(neotron_api::Error::InvalidArg);
        };
        let fb_ptr = self.get_fb();
        if x >= *width {
            return Err(neotron_api::Error::InvalidArg);
        }
        if y >= *height {
            return Err(neotron_api::Error::InvalidArg);
        }
        if fb_ptr.is_null() {
            return Err(neotron_api::Error::NotFound);
        }
        // our video line starts here
        let line_start = unsafe { fb_ptr.byte_add(*stride * (y as usize)) } as *mut u8;
        let chunky_plot_func = match format {
            FramebufferFormat::Chunky8 => chunky_plot::<8>,
            FramebufferFormat::Chunky4 => chunky_plot::<4>,
            FramebufferFormat::Chunky2 => chunky_plot::<2>,
            FramebufferFormat::Chunky1 => chunky_plot::<1>,
        };
        unsafe {
            chunky_plot_func(line_start, x, colour);
        }

        Ok(())
    }
}

// ===========================================================================
// Private types
// ===========================================================================

#[derive(Debug, PartialEq, Eq)]
enum FramebufferMode {
    Graphics {
        /// width in pixels
        width: u16,
        /// height in pixels
        height: u16,
        /// pixel format
        format: FramebufferFormat,
        /// How many bytes per line?
        stride: usize,
    },
    Text,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum FramebufferFormat {
    Chunky1,
    Chunky2,
    Chunky4,
    Chunky8,
}

/// Handles the inner details of where we are on screen.
///
/// Separate from the parser, so it can be passed to the `advance` method.
struct ConsoleInner {
    /// The start of our text buffer.
    ///
    /// Always 32-bit aligned.
    addr: *mut u32,
    /// our current screen format
    mode: FramebufferMode,
    /// The width of the screen in characters
    width_chars: u16,
    /// The height of the screen in characters
    height_chars: u16,
    /// The current row position in characters
    row: u16,
    /// The current column position in characters
    col: u16,
    /// The attribute to apply to the next character we draw
    attr: Attr,
    /// Have we seen the ANSI 'bold' command?
    bright: bool,
    /// Have we seen the ANSI 'reverse' command?
    reverse: bool,
    /// Should we draw a cursor?
    cursor_wanted: bool,
    /// How many times has the cursor been turned off?
    ///
    /// The cursor is only enabled when at `0`.
    cursor_depth: u8,
    /// What character should be where the cursor currently is?
    cursor_holder: Option<u8>,
}

impl ConsoleInner {
    const DEFAULT_ATTR: Attr = Attr::new(
        TextForegroundColour::LightGray,
        TextBackgroundColour::Black,
        false,
    );

    /// Replace the glyph at the current location with a cursor.
    fn cursor_enable(&mut self) {
        self.cursor_depth = self.cursor_depth.saturating_sub(1);
        if self.cursor_depth == 0
            && self.cursor_wanted
            && self.cursor_holder.is_none()
            && self.mode == FramebufferMode::Text
        {
            // Remember what was where our cursor is (unless the cursor is off-screen, when we make something up)
            if self.row < self.height_chars && self.col < self.width_chars {
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
            if self.row < self.height_chars && self.col < self.width_chars {
                // cursor was on-screen, so restore it
                self.write(glyph);
            }
        }
        self.cursor_depth += 1;
    }

    /// Move the cursor relative to the current location.
    ///
    /// Clamps to the visible screen.
    fn move_cursor_relative(&mut self, rows: i16, cols: i16) {
        let new_row = self.row as i16 + rows;
        if new_row < 0 {
            self.row = 0;
        } else if new_row >= self.height_chars as i16 {
            self.row = self.height_chars - 1;
        } else {
            self.row = new_row as u16;
        }
        let new_col = self.col as i16 + cols;
        if new_col < 0 {
            self.col = 0;
        } else if new_col >= self.width_chars as i16 {
            self.col = self.width_chars - 1;
        } else {
            self.col = new_col as u16;
        }
    }

    /// Move the cursor to the given location.
    ///
    /// Clamps to the visible screen.
    fn move_cursor_absolute(&mut self, rows: u16, cols: u16) {
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
        while self.col >= self.width_chars {
            self.col -= self.width_chars;
            self.row += 1;
        }
        while self.row >= self.height_chars {
            self.row -= 1;
            self.scroll_page();
        }
    }

    /// Blank the screen
    fn clear(&mut self) {
        self.cursor_disable();
        for row in 0..self.height_chars {
            for col in 0..self.width_chars {
                self.write_at(row, col, b' ', false);
            }
        }
        self.home();
        self.cursor_enable();
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
    fn write_at(&mut self, row: u16, col: u16, glyph: u8, is_cursor: bool) {
        if self.mode != FramebufferMode::Text {
            // console disabled.
            // TODO: support bitmap font rendering onto a graphical framebuffer
            return;
        }
        assert!(row < self.height_chars, "{} >= {}?", row, self.height_chars);
        assert!(col < self.width_chars, "{} => {}?", col, self.width_chars);
        if !crate::IS_PANIC.load(core::sync::atomic::Ordering::Relaxed) && !is_cursor {
            assert!(self.cursor_holder.is_none());
        }

        let offset = ((row * self.width_chars) + col) * 2;
        let byte_addr = self.addr as *mut u8;
        unsafe { core::ptr::write_volatile(byte_addr.add(offset as usize), glyph) };
        let attr = if self.reverse {
            let new_fg = self.attr.bg().make_foreground();
            let new_bg = self.attr.fg().make_background();
            Attr::new(new_fg, new_bg, false)
        } else {
            self.attr
        };

        unsafe { core::ptr::write_volatile(byte_addr.add(offset as usize + 1), attr.as_u8()) };
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
    fn read_at(&mut self, row: u16, col: u16) -> u8 {
        if self.mode != FramebufferMode::Text {
            // console disabled
            // TODO: support bitmap font parsing off of a graphical framebuffer
            return 0;
        }
        assert!(row < self.height_chars, "{} >= {}?", row, self.height_chars);
        assert!(col < self.width_chars, "{} => {}?", col, self.width_chars);
        if !crate::IS_PANIC.load(core::sync::atomic::Ordering::Relaxed) {
            assert!(self.cursor_holder.is_none());
        }
        let offset = ((row * self.width_chars) + col) * 2;
        let byte_addr = self.addr as *const u8;
        unsafe { core::ptr::read_volatile(byte_addr.add(offset as usize)) }
    }

    /// Move everyone on screen up one line, losing the top line.
    ///
    /// The bottom line will be all space characters.
    fn scroll_page(&mut self) {
        if self.mode != FramebufferMode::Text {
            // console disabled
            // TODO: support bitmap font rendering onto a graphical framebuffer
            return;
        }
        let row_len_words = self.width_chars / 2;
        unsafe {
            // Scroll rows[1..=height-1] to become rows[0..=height-2].
            core::ptr::copy(
                self.addr.add(row_len_words as usize),
                self.addr,
                (row_len_words * (self.height_chars - 1)) as usize,
            );
        }
        // Blank the bottom line of the screen (rows[height-1]).
        for col in 0..self.width_chars {
            self.write_at(self.height_chars - 1, col, b' ', false);
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
            b'\t' => {
                self.col = (self.col + 8) & !7;
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
        let mut first = *params.iter().next().and_then(|s| s.first()).unwrap_or(&1) as i32;
        let mut second = *params.iter().nth(1).and_then(|s| s.first()).unwrap_or(&1) as i32;

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
                            self.attr.set_fg(TextForegroundColour::Black);
                        }
                        31 => {
                            self.attr.set_fg(TextForegroundColour::Red);
                        }
                        32 => {
                            self.attr.set_fg(TextForegroundColour::Green);
                        }
                        33 => {
                            self.attr.set_fg(TextForegroundColour::Brown);
                        }
                        34 => {
                            self.attr.set_fg(TextForegroundColour::Blue);
                        }
                        35 => {
                            self.attr.set_fg(TextForegroundColour::Magenta);
                        }
                        36 => {
                            self.attr.set_fg(TextForegroundColour::Cyan);
                        }
                        37 | 39 => {
                            self.attr.set_fg(TextForegroundColour::LightGray);
                        }
                        // Background
                        40 => {
                            self.attr.set_bg(TextBackgroundColour::Black);
                        }
                        41 => {
                            self.attr.set_bg(TextBackgroundColour::Red);
                        }
                        42 => {
                            self.attr.set_bg(TextBackgroundColour::Green);
                        }
                        43 => {
                            self.attr.set_bg(TextBackgroundColour::Brown);
                        }
                        44 => {
                            self.attr.set_bg(TextBackgroundColour::Blue);
                        }
                        45 => {
                            self.attr.set_bg(TextBackgroundColour::Magenta);
                        }
                        46 => {
                            self.attr.set_bg(TextBackgroundColour::Cyan);
                        }
                        47 | 49 => {
                            self.attr.set_bg(TextBackgroundColour::LightGray);
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
                    self.attr.set_fg(self.attr.fg().brighten())
                }
            }
            'A' => {
                // Cursor Up
                if first == 0 {
                    first = 1;
                }
                self.move_cursor_relative(-first as i16, 0);
            }
            'B' => {
                // Cursor Down
                if first == 0 {
                    first = 1;
                }
                self.move_cursor_relative(first as i16, 0);
            }
            'C' => {
                // Cursor Forward
                if first == 0 {
                    first = 1;
                }
                self.move_cursor_relative(0, first as i16);
            }
            'D' => {
                // Cursor Back
                if first == 0 {
                    first = 1;
                }
                self.move_cursor_relative(0, -first as i16);
            }
            'E' => {
                // Cursor next line
                if first == 0 {
                    first = 1;
                }
                self.move_cursor_relative(first as i16, 0);
                self.move_cursor_absolute(self.row, 0);
            }
            'F' => {
                // Cursor previous line
                if first == 0 {
                    first = 1;
                }
                self.move_cursor_relative(-first as i16, 0);
                self.move_cursor_absolute(self.row, 0);
            }
            'G' => {
                // Cursor horizontal absolute
                if first == 0 {
                    first = 1;
                }
                // We are zero-indexed, ANSI is 1-indexed
                self.move_cursor_absolute(self.row, (first - 1) as u16);
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
                self.move_cursor_absolute((first - 1) as u16, (second - 1) as u16);
            }
            'J' => {
                // Erase in Display
                match first {
                    0 => {
                        // Erase the cursor through the end of the display
                        for row in 0..self.height_chars {
                            for col in 0..self.width_chars {
                                if row > self.row || (row == self.row && col >= self.col) {
                                    self.write_at(row, col, b' ', false);
                                }
                            }
                        }
                    }
                    1 => {
                        // Erase from the beginning of the display through the cursor
                        for row in 0..self.height_chars {
                            for col in 0..self.width_chars {
                                if row < self.row || (row == self.row && col <= self.col) {
                                    self.write_at(row, col, b' ', false);
                                }
                            }
                        }
                    }
                    2 => {
                        // Erase the complete display
                        for row in 0..self.height_chars {
                            for col in 0..self.width_chars {
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
                        for col in self.col..self.width_chars {
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
                        for col in 0..self.width_chars {
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

/// Plot a line
///
/// Adapted from https://en.wikipedia.org/wiki/Bresenham%27s_line_algorithm#All_cases
///
/// # Safety
///
/// Ensure `fb_ptr` points to a buffer that is at least `stride * (y_max + 1)`
/// bytes long, where `y_max` is the larger of `y0` and `y1`.
unsafe fn plot_line<const BPP: u8>(
    fb_ptr: *mut u8,
    stride: usize,
    mut x0: i16,
    mut y0: i16,
    x1: i16,
    y1: i16,
    colour: u32,
) {
    let dx = x1.abs_diff(x0) as i16;
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1.abs_diff(y0) as i16);
    let sy = if y0 < y1 { 1 } else { -1 };
    let line_offset = if y0 < y1 {
        stride as isize
    } else {
        -(stride as isize)
    };
    let mut error = dx + dy;
    let mut line_start = unsafe { fb_ptr.add(stride * y0 as usize) };
    loop {
        chunky_plot::<BPP>(line_start, x0 as u16, colour);
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = error * 2;
        if e2 >= dy {
            error += dy;
            x0 += sx;
        }
        if e2 <= dx {
            error += dx;
            y0 += sy;
            line_start = line_start.offset(line_offset);
        }
    }
}

/// Plot a single pixel into one line of video.
///
/// # Safety
///
/// Ensure `line_start` points to a buffer that is at least `x * BPP / 8` bytes long.
unsafe fn chunky_plot<const BPP: u8>(line_start: *mut u8, x: u16, colour: u32) {
    // this is 8, 4, 2 or 1
    let pixels_per_byte = 8 / BPP;
    // pick a byte in the line
    let byte_ptr = unsafe { line_start.add(x as usize / pixels_per_byte as usize) };
    // load the byte
    let mut byte = unsafe { byte_ptr.read() };
    // this is pixels_per_byte-1 to 0, because the left hand pixel has the upper-most bits
    let pixel_in_byte = (pixels_per_byte - 1) - (x % pixels_per_byte as u16) as u8;
    // This is 2, 4, 16 or 256
    let num_colours = (1 << BPP) as u32;
    // this is 0b1, 0b11, 0xF or 0xFF
    let pixel_mask = num_colours - 1;
    // this marks the pixels of interest
    let shifted_pixel_mask = (pixel_mask << (pixel_in_byte * BPP)) as u8;
    // cap the colour
    let shifted_new_colour = ((colour & pixel_mask) << (pixel_in_byte * BPP)) as u8;
    // zero out the old colour
    byte &= !shifted_pixel_mask;
    // set the new colour
    byte |= shifted_new_colour;
    // write it back
    unsafe {
        byte_ptr.write(byte);
    }
}

// ===========================================================================
// Public functions
// ===========================================================================

// None

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::{chunky_plot, VgaConsole};
    const WIDTH: usize = 12;
    const HEIGHT: usize = 7;

    /// Convert a text buffer into a string we can compare against.
    ///
    /// Each glyph and attribute is printed like "xx yy", separated by "|" for
    /// each column, and "\n" for each row.
    fn print_buffer(buffer: &[u32]) -> String {
        use std::fmt::Write;
        let mut output = String::new();
        let mut pos = 0;
        let base_ptr = buffer.as_ptr() as *const u8;
        for _r in 0..HEIGHT {
            for _c in 0..WIDTH {
                write!(
                    output,
                    "{:02x} {:02x}|",
                    unsafe { *base_ptr.add(pos) },
                    unsafe { *base_ptr.add(pos + 1) }
                )
                .unwrap();
                pos += 2;
            }
            writeln!(output).unwrap();
        }
        output
    }

    #[test]
    fn basic_print() {
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
        console.write_bstr(b"0\n");
        console.write_bstr(b"1\n");
        for _ in 0..HEIGHT - 1 {
            console.write_bstr(b"\n");
        }
        // We are now off the bottom of the screen
        assert_eq!(console.inner.row, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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
        let mut buffer = [0u32; WIDTH * HEIGHT / 2];
        let mut console = VgaConsole::new(buffer.as_mut_ptr(), WIDTH as u16, HEIGHT as u16);
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

    #[test]
    fn chunky1_test() {
        let mut buffer = vec![0x00u8; (640 / 8) + 1];

        _ = unsafe { chunky_plot::<1>(buffer.as_mut_ptr(), 0, 1) };
        assert_eq!(
            &buffer[0..4],
            [0b1000_0000, 0b0000_0000, 0b0000_0000, 0b0000_0000],
            "Got {:02x?}",
            &buffer[0..4]
        );

        _ = unsafe { chunky_plot::<1>(buffer.as_mut_ptr(), 1, 1) };
        assert_eq!(
            &buffer[0..4],
            [0b1100_0000, 0b0000_0000, 0b0000_0000, 0b0000_0000],
            "Got {:02x?}",
            &buffer[0..4]
        );

        _ = unsafe { chunky_plot::<1>(buffer.as_mut_ptr(), 8, 1) };
        assert_eq!(
            &buffer[0..4],
            [0b1100_0000, 0b1000_0000, 0b0000_0000, 0b0000_0000],
            "Got {:02x?}",
            &buffer[0..4]
        );

        _ = unsafe { chunky_plot::<1>(buffer.as_mut_ptr(), 15, 1) };
        assert_eq!(
            &buffer[0..4],
            [0b1100_0000, 0b1000_0001, 0b0000_0000, 0b0000_0000],
            "Got {:02x?}",
            &buffer[0..4]
        );

        _ = unsafe { chunky_plot::<1>(buffer.as_mut_ptr(), 15, 0) };
        assert_eq!(
            &buffer[0..4],
            [0b1100_0000, 0b1000_0000, 0b0000_0000, 0b0000_0000],
            "Got {:02x?}",
            &buffer[0..4]
        );
    }

    #[test]
    fn chunky2_test() {
        let mut buffer = vec![0x00u8; (640 / 4) + 1];

        _ = unsafe { chunky_plot::<2>(buffer.as_mut_ptr(), 0, 1) };
        assert_eq!(
            &buffer[0..4],
            [0b0100_0000, 0b0000_0000, 0b0000_0000, 0b0000_0000],
            "Got {:02x?}",
            &buffer[0..4]
        );

        _ = unsafe { chunky_plot::<2>(buffer.as_mut_ptr(), 1, 2) };
        assert_eq!(
            &buffer[0..4],
            [0b0110_0000, 0b0000_0000, 0b0000_0000, 0b0000_0000],
            "Got {:02x?}",
            &buffer[0..4]
        );

        _ = unsafe { chunky_plot::<2>(buffer.as_mut_ptr(), 1, 3) };
        assert_eq!(
            &buffer[0..4],
            [0b0111_0000, 0b0000_0000, 0b0000_0000, 0b0000_0000],
            "Got {:02x?}",
            &buffer[0..4]
        );

        _ = unsafe { chunky_plot::<2>(buffer.as_mut_ptr(), 4, 1) };
        assert_eq!(
            &buffer[0..4],
            [0b0111_0000, 0b0100_0000, 0b0000_0000, 0b0000_0000],
            "Got {:02x?}",
            &buffer[0..4]
        );

        _ = unsafe { chunky_plot::<2>(buffer.as_mut_ptr(), 7, 3) };
        assert_eq!(
            &buffer[0..4],
            [0b0111_0000, 0b0100_0011, 0b0000_0000, 0b0000_0000],
            "Got {:02x?}",
            &buffer[0..4]
        );
    }

    #[test]
    fn chunky4_test() {
        let mut buffer = vec![0x00u8; (640 / 2) + 1];

        _ = unsafe { chunky_plot::<4>(buffer.as_mut_ptr(), 0, 1) };
        assert_eq!(
            &buffer[0..4],
            [0b0001_0000, 0b0000_0000, 0b0000_0000, 0b0000_0000],
            "Got {:02x?}",
            &buffer[0..4]
        );

        _ = unsafe { chunky_plot::<4>(buffer.as_mut_ptr(), 1, 2) };
        assert_eq!(
            &buffer[0..4],
            [0b0001_0010, 0b0000_0000, 0b0000_0000, 0b0000_0000],
            "Got {:02x?}",
            &buffer[0..4]
        );

        _ = unsafe { chunky_plot::<4>(buffer.as_mut_ptr(), 1, 3) };
        assert_eq!(
            &buffer[0..4],
            [0b0001_0011, 0b0000_0000, 0b0000_0000, 0b0000_0000],
            "Got {:02x?}",
            &buffer[0..4]
        );

        _ = unsafe { chunky_plot::<4>(buffer.as_mut_ptr(), 4, 1) };
        assert_eq!(
            &buffer[0..4],
            [0b0001_0011, 0b0000_0000, 0b0001_0000, 0b0000_0000],
            "Got {:02x?}",
            &buffer[0..4]
        );

        _ = unsafe { chunky_plot::<4>(buffer.as_mut_ptr(), 7, 15) };
        assert_eq!(
            &buffer[0..4],
            [0b0001_0011, 0b0000_0000, 0b0001_0000, 0b0000_1111],
            "Got {:02x?}",
            &buffer[0..4]
        );
    }

    #[test]
    fn chunky8_test() {
        let mut buffer = vec![0x00u8; 641];

        _ = unsafe { chunky_plot::<8>(buffer.as_mut_ptr(), 0, 1) };
        assert_eq!(&buffer[0..4], [1, 0, 0, 0]);

        _ = unsafe { chunky_plot::<8>(buffer.as_mut_ptr(), 1, 2) };
        assert_eq!(&buffer[0..4], [1, 2, 0, 0]);

        _ = unsafe { chunky_plot::<8>(buffer.as_mut_ptr(), 1, 255) };
        assert_eq!(&buffer[0..4], [1, 255, 0, 0]);

        _ = unsafe { chunky_plot::<8>(buffer.as_mut_ptr(), 3, 127) };
        assert_eq!(&buffer[0..4], [1, 255, 0, 127],);

        _ = unsafe { chunky_plot::<8>(buffer.as_mut_ptr(), 3, 255) };
        assert_eq!(&buffer[0..4], [1, 255, 0, 255],);
    }
}

// ===========================================================================
// End of file
// ===========================================================================
