use std::io::prelude::*;

const LINKER_SCRIPT: &str = "neotron-os-arm.ld";

fn main() {
    if Ok("none") == std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        let start_address = std::env::var("NEOTRON_OS_START_ADDRESS");
        let start_address = start_address.as_deref().unwrap_or("0x10020000");
        copy_linker_script(start_address);
        println!("cargo::rustc-link-arg-bin=neotron-os=-T{}", LINKER_SCRIPT);
        println!("cargo::rerun-if-env-changed=NEOTRON_OS_START_ADDRESS");
    }

    if let Ok(cmd_output) = std::process::Command::new("git")
        .arg("describe")
        .arg("--all")
        .arg("--dirty")
        .arg("--long")
        .output()
    {
        let git_version = std::str::from_utf8(&cmd_output.stdout).unwrap();
        println!(
            "cargo::rustc-env=OS_VERSION={} (git:{})",
            env!("CARGO_PKG_VERSION"),
            git_version.trim()
        );
    } else {
        println!("cargo::rustc-env=OS_VERSION={}", env!("CARGO_PKG_VERSION"));
    }

    if Ok("macos") == std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        println!("cargo::rustc-link-lib=c");
    }

    if Ok("windows") == std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        println!("cargo::rustc-link-lib=dylib=msvcrt");
    }

    if let Some(path) = option_env!("ROMFS_PATH") {
        println!("cargo::rustc-cfg=romfs_enabled=\"yes\"");
        println!("cargo::rerun-if-env-changed=ROMFS_PATH");
        println!("cargo::rerun-if-changed={}", path);
    }
    println!("cargo::rustc-check-cfg=cfg(romfs_enabled, values(\"yes\"))");
}

/// Put the given script in our output directory and ensure it's on the linker
/// search path.
fn copy_linker_script(start_address: &str) {
    let out = &std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let contents = std::fs::read_to_string(LINKER_SCRIPT).expect("loading ld script");
    let patched = contents.replace("${{start_address}}", start_address);
    std::fs::File::create(out.join(LINKER_SCRIPT))
        .unwrap()
        .write_all(patched.as_bytes())
        .unwrap();
    println!("cargo::rustc-link-search={}", out.display());
}

// End of file
