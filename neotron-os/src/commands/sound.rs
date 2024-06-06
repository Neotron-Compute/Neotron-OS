//! Sound related commands for Neotron OS

use crate::{bios, osprint, osprintln, Ctx, API, FILESYSTEM};

pub static MIXER_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: mixer,
        parameters: &[
            menu::Parameter::Optional {
                parameter_name: "mixer",
                help: Some("Which mixer to adjust"),
            },
            menu::Parameter::Optional {
                parameter_name: "level",
                help: Some("New level for this mixer, as an integer."),
            },
        ],
    },
    command: "mixer",
    help: Some("Control the audio mixer"),
};

pub static PLAY_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: play,
        parameters: &[menu::Parameter::Mandatory {
            parameter_name: "filename",
            help: Some("Which file to play"),
        }],
    },
    command: "play",
    help: Some("Play a raw 16-bit LE 48 kHz stereo file"),
};

/// Called when the "mixer" command is executed.
fn mixer(_menu: &menu::Menu<Ctx>, item: &menu::Item<Ctx>, args: &[&str], _ctx: &mut Ctx) {
    let selected_mixer = menu::argument_finder(item, args, "mixer").unwrap();
    let level_str = menu::argument_finder(item, args, "level").unwrap();

    let level_int = if let Some(level_str) = level_str {
        let Ok(value) = level_str.parse::<u8>() else {
            osprintln!("{} is not an integer", level_str);
            return;
        };
        Some(value)
    } else {
        None
    };

    let mixer_int = selected_mixer.and_then(|n| n.parse::<u8>().ok());

    let api = API.get();

    if let (Some(selected_mixer), Some(level_int)) = (selected_mixer, level_int) {
        let mut found = false;
        for mixer_id in 0u8..=255u8 {
            match (api.audio_mixer_channel_get_info)(mixer_id) {
                bios::FfiOption::Some(mixer_info) => {
                    if (Some(mixer_id) == mixer_int) || (mixer_info.name.as_str() == selected_mixer)
                    {
                        if let Err(e) =
                            (api.audio_mixer_channel_set_level)(mixer_id, level_int).into()
                        {
                            osprintln!(
                                "Failed to set mixer {:?} (id {}) to {}: {:?}",
                                selected_mixer,
                                mixer_id,
                                level_int,
                                e
                            );
                        }
                        found = true;
                        break;
                    }
                }
                bios::FfiOption::None => {
                    break;
                }
            }
        }

        if !found {
            osprintln!("Don't know mixer {:?}", selected_mixer);
        }
    }

    osprintln!("Mixers:");
    for mixer_id in 0u8..=255u8 {
        match (api.audio_mixer_channel_get_info)(mixer_id) {
            bios::FfiOption::Some(mixer_info) => {
                let dir_str = match mixer_info.direction.make_safe() {
                    Ok(bios::audio::Direction::Input) => "In",
                    Ok(bios::audio::Direction::Loopback) => "Loop",
                    Ok(bios::audio::Direction::Output) => "Out",
                    _ => "??",
                };
                if (Some(mixer_id) == mixer_int)
                    || selected_mixer
                        .map(|s| s == mixer_info.name.as_str())
                        .unwrap_or(true)
                {
                    osprintln!(
                        "\t{}: {} ({}) {}/{}",
                        mixer_id,
                        mixer_info.name,
                        dir_str,
                        mixer_info.current_level,
                        mixer_info.max_level
                    );
                }
            }
            bios::FfiOption::None => {
                // Run out of mixers
                break;
            }
        }
    }
}

/// Called when the "play" command is executed.
fn play(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], ctx: &mut Ctx) {
    fn play_inner(file_name: &str, scratch: &mut [u8]) -> Result<(), crate::fs::Error> {
        osprintln!("Loading /{} from Block Device 0", file_name);
        let file = FILESYSTEM.open_file(file_name, embedded_sdmmc::Mode::ReadOnly)?;

        osprintln!("Press Q to quit, P to pause/unpause...");

        let api = API.get();

        let buffer = &mut scratch[0..4096];
        let mut bytes = 0;
        let mut delta = 0;

        let mut pause = false;

        'playback: while !file.is_eof() {
            if !pause {
                let bytes_read = file.read(buffer)?;
                let mut buffer = &buffer[0..bytes_read];
                while !buffer.is_empty() {
                    let slice = bios::FfiByteSlice::new(buffer);
                    let played = unsafe { (api.audio_output_data)(slice).unwrap() };
                    buffer = &buffer[played..];
                    delta += played;
                    if delta > 48000 {
                        bytes += delta;
                        delta = 0;
                        let milliseconds = bytes / ((48000 / 1000) * 4);
                        osprint!(
                            "\rPlayed: {}.{:03} s",
                            milliseconds / 1000,
                            milliseconds % 1000
                        );
                    }
                }
            }

            let mut buffer = [0u8; 16];
            let count = { crate::STD_INPUT.lock().get_data(&mut buffer) };
            for b in &buffer[0..count] {
                if *b == b'q' || *b == b'Q' {
                    osprintln!("\nQuitting playback!");
                    break 'playback;
                } else if (*b == b'p' || *b == b'P') && pause {
                    pause = false;
                } else if (*b == b'p' || *b == b'P') && !pause {
                    let milliseconds = bytes / ((48000 / 1000) * 4);
                    osprint!(
                        "\rPaused: {}.{:03} s",
                        milliseconds / 1000,
                        milliseconds % 1000
                    );
                    pause = true;
                }
            }
        }
        osprintln!();
        Ok(())
    }

    if let Err(e) = play_inner(args[0], ctx.tpa.as_slice_u8()) {
        osprintln!("\nError during playback: {:?}", e);
    }
}

// End of file
