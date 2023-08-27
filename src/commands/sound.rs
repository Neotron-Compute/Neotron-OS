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

pub static MP3_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: playmp3,
        parameters: &[menu::Parameter::Mandatory {
            parameter_name: "filename",
            help: Some("Which file to play"),
        }],
    },
    command: "mp3",
    help: Some("Play an MP3 file"),
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

/// Called when the "play" command is executed.
fn playmp3(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, args: &[&str], ctx: &mut Ctx) {
    use core::slice::Chunks;
    use picomp3lib_rs::Mp3;
    const BUFF_SZ: usize = 512*2;
    const CHUNK_SZ: usize = 512;
    #[derive(Debug)]
    struct Buffer {
        pub mp3_byte_buffer: [u8; BUFF_SZ],
        pub buff_start: usize,
        pub buff_end: usize,
    }
    use core::fmt;
    impl fmt::Display for Buffer {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(
                f,
                "start:{} end:{} used:{} avail:{} tail:{}",
                self.buff_start,
                self.buff_end,
                self.used(),
                self.available(),
                self.tail_free()
            )
        }
    }

    impl Buffer {
        pub fn new() -> Self {
            Self {
                mp3_byte_buffer: [0u8; BUFF_SZ],
                buff_start: 0,
                buff_end: 0,
            }
        }

        /// How much data is in the buffer
        pub fn used(&self) -> usize {
            self.buff_end - self.buff_start
        }

        /// How much space is free in the buffer
        pub fn available(&self) -> usize {
            BUFF_SZ - self.used()
        }

        /// How much space is free at the end of the buffer
        pub fn tail_free(&self) -> usize {
            BUFF_SZ - self.buff_end
        }

        /// Shuffle all bytes along so that start of buffer == start of data
        pub fn remove_unused(&mut self) {
            if self.buff_start != 0 {
                let used: usize = self.used();
                for i in 0..used {
                    self.mp3_byte_buffer[i] = self.mp3_byte_buffer[i + self.buff_start];
                }
                self.buff_start = 0;
                self.buff_end = used;
            }
        }

        /// Using the provided iterator, load more data into the buffer
        pub fn load_more(&mut self, loader: &mut Chunks<'_, u8>) {
            self.remove_unused();
            while self.available() >= CHUNK_SZ {
                let newdata = loader.next();
                match newdata {
                    Some(d) => {
                        for i in 0..d.len() {
                            self.mp3_byte_buffer[self.buff_end] = d[i];
                            self.buff_end += 1;
                        }
                    }
                    None => {
                        return;
                    }
                }
            }
        }

        /// Using the provided slice, load more data into the buffer.
        /// Returns the number of bytes consumed
        pub fn load_slice(&mut self, data: &[u8]) -> usize {
            self.remove_unused();

            let loadsize = usize::min(self.tail_free(), data.len());
            for i in 0..loadsize {
                self.mp3_byte_buffer[self.buff_end] = data[i];
                self.buff_end += 1;
            }
            loadsize
        }

        /// Increment our "start pointer". use this as you consume slices from the start
        pub fn increment_start(&mut self, increment: usize) {
            self.buff_start += increment;
            self.remove_unused();
        }

        /// Return a slice over the remaining data in the buffer
        pub fn get_slice(&self) -> &[u8] {
            &self.mp3_byte_buffer[self.buff_start..self.buff_end]
        }
    }

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
        let (mut filebuf, scratch) = scratch.split_at_mut(512);
        let (mut buffer, scratch) = scratch.split_at_mut(8196);
        let (buffer2, _scratch) = scratch.split_at_mut(8196 * 2);

        let mut mp3dec = Mp3::new();
        let mut mp3_file_buffer = Buffer::new();
        osprintln!("\r {}", mp3_file_buffer);
        // load initial data - this should indicate max file read size as well
        let read_size = {
            let bytes_read = mgr.read(&mut volume, &mut file, &mut filebuf)?;
            let mp3_written = mp3_file_buffer.load_slice(&filebuf[0..bytes_read]);
            if bytes_read != mp3_written {
                osprintln!("mp3_file_buffer didn't have enough space, dropping bytes");
            }
            bytes_read
        };
        osprintln!("\r {}", mp3_file_buffer);
        // fill mp3_file_buffer as much as possible on first pass so we can read + skip id3
        while mp3_file_buffer.available() >= read_size {
            let bytes_read = mgr.read(&mut volume, &mut file, &mut filebuf)?;
            let mp3_written = mp3_file_buffer.load_slice(&filebuf[0..bytes_read]);
            if bytes_read != mp3_written {
                osprintln!("mp3_file_buffer didn't have enough space, dropping bytes");
            }
        }
        let start = Mp3::find_sync_word(mp3_file_buffer.get_slice()) as usize;
        mp3_file_buffer.increment_start(start);
        let mut frame = mp3dec
            .get_next_frame_info(mp3_file_buffer.get_slice())
            .unwrap();
        osprintln!("info: {:?}", frame);
        osprintln!("\r {}", mp3_file_buffer);

        let mut break_early = false;
        while !file.eof() && !break_early {
            // load another chunk if there is space in the mp3 file buffer
            if mp3_file_buffer.available() > read_size {
                let bytes_read = mgr.read(&mut volume, &mut file, &mut filebuf)?;
                let mp3_written = mp3_file_buffer.load_slice(&filebuf[0..bytes_read]);
                if bytes_read != mp3_written {
                    osprintln!("mp3_file_buffer didn't have enough space, dropping bytes");
                }
            }

            let newlen = mp3_file_buffer.used();
            let oldlen = newlen;
            let audio_out_i16_ptr =
                unsafe { core::mem::transmute::<&mut [u8], &mut [i16]>(buffer) };
            match mp3dec.decode(
                mp3_file_buffer.get_slice(),
                newlen as i32,
                audio_out_i16_ptr,
            ) {
                Ok(newlen) => {
                    let consumed = oldlen as usize - newlen as usize;
                    if consumed > mp3_file_buffer.used() {
                        osprintln!("huh. out of data.");
                        break_early = true;
                    }
                    mp3_file_buffer.increment_start(consumed);
                    // osprintln!("buffer: {}", mp3_file_buffer);
                }
                Err(e) => {
                    if e == picomp3lib_rs::DecodeErr::InDataUnderflow {
                        osprintln!("ran out of data while decoding");
                        let bytes_read = mgr.read(&mut volume, &mut file, &mut buffer)?;
                        let _mp3_written = mp3_file_buffer.load_slice(&filebuf[0..bytes_read]);
                        break_early = true;
                    }
                }
            }
            // get info about the last frame decoded
            frame = mp3dec.get_last_frame_info();

            // codec doesn't support any samplerate except for 48khz stereo yet.
            // do some simple frame doubling to make audio performance possible
            let framedouble = frame.samprate <= 24000 || frame.nChans == 1;
            let bytes_read = (frame.outputSamps) as usize * 2;
            let buffer = &buffer[0..bytes_read];

            // double all samples for now. this works best with a mono mp3
            if framedouble {
                for i in 0..(frame.outputSamps/2) as usize {
                    let in_offset = 4 * i;
                    let out_offset = 8 * i;
                    // left
                    buffer2[out_offset] = buffer[in_offset + 0];
                    buffer2[out_offset + 1] = buffer[in_offset + 1];
                    // right
                    buffer2[out_offset + 2] = buffer[in_offset + 2];
                    buffer2[out_offset + 3] = buffer[in_offset + 3];
                    // left
                    buffer2[out_offset + 4] = buffer[in_offset + 0];
                    buffer2[out_offset + 5] = buffer[in_offset + 1];
                    // right
                    buffer2[out_offset + 6] = buffer[in_offset + 2];
                    buffer2[out_offset + 7] = buffer[in_offset + 3];
                }
            }

            let mut buffer3 = if framedouble {
                &buffer2[0..(bytes_read * 2)]
            } else {
                &buffer[0..(bytes_read)]
            };

            while !buffer3.is_empty() && !break_early {
                let slice = neotron_common_bios::FfiByteSlice::new(buffer3);
                let played = unsafe { (api.audio_output_data)(slice).unwrap() };
                buffer3 = &buffer3[played..];
            }
        }
        osprintln!();
        Ok(())
    }
    if let Err(e) = play_inner(args[0], ctx.tpa.as_slice_u8()) {
        osprintln!("\nError during playback: {:?}", e);
    }
}
