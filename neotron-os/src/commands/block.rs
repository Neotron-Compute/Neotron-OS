//! Block Device related commands for Neotron OS

use super::{parse_u64, parse_u8};
use crate::{bios, osprint, osprintln, Ctx, API};

pub static READ_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: read_block,
        parameters: &[
            menu::Parameter::Mandatory {
                parameter_name: "device_idx",
                help: Some("The block device ID to fetch from"),
            },
            menu::Parameter::Mandatory {
                parameter_name: "block_idx",
                help: Some("The block to fetch, 0..num_blocks"),
            },
        ],
    },
    command: "readblk",
    help: Some("Display one disk block, as hex"),
};

/// Called when the "read_block" command is executed.
fn read_block(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], _ctx: &mut Ctx) {
    let api = API.get();
    let Ok(device_idx) = parse_u8(args[0]) else {
        osprintln!("Couldn't parse {:?}", args[0]);
        return;
    };
    let Ok(block_idx) = parse_u64(args[1]) else {
        osprintln!("Couldn't parse {:?}", args[1]);
        return;
    };
    osprintln!("Reading block {}:", block_idx);
    let mut buffer = [0u8; 512];
    match (api.block_read)(
        device_idx,
        bios::block_dev::BlockIdx(block_idx),
        1,
        bios::FfiBuffer::new(&mut buffer),
    ) {
        bios::ApiResult::Ok(_) => {
            // Carry on
            let mut count = 0;
            for chunk in buffer.chunks(32) {
                osprint!("{:03x}: ", count);
                for b in chunk {
                    osprint!("{:02x}", *b);
                }
                count += chunk.len();
                osprintln!();
            }
        }
        bios::ApiResult::Err(e) => {
            osprintln!("Failed to read: {:?}", e);
        }
    }
}

// End of file
