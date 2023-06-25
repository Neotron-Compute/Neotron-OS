//! File Systems related commands for Neotron OS

use embedded_sdmmc::VolumeIdx;

use crate::{bios, print, println, Ctx};

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

/// Called when the "dir" command is executed.
fn dir(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    fn work() -> Result<(), embedded_sdmmc::Error<bios::Error>> {
        println!("Listing files on Block Device 0, /");
        let bios_block = crate::fs::BiosBlock();
        let time = crate::fs::BiosTime();
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
fn load(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], ctx: &mut Ctx) {
    let Some(filename) = args.first() else {
        println!("Need a filename");
        return;
    };
    match ctx.tpa.load_program(filename) {
        Ok(_) => {}
        Err(e) => {
            println!("Error: {:?}", e);
        }
    }
}
