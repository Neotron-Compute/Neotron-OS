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
    osprintln!("Press Ctrl-X to quit");
    const CTRL_X: u8 = 0x18;
    'outer: loop {
        if let Some(ev) = crate::STD_INPUT.lock().get_raw() {
            osprintln!("Event: {ev:?}");
            if ev == pc_keyboard::DecodedKey::Unicode(CTRL_X as char) {
                break 'outer;
            }
        }
        let mut buffer = [0u8; 8];
        let count = if let Some(serial) = crate::SERIAL_CONSOLE.lock().as_mut() {
            serial
                .read_data(&mut buffer)
                .ok()
                .and_then(|n| if n == 0 { None } else { Some(n) })
        } else {
            None
        };
        if let Some(count) = count {
            osprintln!("Serial RX: {:x?}", &buffer[0..count]);
            for b in &buffer[0..count] {
                if *b == CTRL_X {
                    break 'outer;
                }
            }
        }
    }
    osprintln!("Finished.");
}
