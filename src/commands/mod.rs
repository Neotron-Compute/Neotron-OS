//! Commands for Neotron OS
//!
//! Defines the top-level menu, and the commands it can call.

pub use super::Ctx;

mod config;
mod hardware;
mod input;
mod ram;
mod screen;
mod timedate;

pub static OS_MENU: menu::Menu<Ctx> = menu::Menu {
    label: "root",
    items: &[
        &timedate::DATE_ITEM,
        &config::COMMAND_ITEM,
        &hardware::LSHW_ITEM,
        &ram::HEXDUMP_ITEM,
        &ram::LOAD_ITEM,
        #[cfg(target_os = "none")]
        &ram::RUN_ITEM,
        &screen::CLEAR_ITEM,
        &screen::BENCH_ITEM,
        &screen::FILL_ITEM,
        &screen::MANDEL_ITEM,
        &input::KBTEST_ITEM,
    ],
    entry: None,
    exit: None,
};
