//! # Handles Keyboard Character Map
//!
//! We get events from the BIOS HID API which describe keypresses. This module
//! provides a type which can convert keypresses into a stream of Unicode
//! characters.
//!
//! It only handles UK English keyboards at present.

use crate::bios;

use bios::hid::KeyCode;

#[derive(Copy, Clone, PartialEq, Eq, Debug, PartialOrd, Ord)]
pub enum SpecialKey {
    /// The Down cursor key
    ArrowDown,
    /// The Left cursor key
    ArrowLeft,
    /// The Right cursor key
    ArrowRight,
    /// The Up cursor key
    ArrowUp,
    /// The `Delete` (`Del`) key
    Delete,
    /// The `End` key
    End,
    /// The `F1` key
    F1,
    /// The `F2` key
    F2,
    /// The `F3` key
    F3,
    /// The `F4` key
    F4,
    /// The `F5` key
    F5,
    /// The `F6` key
    F6,
    /// The `F7` key
    F7,
    /// The `F8` key
    F8,
    /// The `F9` key
    F9,
    /// The `F10` key
    F10,
    /// The `F11` key
    F11,
    /// The `F12` key
    F12,
    /// The `Home` key
    Home,
    /// The `Insert` key
    Insert,
    /// The `Right-click Menu` key
    Menus,
    /// The `Num Lock` key on the Numeric Keypad
    NumpadLock,
    /// The `Page Down` key
    PageDown,
    /// The `Page Up` key
    PageUp,
    /// The `Pause/Break` key
    PauseBreak,
    /// The `Print Screen` (`PrtScr`) key
    PrintScreen,
    /// The `Scroll Lock` key
    ScrollLock,
    /// Media transport: previous track
    PrevTrack,
    /// Media transport: next track
    NextTrack,
    /// Media transport: mute audio
    Mute,
    /// Application key: open calculator
    Calculator,
    /// Media transport: play audio
    Play,
    /// Media transport: stop audio
    Stop,
    /// Media transport: turn volume down
    VolumeDown,
    /// Media transport: turn volume up
    VolumeUp,
    /// Media transport: open browser to homepage
    WWWHome,
    /// Keyboard Power On Test passed.
    ///
    /// Expect this once on start-up.
    PowerOnTestOk,
}

/// Represents a single keypress.
///
/// Could be a Unicode character, or a special key (like Arrow Up).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Keypress {
    /// Got a new Unicode character from the keyboard
    Unicode(char),
    /// Got some special keypress (e.g. an arrow key)
    Special(SpecialKey),
}

/// Decodes a 104/105-key UK English keyboard.
#[derive(Debug, Clone, Default)]
pub struct UKEnglish {
    left_shift: bool,
    right_shift: bool,
    left_alt: bool,
    right_alt: bool,
    left_ctrl: bool,
    right_ctrl: bool,
    left_win: bool,
    right_win: bool,
    caps_lock: bool,
}

impl UKEnglish {
    pub fn new() -> UKEnglish {
        Default::default()
    }

    pub fn shift_pressed(&self) -> bool {
        self.left_shift || self.right_shift
    }

    pub fn want_capitals(&self) -> bool {
        self.caps_lock ^ self.shift_pressed()
    }

    /// Handle a new BIOS HID event.
    ///
    /// May generate a new keypress.
    pub fn handle_event(&mut self, event: bios::hid::HidEvent) -> Option<Keypress> {
        match event {
            bios::hid::HidEvent::KeyPress(code) => self.handle_press(code),
            bios::hid::HidEvent::KeyRelease(code) => {
                self.handle_release(code);
                None
            }
            _ => {
                // Ignore mouse and other events
                None
            }
        }
    }

