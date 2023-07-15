//! Filesystem related types

use chrono::{Datelike, Timelike};
use neotron_common_bios as bios;

use crate::API;

pub struct BiosBlock();

impl embedded_sdmmc::BlockDevice for BiosBlock {
    type Error = bios::Error;

    fn read(
        &self,
        blocks: &mut [embedded_sdmmc::Block],
        start_block_idx: embedded_sdmmc::BlockIdx,
        _reason: &str,
    ) -> Result<(), Self::Error> {
        let api = API.get();
        let byte_slice = unsafe {
            core::slice::from_raw_parts_mut(
                blocks.as_mut_ptr() as *mut u8,
                blocks.len() * embedded_sdmmc::Block::LEN,
            )
        };
        match (api.block_read)(
            0,
            bios::block_dev::BlockIdx(u64::from(start_block_idx.0)),
            blocks.len() as u8,
            bios::FfiBuffer::new(byte_slice),
        ) {
            bios::ApiResult::Ok(_) => Ok(()),
            bios::ApiResult::Err(e) => Err(e),
        }
    }

    fn write(
        &self,
        blocks: &[embedded_sdmmc::Block],
        start_block_idx: embedded_sdmmc::BlockIdx,
    ) -> Result<(), Self::Error> {
        let api = API.get();
        let byte_slice = unsafe {
            core::slice::from_raw_parts(
                blocks.as_ptr() as *const u8,
                blocks.len() * embedded_sdmmc::Block::LEN,
            )
        };
        match (api.block_write)(
            0,
            bios::block_dev::BlockIdx(u64::from(start_block_idx.0)),
            blocks.len() as u8,
            bios::FfiByteSlice::new(byte_slice),
        ) {
            bios::ApiResult::Ok(_) => Ok(()),
            bios::ApiResult::Err(e) => Err(e),
        }
    }

    fn num_blocks(&self) -> Result<embedded_sdmmc::BlockCount, Self::Error> {
        let api = API.get();
        match (api.block_dev_get_info)(0) {
            bios::FfiOption::Some(info) => Ok(embedded_sdmmc::BlockCount(info.num_blocks as u32)),
            bios::FfiOption::None => Err(bios::Error::InvalidDevice),
        }
    }
}

pub struct BiosTime();

impl embedded_sdmmc::TimeSource for BiosTime {
    fn get_timestamp(&self) -> embedded_sdmmc::Timestamp {
        let time = API.get_time();
        embedded_sdmmc::Timestamp {
            year_since_1970: (time.year() - 1970) as u8,
            zero_indexed_month: time.month0() as u8,
            zero_indexed_day: time.day0() as u8,
            hours: time.hour() as u8,
            minutes: time.minute() as u8,
            seconds: time.second() as u8,
        }
    }
}
