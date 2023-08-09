//! Sound related commands for Neotron OS

use crate::{osprint, osprintln, Ctx, API};

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

    let api = API.get();

    if let (Some(selected_mixer), Some(level_int)) = (selected_mixer, level_int) {
        let mut found = false;
        for mixer_id in 0u8..=255u8 {
            match (api.audio_mixer_channel_get_info)(mixer_id) {
                neotron_common_bios::FfiOption::Some(mixer_info) => {
                    if mixer_info.name.as_str() == selected_mixer {
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
                neotron_common_bios::FfiOption::None => {
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
            neotron_common_bios::FfiOption::Some(mixer_info) => {
                let dir_str = match mixer_info.direction {
                    neotron_common_bios::audio::Direction::Input => "In",
                    neotron_common_bios::audio::Direction::Loopback => "Loop",
                    neotron_common_bios::audio::Direction::Output => "Out",
                };
                if selected_mixer
                    .and_then(|s| Some(s == mixer_info.name.as_str()))
                    .unwrap_or(true)
                {
                    osprintln!(
                        "#{}: {} ({}) {}/{}",
                        mixer_id,
                        mixer_info.name,
                        dir_str,
                        mixer_info.current_level,
                        mixer_info.max_level
                    );
                }
            }
            neotron_common_bios::FfiOption::None => {
                // Run out of mixers
                break;
            }
        }
    }
}

/// Called when the "play" command is executed.
fn play(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], ctx: &mut Ctx) {
    fn play_inner(
        file_name: &str,
        scratch: &mut [u8],
    ) -> Result<(), embedded_sdmmc::Error<neotron_common_bios::Error>> {
        osprintln!("Loading /{} from Block Device 0", file_name);
        let bios_block = crate::fs::BiosBlock();
        let time = crate::fs::BiosTime();
        let mut mgr = embedded_sdmmc::VolumeManager::new(bios_block, time);
        // Open the first partition
        let mut volume = mgr.get_volume(embedded_sdmmc::VolumeIdx(0))?;
        let root_dir = mgr.open_root_dir(&volume)?;
        let mut file = mgr.open_file_in_dir(
            &mut volume,
            &root_dir,
            file_name,
            embedded_sdmmc::Mode::ReadOnly,
        )?;

        let api = API.get();

        let mut buffer = &mut scratch[0..4096];
        let mut bytes = 0;
        let mut delta = 0;

        while !file.eof() {
            let bytes_read = mgr.read(&mut volume, &mut file, &mut buffer)?;
            let mut buffer = &buffer[0..bytes_read];
            while !buffer.is_empty() {
                let slice = neotron_common_bios::FfiByteSlice::new(buffer);
                let played = unsafe { (api.audio_output_data)(slice).unwrap() };
                buffer = &buffer[played..];
                delta += played;
                if delta > 48000 {
                    bytes += delta;
                    delta = 0;
                    let milliseconds = bytes / ((48000 / 1000) * 4);
                    osprint!(
                        "\rPlayed: {}:{} ms",
                        milliseconds / 1000,
                        milliseconds % 1000
                    );
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
