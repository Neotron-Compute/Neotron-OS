//! CLI commands for getting/setting time/date

use chrono::{Datelike, Timelike};

use crate::{osprintln, Ctx, API};

pub static DATE_ITEM: menu::Item<Ctx> = menu::Item {
    item_type: menu::ItemType::Callback {
        function: date,
        parameters: &[menu::Parameter::Optional {
            parameter_name: "timestamp",
            help: Some("The new date/time, in ISO8601 format"),
        }],
    },
    command: "date",
    help: Some("Get/set the time and date"),
};

/// Called when the "date" command is executed.
fn date(_menu: &menu::Menu<Ctx>, item: &menu::Item<Ctx>, args: &[&str], _ctx: &mut Ctx) {
    if let Ok(Some(timestamp)) = menu::argument_finder(item, args, "timestamp") {
        osprintln!("Setting date/time to {:?}", timestamp);
        static DATE_FMT: &str = "%Y-%m-%dT%H:%M:%S";
        let Ok(timestamp) = chrono::NaiveDateTime::parse_from_str(timestamp, DATE_FMT) else {
            osprintln!("Unable to parse date/time");
            return;
        };
        API.set_time(timestamp);
    }

    let time = API.get_time();
    // Ensure this matches `DATE_FMT`, for consistency
    osprintln!(
        "The time is {:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}",
        time.year(),
        time.month(),
        time.day(),
        time.hour(),
        time.minute(),
        time.second(),
        time.nanosecond()
    );
}

// End of file
