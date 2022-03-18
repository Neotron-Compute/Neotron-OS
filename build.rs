fn main() {
    println!("cargo:rustc-link-arg-bin=flash1002=-Tneotron-flash-1002.ld");
    println!("cargo:rustc-link-arg-bin=flash0802=-Tneotron-flash-0802.ld");
    println!("cargo:rustc-link-arg-bin=flash0002=-Tneotron-flash-0002.ld");

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
}
