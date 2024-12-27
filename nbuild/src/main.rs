//! A series of utilities for building Neotron OS

use clap::{Parser, Subcommand};

#[derive(Debug, Subcommand)]
enum Commands {
    /// Builds the OS and the ROMFS
    Binary {
        /// The start address in Flash where Neotron OS should live
        #[clap(long, default_value = "0x1002_0000")]
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
    /// Checks the Neotron OS source code using clippy
    Clippy,
    /// Runs any tests
    Test,
}

/// A simple utility for building Neotron OS and a suitable ROMFS image
#[derive(Debug, Parser)]
#[clap(name = "nbuild", version = "0.1.0", author = "The Neotron Developers")]
pub struct NBuildApp {
    /// The task to perform
    #[command(subcommand)]
    command: Option<Commands>,
}

fn packages() -> Vec<nbuild::Package> {
    vec![
        // *** build system ***
        nbuild::Package {
            name: "nbuild",
            path: std::path::Path::new("./nbuild/Cargo.toml"),
            output_template: None,
            kind: nbuild::PackageKind::NBuild,
            testable: nbuild::Testable::All,
        },
        // *** utilities ***
        nbuild::Package {
            name: "flames",
            path: std::path::Path::new("./utilities/flames/Cargo.toml"),
            output_template: Some("./target/{target}/{profile}/flames"),
            kind: nbuild::PackageKind::Utility,
            testable: nbuild::Testable::No,
        },
        nbuild::Package {
            name: "neoplay",
            path: std::path::Path::new("./utilities/neoplay/Cargo.toml"),
            output_template: Some("./target/{target}/{profile}/neoplay"),
            kind: nbuild::PackageKind::Utility,
            testable: nbuild::Testable::No,
        },
        nbuild::Package {
            name: "snake",
            path: std::path::Path::new("./utilities/snake/Cargo.toml"),
            output_template: Some("./target/{target}/{profile}/snake"),
            kind: nbuild::PackageKind::Utility,
            testable: nbuild::Testable::No,
        },
        nbuild::Package {
            name: "vidtest",
            path: std::path::Path::new("./utilities/vidtest/Cargo.toml"),
            output_template: Some("./target/{target}/{profile}/vidtest"),
            kind: nbuild::PackageKind::Utility,
            testable: nbuild::Testable::No,
        },
        // *** OS ***
        nbuild::Package {
            name: "Neotron OS",
            path: std::path::Path::new("./neotron-os/Cargo.toml"),
            output_template: Some("./target/{target}/{profile}/neotron-os"),
            kind: nbuild::PackageKind::Os,
            testable: nbuild::Testable::Libs,
        },
    ]
}

fn main() {
    println!("Neotron OS nbuild tool");
    let args = NBuildApp::parse();
    let packages = packages();
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
            binary(&packages, &start_address, &target);
        }
        Some(Commands::Library { target }) => library(&packages, target.as_deref()),
        Some(Commands::Format { check }) => format(&packages, check),
        Some(Commands::Clippy) => clippy(&packages),
        Some(Commands::Test) => test(&packages),
    }
}

