//! # VGA Console
//!
//! Code for dealing with a VGA-style console, where there's a buffer of 16-bit
//! values, each corresponding to a glyph and some attributes.

use neotron_common_bios::video::{Attr, TextBackgroundColour, TextForegroundColour};

#[derive(Debug)]
pub struct VgaConsole {
    addr: *mut u8,
    width: isize,
    height: isize,
    row: isize,
    col: isize,
}

impl VgaConsole {
    /// White on Black
    const DEFAULT_ATTR: Attr = Attr::new(
        TextForegroundColour::WHITE,
        TextBackgroundColour::BLACK,
        false,
    );

    pub fn new(addr: *mut u8, width: isize, height: isize) -> VgaConsole {
        VgaConsole {
            addr,
            width,
            height,
            row: 0,
            col: 0,
        }
    }

    fn move_char_right(&mut self) {
        self.col += 1;
    }

    fn move_char_down(&mut self) {
        self.row += 1;
    }

    fn reset_cursor(&mut self) {
        self.row = 0;
        self.col = 0;
    }

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

    pub fn clear(&mut self) {
        for row in 0..self.height {
            for col in 0..self.width {
                self.write_at(row, col, b' ', Some(Self::DEFAULT_ATTR));
            }
        }
        self.reset_cursor();
    }

    fn write(&mut self, glyph: u8, attr: Option<Attr>) {
        self.write_at(self.row, self.col, glyph, attr);
    }

    fn write_at(&mut self, row: isize, col: isize, glyph: u8, attr: Option<Attr>) {
        assert!(row < self.height, "{} >= {}?", row, self.height);
        assert!(col < self.width, "{} => {}?", col, self.width);
        let offset = ((row * self.width) + col) * 2;
        unsafe { core::ptr::write_volatile(self.addr.offset(offset), glyph) };
        if let Some(a) = attr {
            unsafe { core::ptr::write_volatile(self.addr.offset(offset + 1), a.0) };
        }
    }

    fn scroll_page(&mut self) {
        let row_len_bytes = self.width * 2;
        unsafe {
            // Scroll rows[1..=height-1] to become rows[0..=height-2].
            core::ptr::copy(
                self.addr.offset(row_len_bytes),
                self.addr,
                (row_len_bytes * (self.height - 1)) as usize,
            );
            // Blank the bottom line of the screen (rows[height-1]).
            for col in 0..self.width {
                self.write_at(self.height - 1, col, b' ', Some(Self::DEFAULT_ATTR));
            }
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
            '\u{0000}'..='\u{007F}' => input as u8,
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
            '\u{00AD}' => 240, // SHY
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
            _ => b'?',
        }
    }
}

impl core::fmt::Write for VgaConsole {
    fn write_str(&mut self, data: &str) -> core::fmt::Result {
        for ch in data.chars() {
            self.scroll_as_required();
            match ch {
                '\u{0008}' => {
                    // This is a backspace, so we go back one character (if we
                    // can). We expect the caller to provide "\u{0008} \u{0008}"
                    // to actually erase the char then move the cursor over it.
                    if self.col > 0 {
                        self.col -= 1;
                    }
                }
                '\r' => {
                    self.col = 0;
                }
                '\n' => {
                    self.col = 0;
                    self.move_char_down();
                }
                _ => {
                    self.write(Self::map_char_to_glyph(ch), None);
                    self.move_char_right();
                }
            }
        }
        Ok(())
    }
}
