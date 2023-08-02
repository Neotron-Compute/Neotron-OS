//! Sound related commands for Neotron OS

use crate::{osprintln, Ctx, API};

pub static MIXER_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: mixer,
        parameters: &[],
    },
    command: "mixer",
    help: Some("Control the audio mixer"),
};

/// Called when the "mixer" command is executed.
fn mixer(_menu: &menu::Menu<Ctx>, _item: &menu::Item<Ctx>, _args: &[&str], _ctx: &mut Ctx) {
    let api = API.get();
    osprintln!("Mixers:");
    for mixer_id in 0u8..=255u8 {
        match (api.audio_mixer_channel_get_info)(mixer_id) {
            neotron_common_bios::FfiOption::Some(mixer_info) => {
                let dir_str = match mixer_info.direction {
                    neotron_common_bios::audio::Direction::Input => "In",
                    neotron_common_bios::audio::Direction::Loopback => "Loop",
                    neotron_common_bios::audio::Direction::Output => "Out",
                };
                osprintln!(
                    "#{}: {} ({}) {}/{}",
                    mixer_id,
                    mixer_info.name,
                    dir_str,
                    mixer_info.current_level,
                    mixer_info.max_level
                );
            }
            neotron_common_bios::FfiOption::None => {
                // Run out of mixers
                break;
            }
        }
    }
}
