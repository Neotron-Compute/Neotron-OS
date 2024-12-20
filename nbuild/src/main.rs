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
    Libraries {
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
        nbuild::Package {
            name: "nbuild",
            path: std::path::Path::new("./nbuild/Cargo.toml"),
            output: std::path::Path::new("./nbuild/target/debug/nbuild{exe}"),
            kind: nbuild::PackageKind::NBuild,
        },
        nbuild::Package {
            name: "flames utility",
            path: std::path::Path::new("./utilities/flames/Cargo.toml"),
            output: std::path::Path::new("./target/{target}/{profile}/flames"),
            kind: nbuild::PackageKind::Utility,
        },
        nbuild::Package {
            name: "Neotron OS",
            path: std::path::Path::new("./neotron-os/Cargo.toml"),
            output: std::path::Path::new("./target/{target}/{profile}/neotron-os"),
            kind: nbuild::PackageKind::Os,
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
        Some(Commands::Libraries { target }) => library(&packages, target.as_deref()),
        Some(Commands::Format { check }) => format(&packages, check),
        Some(Commands::Clippy) => clippy(&packages),
    }
}

/// Builds the utility and OS packages as binaries
fn binary(packages: &[nbuild::Package], start_address: &str, target: &str) {
    let mut is_error = false;
    let Ok(start_address) = nbuild::parse_int(start_address) else {
        eprintln!("{:?} was not a valid integer", start_address);
        std::process::exit(1);
    };
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
    }
    for package in packages
        .iter()
        .filter(|p| p.kind == nbuild::PackageKind::Os)
    {
        println!(
            "Cross-compiling {}, using start address 0x{:08x} and target {:?}",
            package.name, start_address, target
        );
        let environment = [(
            "NEOTRON_OS_START_ADDRESS",
            format!("0x{:08x}", start_address),
        )];
        if let Err(e) = nbuild::cargo_with_env(
            &["build", "--release"],
            Some(target),
            package.path,
            &environment,
        ) {
            eprintln!("Build of {} failed: {}", package.name, e);
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
        if let Err(e) = nbuild::cargo(&["build", "--lib"], target, package.path) {
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

// End of file