/// Builds the utility and OS packages as binaries
fn binary(packages: &[nbuild::Package], start_address: &str, target: &str) {
    use chrono::{Datelike, Timelike};

    let mut is_error = false;
    let Ok(start_address) = nbuild::parse_int(start_address) else {
        eprintln!("{:?} was not a valid integer", start_address);
        std::process::exit(1);
    };

    let mut romfs_entries = Vec::new();
    // Build utilities
    for package in packages
        .iter()
        .filter(|p| p.kind == nbuild::PackageKind::Utility)
    {
        println!(
            "Cross-compiling {}, using target {:?}",
            package.name, target
        );
        if let Err(e) = nbuild::cargo(&["build", "--release"], Some(target), package.path) {
            eprintln!("Build of {} failed: {}", package.name, e);
            is_error = true;
        }
        let package_output = package
            .output(target, "release")
            .expect("utilties should have an output");
        let contents = match std::fs::read(&package_output) {
            Ok(contents) => contents,
            Err(e) => {
                eprintln!("Reading of {} failed: {}", package_output, e);
                continue;
            }
        };
        let ctime = std::time::SystemTime::now();
        let ctime = chrono::DateTime::<chrono::Utc>::from(ctime);
        romfs_entries.push(neotron_romfs::Entry {
            metadata: neotron_romfs::EntryMetadata {
                file_name: package.name,
                ctime: neotron_api::file::Time {
                    year_since_1970: (ctime.year() - 1970) as u8,
                    zero_indexed_month: ctime.month0() as u8,
                    zero_indexed_day: ctime.day0() as u8,
                    hours: ctime.hour() as u8,
                    minutes: ctime.minute() as u8,
                    seconds: ctime.second() as u8,
                },
                file_size: contents.len() as u32,
            },
            contents,
        });
    }

    // Build ROMFS
    let mut buffer = Vec::new();
    let _size = match neotron_romfs::RomFs::construct_into(&mut buffer, &romfs_entries) {
        Ok(size) => size,
        Err(e) => {
            eprintln!("Making ROMFS failed: {:?}", e);
            std::process::exit(1);
        }
    };
    let mut romfs_path = std::path::PathBuf::new();
    romfs_path.push(std::env::current_dir().expect("We have no CWD?"));
    romfs_path.push("target");
    romfs_path.push(target);
    romfs_path.push("release");
    romfs_path.push("romfs.bin");
    if let Err(e) = std::fs::write(&romfs_path, &buffer) {
        eprintln!("Writing ROMFS to {} failed: {:?}", romfs_path.display(), e);
        std::process::exit(1);
    }
    println!("Built ROMFS at {}", romfs_path.display());

    // Build OS
    for package in packages
        .iter()
        .filter(|p| p.kind == nbuild::PackageKind::Os)
    {
        println!(
            "Cross-compiling {}, using start address 0x{:08x} and target {:?}",
            package.name, start_address, target
        );
        let environment = [
            (
                "NEOTRON_OS_START_ADDRESS",
                format!("0x{:08x}", start_address),
            ),
            ("ROMFS_PATH", romfs_path.to_string_lossy().to_string()),
        ];
        if let Err(e) = nbuild::cargo_with_env(
            &["build", "--release"],
            Some(target),
            package.path,
            &environment,
        ) {
            eprintln!("Build of {} failed: {}", package.name, e);
            is_error = true;
        }
        let package_output = package
            .output(target, "release")
            .expect("PackageKind::Os should always have output");
        if let Err(e) = nbuild::make_bin(&package_output) {
            eprintln!("objcopy of {} failed: {}", package_output, e);
            is_error = true;
        }
    }
    if is_error {
        std::process::exit(1);
    }
}

/// Builds the OS packages as a library
fn library(packages: &[nbuild::Package], target: Option<&str>) {
    let mut is_error = false;
    println!(
        "Compiling Neotron OS library, using target {:?}",
        target.unwrap_or("native")
    );
    for package in packages
        .iter()
        .filter(|p| p.kind == nbuild::PackageKind::Os)
    {
        println!(
            "Compiling {}, target {:?}",
            package.name,
            target.unwrap_or("native")
        );
        if let Err(e) = nbuild::cargo(&["build", "--release", "--lib"], target, package.path) {
            eprintln!("Build of {} failed: {}", package.name, e);
            is_error = true;
        }
    }
    if is_error {
        std::process::exit(1);
    }
}

/// Runs `cargo fmt` over all the packages
fn format(packages: &[nbuild::Package], check: bool) {
    let mut is_error = false;
    let commands = if check {
        vec!["fmt", "--check"]
    } else {
        vec!["fmt"]
    };
    for package in packages.iter() {
        println!("Formatting {}", package.name);
        if let Err(e) = nbuild::cargo(&commands, None, package.path) {
            eprintln!("Format failed: {}", e);
            is_error = true;
        }
    }
    if is_error {
        std::process::exit(1);
    }
}

/// Runs `cargo clippy` over all the packages
fn clippy(packages: &[nbuild::Package]) {
    let mut is_error = false;
    for package in packages.iter() {
        println!("Linting {} with clippy", package.name);
        if let Err(e) = nbuild::cargo(&["clippy"], None, package.path) {
            eprintln!("Lint failed: {}", e);
            is_error = true;
        }
    }
    if is_error {
        std::process::exit(1);
    }
}

/// Runs `cargo test` over all the packages
fn test(packages: &[nbuild::Package]) {
    let mut is_error = false;
    for package in packages
        .iter()
        .filter(|p| p.testable == nbuild::Testable::Libs)
    {
        println!("Testing {}", package.name);
        if let Err(e) = nbuild::cargo(&["test", "--lib"], None, package.path) {
            eprintln!("Test failed: {}", e);
            is_error = true;
        }
    }
    for package in packages
        .iter()
        .filter(|p| p.testable == nbuild::Testable::All)
    {
        println!("Testing {}", package.name);
        if let Err(e) = nbuild::cargo(&["test"], None, package.path) {
            eprintln!("Test failed: {}", e);
            is_error = true;
        }
    }
    if is_error {
        std::process::exit(1);
    }
}

// End of file
