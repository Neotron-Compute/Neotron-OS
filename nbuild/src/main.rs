//! A series of utilities for building Neotron OS

use clap::{Parser, Subcommand};

#[derive(Debug, Subcommand)]
enum Commands {
    /// Builds the OS and the ROMFS
    Binary {
        /// The start address in Flash where Neotron OS should live
        #[clap(long, default_value = "0x1000_0000")]
        start_address: String,
        /// The target we're building Neotron OS for
        #[clap(long, default_value = "thumbv6m-none-eabi")]
        target: String,
    },
    /// Builds the OS as a library, for the native machine
    Library {
        /// The target we're building Neotron OS for
        #[clap(long)]
        target: Option<String>,
    },
    /// Handles formatting of the Neotron OS source code
    Format {
        /// Whether to just check the formatting
        #[clap(long)]
        check: bool,
    },
}

/// A simple utility for building Neotron OS and a suitable ROMFS image
#[derive(Debug, Parser)]
#[clap(name = "nbuild", version = "0.1.0", author = "The Neotron Developers")]
pub struct NBuildApp {
    /// The task to perform
    #[command(subcommand)]
    command: Option<Commands>,
}

fn main() {
    println!("Neotron OS nbuild tool");
    let args = NBuildApp::parse();
    match args.command {
        None => {
            // No command given
            println!("No command given. Try `cargo nbuild help`.");
            std::process::exit(1);
        }
        Some(Commands::Binary {
            start_address,
            target,
        }) => {
            let Ok(start_address) = nbuild::parse_int(&start_address) else {
                eprintln!("{:?} was not a valid integer", start_address);
                std::process::exit(1);
            };
            println!(
                "Cross-compiling Neotron OS binaries, using start address 0x{:08x} and target {:?}",
                start_address, target
            );
            if let Err(e) = nbuild::cargo(
                &["build", "--bins", "--release"],
                Some(&target),
                "./neotron-os/Cargo.toml",
            ) {
                eprintln!("Build failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Library { target }) => {
            let target = target.as_deref();
            println!(
                "Compiling Neotron OS library, using target {:?}",
                target.unwrap_or("native")
            );
            if let Err(e) = nbuild::cargo(&["build", "--lib"], target, "./neotron-os/Cargo.toml") {
                eprintln!("Build failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Format { check }) => {
            let mut is_error = false;
            let commands = if check {
                vec!["fmt", "--check"]
            } else {
                vec!["fmt"]
            };
            println!("Formatting Neotron OS");
            if let Err(e) = nbuild::cargo(&commands, None, "./neotron-os/Cargo.toml") {
                eprintln!("Format failed: {}", e);
                is_error = true;
            }
            if let Err(e) = nbuild::cargo(&commands, None, "./nbuild/Cargo.toml") {
                eprintln!("Format failed: {}", e);
                is_error = true;
            }
            if is_error {
                std::process::exit(1);
            }
        }
    }
}

// End of file
