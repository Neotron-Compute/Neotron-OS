//! Input related commands for Neotron OS

use crate::{bios, osprintln, Ctx, API};

pub static KBTEST_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: kbtest,
        parameters: &[],
    },
    command: "input_kbtest",
    help: Some("Test the keyboard (press ESC to quit)"),
};

/// Called when the "kbtest" command is executed.
fn kbtest(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], ctx: &mut Ctx) {
    let api = API.get();
    loop {
        match (api.hid_get_event)() {
            bios::ApiResult::Ok(bios::FfiOption::Some(bios::hid::HidEvent::KeyPress(code))) => {
                let pckb_ev = pc_keyboard::KeyEvent {
                    code,
                    state: pc_keyboard::KeyState::Down,
                };
                if let Some(ev) = ctx.keyboard.process_keyevent(pckb_ev) {
                    osprintln!("Code={code:?} State=Down Decoded={ev:?}");
                } else {
                    osprintln!("Code={code:?} State=Down Decoded=None");
                }
                if code == pc_keyboard::KeyCode::Escape {
                    break;
                }
            }
            bios::ApiResult::Ok(bios::FfiOption::Some(bios::hid::HidEvent::KeyRelease(code))) => {
                let pckb_ev = pc_keyboard::KeyEvent {
                    code,
                    state: pc_keyboard::KeyState::Up,
                };
                if let Some(ev) = ctx.keyboard.process_keyevent(pckb_ev) {
                    osprintln!("Code={code:?} State=Up Decoded={ev:?}");
                } else {
                    osprintln!("Code={code:?} State=Up Decoded=None");
                }
            }
            bios::ApiResult::Ok(bios::FfiOption::Some(bios::hid::HidEvent::MouseInput(
                _ignore,
            ))) => {}
            bios::ApiResult::Ok(bios::FfiOption::None) => {
                // Do nothing
            }
            bios::ApiResult::Err(e) => {
                osprintln!("Failed to get HID events: {:?}", e);
            }
        }
    }
}
