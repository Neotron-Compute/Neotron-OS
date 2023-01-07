//! # OS Configuration
//!
//! Handles persistently storing OS configuration, using the BIOS.

use crate::{bios, API};
use serde::{Deserialize, Serialize};

/// Represents our configuration information that we ask the BIOS to serialise
#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    vga_console_on: bool,
    serial_console_on: bool,
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
        (api.configuration_set)(bios::ApiByteSlice::new(slice));
        Ok(())
    }

    /// Should this system use the VGA console?
    pub fn has_vga_console(&self) -> bool {
        self.vga_console_on
    }

    /// Should this system use the UART console?
    pub fn has_serial_console(&self) -> Option<(u8, bios::serial::Config)> {
        if self.serial_console_on {
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
}

impl core::default::Default for Config {
    fn default() -> Config {
        Config {
            vga_console_on: true,
            serial_console_on: false,
            serial_baud: 115200,
        }
    }
}
