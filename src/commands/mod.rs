//! Commands for Neotron OS
//!
//! Defines the top-level menu, and the commands it can call.

pub use super::Ctx;

mod config;
mod hardware;
mod input;
mod screen;

pub static OS_MENU: menu::Menu<Ctx> = menu::Menu {
    label: "root",
    items: &[
        &config::COMMAND_ITEM,
        &hardware::LSHW_ITEM,
        &screen::CLEAR_ITEM,
        &screen::FILL_ITEM,
        &input::KBTEST_ITEM,
    ],
    entry: None,
    exit: None,
};
