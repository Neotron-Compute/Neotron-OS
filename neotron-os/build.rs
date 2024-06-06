use std::io::prelude::*;

fn main() {
    if let Ok("none") = std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        copy_linker_script("neotron-flash-1002.ld");
        println!("cargo:rustc-link-arg-bin=flash1002=-Tneotron-flash-1002.ld");
        copy_linker_script("neotron-flash-0802.ld");
        println!("cargo:rustc-link-arg-bin=flash0802=-Tneotron-flash-0802.ld");
        copy_linker_script("neotron-flash-0002.ld");
        println!("cargo:rustc-link-arg-bin=flash0002=-Tneotron-flash-0002.ld");
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
            "cargo:rustc-env=OS_VERSION={} (git:{})",
            env!("CARGO_PKG_VERSION"),
            git_version.trim()
        );
    } else {
        println!("cargo:rustc-env=OS_VERSION={}", env!("CARGO_PKG_VERSION"));
    }

    if Ok("macos") == std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        println!("cargo:rustc-link-lib=c");
    }

    if Ok("windows") == std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
        println!("cargo:rustc-link-lib=dylib=msvcrt");
    }

    if option_env!("ROMFS_PATH").is_some() {
        println!("cargo:rustc-cfg=romfs_enabled=\"yes\"");
        println!("cargo:rerun-if-env-changed=ROMFS_PATH");
    }
}

/// Put the given script in our output directory and ensure it's on the linker
/// search path.
fn copy_linker_script(path: &str) {
    let out = &std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    let contents = std::fs::read_to_string(path).expect("loading ld script");
    std::fs::File::create(out.join(path))
        .unwrap()
        .write_all(contents.as_bytes())
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());
}

// End of file
