//! # OS Configuration
//!
//! Handles persistently storing OS configuration, using the BIOS.

use crate::{bios, API};
use serde::{Deserialize, Serialize};

/// Represents our configuration information that we ask the BIOS to serialise
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    vga_console: Option<u8>,
    serial_console: bool,
    serial_baud: u32,
}

impl Config {
    pub fn load() -> Result<Config, &'static str> {
        let api = API.get();
        let mut buffer = [0u8; 64];
        match (api.configuration_get)(bios::FfiBuffer::new(&mut buffer)) {
            bios::ApiResult::Ok(n) => {
                postcard::from_bytes(&buffer[0..n]).map_err(|_e| "Failed to parse config")
            }
            bios::ApiResult::Err(_e) => Err("Failed to load config"),
        }
    }

    pub fn save(&self) -> Result<(), &'static str> {
        let api = API.get();
        let mut buffer = [0u8; 64];
        let slice = postcard::to_slice(self, &mut buffer).map_err(|_e| "Failed to parse config")?;
        match (api.configuration_set)(bios::FfiByteSlice::new(slice)) {
            bios::ApiResult::Ok(_) => Ok(()),
            bios::ApiResult::Err(bios::Error::Unimplemented) => {
                Err("BIOS doesn't support this (yet)")
            }
            bios::ApiResult::Err(_) => Err("BIOS reported an error"),
        }
    }

    /// Should this system use the VGA console?
    pub fn get_vga_console(&self) -> Option<bios::video::Mode> {
        self.vga_console.and_then(bios::video::Mode::try_from_u8)
    }

    // Set whether this system should use the VGA console.
    pub fn set_vga_console(&mut self, new_value: Option<bios::video::Mode>) {
        self.vga_console = new_value.map(|m| m.as_u8());
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
            vga_console: Some(0),
            serial_console: false,
            serial_baud: 115200,
        }
    }
}

// End of file
