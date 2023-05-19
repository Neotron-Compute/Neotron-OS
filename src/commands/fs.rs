//! File Systems related commands for Neotron OS

use chrono::{Datelike, Timelike};
use embedded_sdmmc::VolumeIdx;

use crate::{bios, print, println, Ctx, API};

pub static DIR_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: dir,
        parameters: &[],
    },
    command: "dir",
    help: Some("Dir the root directory on block device 0"),
};

pub static LOAD_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: load,
        parameters: &[menu::Parameter::Mandatory {
            parameter_name: "file",
            help: Some("The file to load"),
        }],
    },
    command: "load",
    help: Some("Load a file into the application area"),
};

struct BiosBlock();

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
            bios::ApiBuffer::new(byte_slice),
        ) {
            bios::Result::Ok(_) => Ok(()),
            bios::Result::Err(e) => Err(e),
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
            bios::ApiByteSlice::new(byte_slice),
        ) {
            bios::Result::Ok(_) => Ok(()),
            bios::Result::Err(e) => Err(e),
        }
    }

    fn num_blocks(&self) -> Result<embedded_sdmmc::BlockCount, Self::Error> {
        let api = API.get();
        match (api.block_dev_get_info)(0) {
            bios::Option::Some(info) => Ok(embedded_sdmmc::BlockCount(info.num_blocks as u32)),
            bios::Option::None => Err(bios::Error::InvalidDevice),
        }
    }
}

struct BiosTime();

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

/// Called when the "dir" command is executed.
fn dir(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    fn work() -> Result<(), embedded_sdmmc::Error<bios::Error>> {
        println!("Listing files on Block Device 0, /");
        let bios_block = BiosBlock();
        let time = BiosTime();
        let mut mgr = embedded_sdmmc::VolumeManager::new(bios_block, time);
        // Open the first partition
        let volume = mgr.get_volume(VolumeIdx(0))?;
        let root_dir = mgr.open_root_dir(&volume)?;
        let mut total_bytes = 0u64;
        let mut num_files = 0;
        mgr.iterate_dir(&volume, &root_dir, |dir_entry| {
            let padding = 8 - dir_entry.name.base_name().len();
            for b in dir_entry.name.base_name() {
                let ch = *b as char;
                print!("{}", if ch.is_ascii_graphic() { ch } else { '?' });
            }
            for _ in 0..padding {
                print!(" ");
            }
            print!(" ");
            let padding = 3 - dir_entry.name.extension().len();
            for b in dir_entry.name.extension() {
                let ch = *b as char;
                print!("{}", if ch.is_ascii_graphic() { ch } else { '?' });
            }
            for _ in 0..padding {
                print!(" ");
            }
            if dir_entry.attributes.is_directory() {
                print!(" <DIR>        ");
            } else {
                print!(" {:-13}", dir_entry.size,);
            }
            print!(
                " {:02}/{:02}/{:04}",
                dir_entry.mtime.zero_indexed_day + 1,
                dir_entry.mtime.zero_indexed_month + 1,
                u32::from(dir_entry.mtime.year_since_1970) + 1970
            );
            println!(
                "  {:02}:{:02}",
                dir_entry.mtime.hours, dir_entry.mtime.minutes
            );
            total_bytes += dir_entry.size as u64;
            num_files += 1;
        })?;
        println!("{:-9} file(s)  {:-13} bytes", num_files, total_bytes);
        Ok(())
    }

    match work() {
        Ok(_) => {}
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }
}

/// Called when the "load" command is executed.
#[cfg(target_os = "none")]
fn load(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], _ctx: &mut Ctx) {
    fn work(args: &[&str]) -> Result<(), embedded_sdmmc::Error<bios::Error>> {
        println!("Loading /{} from Block Device 0", args[0]);
        let bios_block = BiosBlock();
        let time = BiosTime();
        let mut mgr = embedded_sdmmc::VolumeManager::new(bios_block, time);
        // Open the first partition
        let mut volume = mgr.get_volume(VolumeIdx(0))?;
        let root_dir = mgr.open_root_dir(&volume)?;
        let mut file = mgr.open_file_in_dir(
            &mut volume,
            &root_dir,
            args[0],
            embedded_sdmmc::Mode::ReadOnly,
        )?;
        let file_length = file.length();
        // Application space starts 4K into Cortex-M SRAM
        const APPLICATION_START_ADDR: usize = 0x2000_1000;
        let application_ram: &'static mut [u8] = unsafe {
            core::slice::from_raw_parts_mut(APPLICATION_START_ADDR as *mut u8, file_length as usize)
        };
        mgr.read(&mut volume, &mut file, application_ram)?;
        Ok(())
    }

    match work(args) {
        Ok(_) => {}
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }
}
