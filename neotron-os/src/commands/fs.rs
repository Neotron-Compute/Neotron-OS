//! File Systems related commands for Neotron OS

use crate::{osprint, osprintln, Ctx, FILESYSTEM};

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

pub static EXEC_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: exec,
        parameters: &[menu::Parameter::Mandatory {
            parameter_name: "file",
            help: Some("The shell script to run"),
        }],
    },
    command: "exec",
    help: Some("Execute a shell script"),
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

pub static ROM_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: romfn,
        parameters: &[menu::Parameter::Optional {
            parameter_name: "file",
            help: Some("The ROM utility to run"),
        }],
    },
    command: "rom",
    help: Some("Run a program from ROM"),
};

/// Called when the "dir" command is executed.
fn dir(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    fn work() -> Result<(), crate::fs::Error> {
        osprintln!("Listing files on Block Device 0, /");
        let mut total_bytes = 0;
        let mut num_files = 0;
        FILESYSTEM.iterate_root_dir(|dir_entry| {
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
    if let Err(e) = ctx.tpa.load_program(filename) {
        osprintln!("Error: {:?}", e);
    }
}

/// Called when the "exec" command is executed.
fn exec(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], ctx: &mut Ctx) {
    fn work(ctx: &mut Ctx, filename: &str) -> Result<(), crate::fs::Error> {
        let file = FILESYSTEM.open_file(filename, embedded_sdmmc::Mode::ReadOnly)?;
        let buffer = ctx.tpa.as_slice_u8();
        let count = file.read(buffer)?;
        if count != file.length() as usize {
            osprintln!("File too large! Max {} bytes allowed.", buffer.len());
            return Ok(());
        }
        let Ok(s) = core::str::from_utf8(&buffer[0..count]) else {
            osprintln!("File is not valid UTF-8");
            return Ok(());
        };
        // tell the main loop to run from these bytes next
        ctx.exec_tpa = Some(s.len());
        Ok(())
    }

    // index can't panic - we always have enough args
    let r = work(ctx, args[0]);
    match r {
        Ok(_) => {}
        Err(e) => {
            osprintln!("Error: {:?}", e);
        }
    }
}

/// Called when the "type" command is executed.
fn typefn(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], ctx: &mut Ctx) {
    fn work(ctx: &mut Ctx, filename: &str) -> Result<(), crate::fs::Error> {
        let file = FILESYSTEM.open_file(filename, embedded_sdmmc::Mode::ReadOnly)?;
        let buffer = ctx.tpa.as_slice_u8();
        let count = file.read(buffer)?;
        if count != file.length() as usize {
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

/// Called when the "romfn" command is executed.
fn romfn(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], ctx: &mut Ctx) {
    let Ok(romfs) = neotron_romfs::RomFs::new(crate::ROMFS) else {
        osprintln!("No ROM available");
        return;
    };
    if let Some(arg) = args.get(0) {
        let Some(entry) = romfs.find(arg) else {
            osprintln!("Couldn't find {} in ROM", arg);
            return;
        };
        if let Err(e) = ctx.tpa.load_rom_program(entry.contents) {
            osprintln!("Error: {:?}", e);
        }
    } else {
        for entry in romfs.into_iter() {
            if let Ok(entry) = entry {
                osprintln!(
                    "{} ({} bytes)",
                    entry.metadata.file_name,
                    entry.metadata.file_size
                );
            }
        }
    }
}

// End of file
