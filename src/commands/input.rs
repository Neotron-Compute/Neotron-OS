//! Input related commands for Neotron OS

use crate::{bios, println, Ctx, API};

/// Called when the "kbtest" command is executed.
pub fn kbtest(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], ctx: &mut Ctx) {
    let api = API.get();
    loop {
        match (api.hid_get_event)() {
            bios::Result::Ok(bios::Option::Some(bios::hid::HidEvent::KeyPress(code))) => {
                let pckb_ev = pc_keyboard::KeyEvent {
                    code,
                    state: pc_keyboard::KeyState::Down,
                };
                if let Some(ev) = ctx.keyboard.process_keyevent(pckb_ev) {
                    println!("Code={code:?} State=Down Decoded={ev:?}");
                } else {
                    println!("Code={code:?} State=Down Decoded=None");
                }
                if code == pc_keyboard::KeyCode::Escape {
                    break;
                }
            }
            bios::Result::Ok(bios::Option::Some(bios::hid::HidEvent::KeyRelease(code))) => {
                let pckb_ev = pc_keyboard::KeyEvent {
                    code,
                    state: pc_keyboard::KeyState::Up,
                };
                if let Some(ev) = ctx.keyboard.process_keyevent(pckb_ev) {
                    println!("Code={code:?} State=Up Decoded={ev:?}");
                } else {
                    println!("Code={code:?} State=Up Decoded=None");
                }
            }
            bios::Result::Ok(bios::Option::Some(bios::hid::HidEvent::MouseInput(_ignore))) => {}
            bios::Result::Ok(bios::Option::None) => {
                // Do nothing
            }
            bios::Result::Err(e) => {
                println!("Failed to get HID events: {:?}", e);
            }
        }
    }
}
