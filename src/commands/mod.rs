//! Commands for Neotron OS
//!
//! Defines the top-level menu, and the commands it can call.

pub use super::Ctx;

mod block;
mod config;
mod fs;
mod hardware;
mod input;
mod ram;
mod screen;
mod sound;
mod timedate;

pub static OS_MENU: menu::Menu<Ctx> = menu::Menu {
    label: "root",
    items: &[
        &timedate::DATE_ITEM,
        &config::COMMAND_ITEM,
        &block::LSBLK_ITEM,
        &block::READ_ITEM,
        &fs::DIR_ITEM,
        &hardware::LSHW_ITEM,
        &ram::HEXDUMP_ITEM,
        &ram::RUN_ITEM,
        &ram::LOAD_ITEM,
        &fs::LOAD_ITEM,
        &screen::CLS_ITEM,
        &input::KBTEST_ITEM,
        &hardware::SHUTDOWN_ITEM,
        &sound::MIXER_ITEM,
        &sound::PLAY_ITEM,
    ],
    entry: None,
    exit: None,
};