    /// Handle a key being pressed.
    ///
    /// This may generate a new keypress.
    pub fn handle_press(&mut self, code: KeyCode) -> Option<Keypress> {
        match code {
            // Modifiers
            KeyCode::AltLeft => {
                self.left_alt = true;
                None
            }
            KeyCode::AltRight => {
                self.right_alt = true;
                None
            }
            KeyCode::CapsLock => {
                self.caps_lock = !self.caps_lock;
                None
            }
            KeyCode::ControlLeft => {
                self.left_ctrl = true;
                None
            }
            KeyCode::ControlRight => {
                self.right_ctrl = true;
                None
            }
            KeyCode::ShiftLeft => {
                self.left_shift = true;
                None
            }
            KeyCode::ShiftRight => {
                self.right_shift = true;
                None
            }
            KeyCode::WindowsLeft => {
                self.left_win = true;
                None
            }
            KeyCode::WindowsRight => {
                self.right_win = true;
                None
            }
            // Special keys
            KeyCode::ArrowDown => Some(Keypress::Special(SpecialKey::ArrowDown)),
            KeyCode::ArrowLeft => Some(Keypress::Special(SpecialKey::ArrowLeft)),
            KeyCode::ArrowRight => Some(Keypress::Special(SpecialKey::ArrowRight)),
            KeyCode::ArrowUp => Some(Keypress::Special(SpecialKey::ArrowUp)),
            KeyCode::Delete => Some(Keypress::Special(SpecialKey::Delete)),
            KeyCode::End => Some(Keypress::Special(SpecialKey::End)),
            KeyCode::F1 => Some(Keypress::Special(SpecialKey::F1)),
            KeyCode::F2 => Some(Keypress::Special(SpecialKey::F2)),
            KeyCode::F3 => Some(Keypress::Special(SpecialKey::F3)),
            KeyCode::F4 => Some(Keypress::Special(SpecialKey::F4)),
            KeyCode::F5 => Some(Keypress::Special(SpecialKey::F5)),
            KeyCode::F6 => Some(Keypress::Special(SpecialKey::F6)),
            KeyCode::F7 => Some(Keypress::Special(SpecialKey::F7)),
            KeyCode::F8 => Some(Keypress::Special(SpecialKey::F8)),
            KeyCode::F9 => Some(Keypress::Special(SpecialKey::F9)),
            KeyCode::F10 => Some(Keypress::Special(SpecialKey::F10)),
            KeyCode::F11 => Some(Keypress::Special(SpecialKey::F11)),
            KeyCode::F12 => Some(Keypress::Special(SpecialKey::F12)),
            KeyCode::Home => Some(Keypress::Special(SpecialKey::Home)),
            KeyCode::Insert => Some(Keypress::Special(SpecialKey::Insert)),
            KeyCode::Menus => Some(Keypress::Special(SpecialKey::Menus)),
            KeyCode::NumpadLock => Some(Keypress::Special(SpecialKey::NumpadLock)),
            KeyCode::PageDown => Some(Keypress::Special(SpecialKey::PageDown)),
            KeyCode::PageUp => Some(Keypress::Special(SpecialKey::PageUp)),
            KeyCode::PauseBreak => Some(Keypress::Special(SpecialKey::PauseBreak)),
            KeyCode::PrintScreen => Some(Keypress::Special(SpecialKey::PrintScreen)),
            KeyCode::ScrollLock => Some(Keypress::Special(SpecialKey::ScrollLock)),
            KeyCode::PrevTrack => Some(Keypress::Special(SpecialKey::PrevTrack)),
            KeyCode::NextTrack => Some(Keypress::Special(SpecialKey::NextTrack)),
            KeyCode::Mute => Some(Keypress::Special(SpecialKey::Mute)),
            KeyCode::Calculator => Some(Keypress::Special(SpecialKey::Calculator)),
            KeyCode::Play => Some(Keypress::Special(SpecialKey::Play)),
            KeyCode::Stop => Some(Keypress::Special(SpecialKey::Stop)),
            KeyCode::VolumeDown => Some(Keypress::Special(SpecialKey::VolumeDown)),
            KeyCode::VolumeUp => Some(Keypress::Special(SpecialKey::VolumeUp)),
            KeyCode::WWWHome => Some(Keypress::Special(SpecialKey::WWWHome)),
            KeyCode::PowerOnTestOk => Some(Keypress::Special(SpecialKey::PowerOnTestOk)),
            // Letter keys
            KeyCode::A => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('A'))
                } else {
                    Some(Keypress::Unicode('a'))
                }
            }
            KeyCode::B => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('B'))
                } else {
                    Some(Keypress::Unicode('b'))
                }
            }
            KeyCode::C => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('C'))
                } else {
                    Some(Keypress::Unicode('c'))
                }
            }
            KeyCode::D => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('D'))
                } else {
                    Some(Keypress::Unicode('d'))
                }
            }
            KeyCode::E => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('E'))
                } else {
                    Some(Keypress::Unicode('e'))
                }
            }
            KeyCode::F => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('F'))
                } else {
                    Some(Keypress::Unicode('f'))
                }
            }
            KeyCode::G => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('G'))
                } else {
                    Some(Keypress::Unicode('g'))
                }
            }
            KeyCode::H => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('H'))
                } else {
                    Some(Keypress::Unicode('h'))
                }
            }
            KeyCode::I => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('I'))
                } else {
                    Some(Keypress::Unicode('i'))
                }
            }
            KeyCode::J => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('J'))
                } else {
                    Some(Keypress::Unicode('j'))
                }
            }
            KeyCode::K => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('K'))
                } else {
                    Some(Keypress::Unicode('k'))
                }
            }
            KeyCode::L => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('L'))
                } else {
                    Some(Keypress::Unicode('l'))
                }
            }
            KeyCode::M => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('M'))
                } else {
                    Some(Keypress::Unicode('m'))
                }
            }
            KeyCode::N => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('N'))
                } else {
                    Some(Keypress::Unicode('n'))
                }
            }
            KeyCode::O => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('O'))
                } else {
                    Some(Keypress::Unicode('o'))
                }
            }
            KeyCode::P => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('P'))
                } else {
                    Some(Keypress::Unicode('p'))
                }
            }
            KeyCode::Q => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('Q'))
                } else {
                    Some(Keypress::Unicode('q'))
                }
            }
            KeyCode::R => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('R'))
                } else {
                    Some(Keypress::Unicode('r'))
                }
            }
            KeyCode::S => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('S'))
                } else {
                    Some(Keypress::Unicode('s'))
                }
            }
            KeyCode::T => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('T'))
                } else {
                    Some(Keypress::Unicode('t'))
                }
            }
            KeyCode::U => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('U'))
                } else {
                    Some(Keypress::Unicode('u'))
                }
            }
            KeyCode::V => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('V'))
                } else {
                    Some(Keypress::Unicode('v'))
                }
            }
            KeyCode::W => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('W'))
                } else {
                    Some(Keypress::Unicode('w'))
                }
            }
            KeyCode::X => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('X'))
                } else {
                    Some(Keypress::Unicode('x'))
                }
            }
            KeyCode::Y => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('Y'))
                } else {
                    Some(Keypress::Unicode('y'))
                }
            }
            KeyCode::Z => {
                if self.want_capitals() {
                    Some(Keypress::Unicode('Z'))
                } else {
                    Some(Keypress::Unicode('z'))
                }
            }
            // Shiftable keys
            KeyCode::BackSlash => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode('~'))
                } else {
                    Some(Keypress::Unicode('#'))
                }
            }
            KeyCode::BackTick => {
                if self.right_alt {
                    Some(Keypress::Unicode('|'))
                } else if self.shift_pressed() {
                    Some(Keypress::Unicode('¬'))
                } else {
                    Some(Keypress::Unicode('`'))
                }
            }
            KeyCode::BracketSquareLeft => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode('{'))
                } else {
                    Some(Keypress::Unicode('['))
                }
            }
            KeyCode::BracketSquareRight => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode('}'))
                } else {
                    Some(Keypress::Unicode(']'))
                }
            }
            KeyCode::Comma => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode('<'))
                } else {
                    Some(Keypress::Unicode(','))
                }
            }
            KeyCode::Equals => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode('_'))
                } else {
                    Some(Keypress::Unicode('='))
                }
            }
            KeyCode::Key1 => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode('!'))
                } else {
                    Some(Keypress::Unicode('1'))
                }
            }
            KeyCode::Key2 => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode('"'))
                } else {
                    Some(Keypress::Unicode('2'))
                }
            }
            KeyCode::Key3 => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode('£'))
                } else {
                    Some(Keypress::Unicode('3'))
                }
            }
            KeyCode::Key4 => {
                if self.right_alt {
                    Some(Keypress::Unicode('€'))
                } else if self.shift_pressed() {
                    Some(Keypress::Unicode('$'))
                } else {
                    Some(Keypress::Unicode('4'))
                }
            }
            KeyCode::Key5 => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode('%'))
                } else {
                    Some(Keypress::Unicode('5'))
                }
            }
            KeyCode::Key6 => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode('^'))
                } else {
                    Some(Keypress::Unicode('6'))
                }
            }
            KeyCode::Key7 => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode('&'))
                } else {
                    Some(Keypress::Unicode('7'))
                }
            }
            KeyCode::Key8 => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode('*'))
                } else {
                    Some(Keypress::Unicode('8'))
                }
            }
            KeyCode::Key9 => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode('('))
                } else {
                    Some(Keypress::Unicode('9'))
                }
            }
            KeyCode::Key0 => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode(')'))
                } else {
                    Some(Keypress::Unicode('0'))
                }
            }
            KeyCode::Minus => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode('_'))
                } else {
                    Some(Keypress::Unicode('-'))
                }
            }
            KeyCode::SemiColon => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode(':'))
                } else {
                    Some(Keypress::Unicode(';'))
                }
            }
            KeyCode::Slash => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode('?'))
                } else {
                    Some(Keypress::Unicode('/'))
                }
            }
            KeyCode::HashTilde => {
                if self.shift_pressed() {
                    Some(Keypress::Unicode('|'))
                } else {
                    Some(Keypress::Unicode('\\'))
                }
            }
            // Non-shiftable keys
            KeyCode::Backspace => Some(Keypress::Unicode(0x08 as char)),
            KeyCode::Enter => Some(Keypress::Unicode('\r')),
            KeyCode::Escape => Some(Keypress::Unicode(0x1B as char)),
            KeyCode::Fullstop => Some(Keypress::Unicode('.')),
            KeyCode::Numpad1 => Some(Keypress::Unicode('1')),
            KeyCode::Numpad2 => Some(Keypress::Unicode('2')),
            KeyCode::Numpad3 => Some(Keypress::Unicode('3')),
            KeyCode::Numpad4 => Some(Keypress::Unicode('4')),
            KeyCode::Numpad5 => Some(Keypress::Unicode('5')),
            KeyCode::Numpad6 => Some(Keypress::Unicode('6')),
            KeyCode::Numpad7 => Some(Keypress::Unicode('7')),
            KeyCode::Numpad8 => Some(Keypress::Unicode('8')),
            KeyCode::Numpad9 => Some(Keypress::Unicode('9')),
            KeyCode::Numpad0 => Some(Keypress::Unicode('0')),
            KeyCode::NumpadEnter => Some(Keypress::Unicode('\r')),
            KeyCode::NumpadSlash => Some(Keypress::Unicode('\\')),
            KeyCode::NumpadStar => Some(Keypress::Unicode('*')),
            KeyCode::NumpadMinus => Some(Keypress::Unicode('-')),
            KeyCode::NumpadPeriod => Some(Keypress::Unicode('.')),
            KeyCode::NumpadPlus => Some(Keypress::Unicode('+')),
            KeyCode::Spacebar => Some(Keypress::Unicode(' ')),
            KeyCode::Tab => Some(Keypress::Unicode('\t')),
            KeyCode::Quote => Some(Keypress::Unicode('\'')),
        }
    }

    /// Handle a key being released.
    ///
    /// This never generates a new keypress.
    pub fn handle_release(&mut self, code: bios::hid::KeyCode) {
        match code {
            KeyCode::AltLeft => {
                self.left_alt = false;
            }
            KeyCode::AltRight => {
                self.right_alt = false;
            }
            KeyCode::ControlLeft => {
                self.left_ctrl = false;
            }
            KeyCode::ControlRight => {
                self.right_ctrl = false;
            }
            KeyCode::WindowsLeft => {
                self.left_win = false;
            }
            KeyCode::WindowsRight => {
                self.right_win = false;
            }
            KeyCode::ShiftLeft => {
                self.left_shift = false;
            }
            KeyCode::ShiftRight => {
                self.right_shift = false;
            }
            _ => {
                // Ignore
            }
        }
    }
}
