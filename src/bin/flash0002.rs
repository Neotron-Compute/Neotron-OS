//! Binary Neotron OS Image
//!
//! This is for Flash Addresses that start at `0x0002_0000`.
//!
//! Copyright (c) The Neotron Developers, 2022
//!
//! Licence: GPL v3 or higher (see ../LICENCE.md)

#![no_std]
#![no_main]

/// This tells the BIOS how to start the OS. This must be the first four bytes
/// of our portion of Flash.
#[link_section = ".entry_point"]
#[used]
pub static ENTRY_POINT_ADDR: extern "C" fn(&neotron_common_bios::Api) -> ! = neotron_os::os_main;
