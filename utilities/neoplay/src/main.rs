#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

use core::{fmt::Write, ptr::addr_of_mut};

const FILE_BUFFER_LEN: usize = 192 * 1024;
static mut FILE_BUFFER: [u8; FILE_BUFFER_LEN] = [0u8; FILE_BUFFER_LEN];

mod player;

#[cfg(not(target_os = "none"))]
fn main() {
    neotron_sdk::init();
}

#[no_mangle]
extern "C" fn neotron_main() -> i32 {
    if let Err(e) = real_main() {
        let mut stdout = neotron_sdk::stdout();
        let _ = writeln!(stdout, "Error: {:?}", e);
        1
    } else {
        0
    }
}

fn real_main() -> Result<(), neotron_sdk::Error> {
    let mut stdout = neotron_sdk::stdout();
    let stdin = neotron_sdk::stdin();
    let Some(filename) = neotron_sdk::arg(0) else {
        return Err(neotron_sdk::Error::InvalidArg);
    };
    let _ = writeln!(stdout, "Loading {:?}...", filename);
    let path = neotron_sdk::path::Path::new(&filename)?;
    let f = neotron_sdk::File::open(path, neotron_sdk::Flags::empty())?;
    let file_buffer = unsafe {
        let file_buffer = &mut *addr_of_mut!(FILE_BUFFER);
        let n = f.read(file_buffer)?;
        &file_buffer[0..n]
    };
    drop(f);
    // Set 16-bit stereo, 44.1 kHz
    let dsp_path = neotron_sdk::path::Path::new("AUDIO:")?;
    let dsp = neotron_sdk::File::open(dsp_path, neotron_sdk::Flags::empty())?;
    if dsp.ioctl(1, 3 << 60 | 44100).is_err() {
        let _ = writeln!(stdout, "Failed to configure audio");
        return neotron_sdk::Result::Err(neotron_sdk::Error::DeviceSpecific);
    }

    let mut player = match player::Player::new(file_buffer, 44100) {
        Ok(player) => player,
        Err(e) => {
            let _ = writeln!(stdout, "Failed to create player: {:?}", e);
            return Err(neotron_sdk::Error::InvalidArg);
        }
    };

    let _ = writeln!(stdout, "Playing {:?}...", filename);
    let mut sample_buffer = [0u8; 1024];
    // loop some some silence to give us a head-start
    for _i in 0..11 {
        let _ = dsp.write(&sample_buffer);
    }

    loop {
        for chunk in sample_buffer.chunks_exact_mut(4) {
            let (left, right) = player.next_sample(&mut stdout);
            let left_bytes = left.to_le_bytes();
            let right_bytes = right.to_le_bytes();
            chunk[0] = left_bytes[0];
            chunk[1] = left_bytes[1];
            chunk[2] = right_bytes[0];
            chunk[3] = right_bytes[1];
        }
        let _ = dsp.write(&sample_buffer);
        let mut in_buf = [0u8; 1];
        if player.is_finished() {
            break;
        }
        if stdin.read(&mut in_buf).is_ok() && in_buf[0].to_ascii_lowercase() == b'q' {
            break;
        }
    }

    let _ = writeln!(stdout, "Bye!");

    Ok(())
}
