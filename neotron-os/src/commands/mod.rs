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
        &hardware::LSBLK_ITEM,
        &hardware::LSBUS_ITEM,
        &hardware::LSI2C_ITEM,
        &hardware::LSMEM_ITEM,
        &hardware::LSUART_ITEM,
        &hardware::I2C_ITEM,
        &block::READ_ITEM,
        &fs::DIR_ITEM,
        &ram::HEXDUMP_ITEM,
        &ram::RUN_ITEM,
        &fs::LOAD_ITEM,
        &fs::EXEC_ITEM,
        &fs::TYPE_ITEM,
        &fs::ROM_ITEM,
        &screen::CLS_ITEM,
        &screen::MODE_ITEM,
        &screen::GFX_ITEM,
        &input::KBTEST_ITEM,
        &hardware::SHUTDOWN_ITEM,
        &sound::MIXER_ITEM,
        &sound::PLAY_ITEM,
    ],
    entry: None,
    exit: None,
};

/// Parse a string into a `usize`
///
/// Numbers like `0x123` are hex. Numbers like `123` are decimal.
fn parse_usize(input: &str) -> Result<usize, core::num::ParseIntError> {
    if let Some(digits) = input.strip_prefix("0x") {
        // Parse as hex
        usize::from_str_radix(digits, 16)
    } else {
        // Parse as decimal
        input.parse::<usize>()
    }
}

/// Parse a string into a `u8`
///
/// Numbers like `0x123` are hex. Numbers like `123` are decimal.
fn parse_u8(input: &str) -> Result<u8, core::num::ParseIntError> {
    if let Some(digits) = input.strip_prefix("0x") {
        // Parse as hex
        u8::from_str_radix(digits, 16)
    } else {
        // Parse as decimal
        input.parse::<u8>()
    }
}

/// Parse a string into a `u64`
///
/// Numbers like `0x123` are hex. Numbers like `123` are decimal.
fn parse_u64(input: &str) -> Result<u64, core::num::ParseIntError> {
    if let Some(digits) = input.strip_prefix("0x") {
        // Parse as hex
        u64::from_str_radix(digits, 16)
    } else {
        // Parse as decimal
        input.parse::<u64>()
    }
}

// End of file
