//! Screen-related commands for Neotron OS

use crate::{osprint, Ctx};

pub static CLS_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: cls,
        parameters: &[],
    },
    command: "cls",
    help: Some("Clear the screen"),
};

/// Called when the "cls" command is executed.
fn cls(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    // Reset SGR, go home, clear screen,
    let _ = osprint!("\u{001b}[0m\u{001b}[1;1H\u{001b}[2J");
}
