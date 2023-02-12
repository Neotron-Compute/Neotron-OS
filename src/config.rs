//! # OS Configuration
//!
//! Handles persistently storing OS configuration, using the BIOS.

use crate::{bios, API};
use serde::{Deserialize, Serialize};

/// Represents our configuration information that we ask the BIOS to serialise
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    vga_console: bool,
    serial_console: bool,
    serial_baud: u32,
}

impl Config {
    pub fn load() -> Result<Config, &'static str> {
        let api = API.get();
        let mut buffer = [0u8; 64];
        match (api.configuration_get)(bios::ApiBuffer::new(&mut buffer)) {
            bios::Result::Ok(n) => {
                postcard::from_bytes(&buffer[0..n]).map_err(|_e| "Failed to parse config")
            }
            bios::Result::Err(_e) => Err("Failed to load config"),
        }
    }

    pub fn save(&self) -> Result<(), &'static str> {
        let api = API.get();
        let mut buffer = [0u8; 64];
        let slice = postcard::to_slice(self, &mut buffer).map_err(|_e| "Failed to parse config")?;
        match (api.configuration_set)(bios::ApiByteSlice::new(slice)) {
            bios::Result::Ok(_) => Ok(()),
            bios::Result::Err(bios::Error::Unimplemented) => Err("BIOS doesn't support this (yet)"),
            bios::Result::Err(_) => Err("BIOS reported an error"),
        }
    }

    /// Should this system use the VGA console?
    pub fn get_vga_console(&self) -> bool {
        self.vga_console
    }

    // Set whether this system should use the VGA console.
    pub fn set_vga_console(&mut self, new_value: bool) {
        self.vga_console = new_value;
    }

    /// Should this system use the UART console?
    pub fn get_serial_console(&self) -> Option<(u8, bios::serial::Config)> {
        if self.serial_console {
            Some((
                0,
                bios::serial::Config {
                    data_rate_bps: self.serial_baud,
                    data_bits: bios::serial::DataBits::Eight,
                    stop_bits: bios::serial::StopBits::One,
                    parity: bios::serial::Parity::None,
                    handshaking: bios::serial::Handshaking::None,
                },
            ))
        } else {
            None
        }
    }

    /// Turn the serial console off
    pub fn set_serial_console_off(&mut self) {
        self.serial_console = false;
        self.serial_baud = 0;
    }

    /// Turn the serial console on
    pub fn set_serial_console_on(&mut self, serial_baud: u32) {
        self.serial_console = true;
        self.serial_baud = serial_baud;
    }
}

impl core::default::Default for Config {
    fn default() -> Config {
        Config {
            vga_console: true,
            serial_console: false,
            serial_baud: 115200,
        }
    }
}
