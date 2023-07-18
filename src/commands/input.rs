//! Input related commands for Neotron OS

use crate::{osprintln, Ctx};

pub static KBTEST_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: kbtest,
        parameters: &[],
    },
    command: "input_kbtest",
    help: Some("Test the keyboard (press ESC to quit)"),
};

/// Called when the "kbtest" command is executed.
fn kbtest(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    osprintln!("Press ESC to quit");
    loop {
        if let Some(ev) = crate::STD_INPUT.lock().get_raw() {
            osprintln!("Event: {ev:?}");
            if ev == pc_keyboard::DecodedKey::RawKey(pc_keyboard::KeyCode::Escape)
                || ev == pc_keyboard::DecodedKey::Unicode('\u{001b}')
            {
                break;
            }
        }
    }
}
