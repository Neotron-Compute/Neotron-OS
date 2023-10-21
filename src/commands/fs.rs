//! File Systems related commands for Neotron OS

use embedded_sdmmc::VolumeIdx;

use crate::{bios, osprint, osprintln, Ctx};

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

pub static TYPE_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: typefn,
        parameters: &[menu::Parameter::Mandatory {
            parameter_name: "file",
            help: Some("The file to type"),
        }],
    },
    command: "type",
    help: Some("Type a file to the console"),
};

/// Called when the "dir" command is executed.
fn dir(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    fn work() -> Result<(), embedded_sdmmc::Error<bios::Error>> {
        osprintln!("Listing files on Block Device 0, /");
        let bios_block = crate::fs::BiosBlock();
        let time = crate::fs::BiosTime();
        let mut mgr = embedded_sdmmc::VolumeManager::new(bios_block, time);
        // Open the first partition
        let volume = mgr.open_volume(VolumeIdx(0))?;
        let root_dir = mgr.open_root_dir(volume)?;
        let mut total_bytes = 0u64;
        let mut num_files = 0;
        mgr.iterate_dir(root_dir, |dir_entry| {
            let padding = 8 - dir_entry.name.base_name().len();
            for b in dir_entry.name.base_name() {
                let ch = *b as char;
                osprint!("{}", if ch.is_ascii_graphic() { ch } else { '?' });
            }
            for _ in 0..padding {
                osprint!(" ");
            }
            osprint!(" ");
            let padding = 3 - dir_entry.name.extension().len();
            for b in dir_entry.name.extension() {
                let ch = *b as char;
                osprint!("{}", if ch.is_ascii_graphic() { ch } else { '?' });
            }
            for _ in 0..padding {
                osprint!(" ");
            }
            if dir_entry.attributes.is_directory() {
                osprint!(" <DIR>        ");
            } else {
                osprint!(" {:-13}", dir_entry.size,);
            }
            osprint!(
                " {:02}/{:02}/{:04}",
                dir_entry.mtime.zero_indexed_day + 1,
                dir_entry.mtime.zero_indexed_month + 1,
                u32::from(dir_entry.mtime.year_since_1970) + 1970
            );
            osprintln!(
                "  {:02}:{:02}",
                dir_entry.mtime.hours,
                dir_entry.mtime.minutes
            );
            total_bytes += dir_entry.size as u64;
            num_files += 1;
        })?;
        osprintln!("{:-9} file(s)  {:-13} bytes", num_files, total_bytes);
        Ok(())
    }

    match work() {
        Ok(_) => {}
        Err(e) => {
            osprintln!("Error: {:?}", e);
        }
    }
}

/// Called when the "load" command is executed.
fn load(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], ctx: &mut Ctx) {
    let Some(filename) = args.first() else {
        osprintln!("Need a filename");
        return;
    };
    match ctx.tpa.load_program(filename) {
        Ok(_) => {}
        Err(e) => {
            osprintln!("Error: {:?}", e);
        }
    }
}

/// Called when the "type" command is executed.
fn typefn(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], ctx: &mut Ctx) {
    fn work(ctx: &mut Ctx, filename: &str) -> Result<(), embedded_sdmmc::Error<bios::Error>> {
        let bios_block = crate::fs::BiosBlock();
        let time = crate::fs::BiosTime();
        let mut mgr = embedded_sdmmc::VolumeManager::new(bios_block, time);
        // Open the first partition
        let volume = mgr.open_volume(VolumeIdx(0))?;
        let root_dir = mgr.open_root_dir(volume)?;
        let file = mgr.open_file_in_dir(root_dir, filename, embedded_sdmmc::Mode::ReadOnly)?;
        let buffer = ctx.tpa.as_slice_u8();
        let count = mgr.read(file, buffer)?;
        if count != mgr.file_length(file)? as usize {
            osprintln!("File too large! Max {} bytes allowed.", buffer.len());
            return Ok(());
        }
        let Ok(s) = core::str::from_utf8(&buffer[0..count]) else {
            osprintln!("File is not valid UTF-8");
            return Ok(());
        };
        osprintln!("{}", s);
        Ok(())
    }

    // index can't panic - we always have enough args
    let r = work(ctx, args[0]);
    // reset SGR
    osprint!("\u{001b}[0m");
    match r {
        Ok(_) => {}
        Err(e) => {
            osprintln!("Error: {:?}", e);
        }
    }
}

// End of file
