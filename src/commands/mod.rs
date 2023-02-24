//! Commands for Neotron OS
//!
//! Defines the top-level menu, and the commands it can call.

pub use super::Ctx;

mod config;
mod hardware;
mod input;
mod screen;

pub static OS_MENU: menu::Menu<Ctx> = menu::Menu {
    label: "root",
    items: &[
        &menu::Item {
            item_type: menu::ItemType::Callback {
                function: hardware::lshw,
                parameters: &[],
            },
            command: "lshw",
            help: Some("List all the hardware"),
        },
        &menu::Item {
            item_type: menu::ItemType::Callback {
                function: screen::clear,
                parameters: &[],
            },
            command: "clear",
            help: Some("Clear the screen"),
        },
        &menu::Item {
            item_type: menu::ItemType::Callback {
                function: screen::fill,
                parameters: &[],
            },
            command: "fill",
            help: Some("Fill the screen with characters"),
        },
        &menu::Item {
            item_type: menu::ItemType::Callback {
                function: config::command,
                parameters: &[
                    menu::Parameter::Optional {
                        parameter_name: "command",
                        help: Some("Which operation to perform (try help)"),
                    },
                    menu::Parameter::Optional {
                        parameter_name: "value",
                        help: Some("new value for the setting"),
                    },
                ],
            },
            command: "config",
            help: Some("Handle non-volatile OS configuration"),
        },
        &menu::Item {
            item_type: menu::ItemType::Callback {
                function: input::kbtest,
                parameters: &[],
            },
            command: "kbtest",
            help: Some("Test the keyboard (press ESC to quit)"),
        },
    ],
    entry: None,
    exit: None,
};
