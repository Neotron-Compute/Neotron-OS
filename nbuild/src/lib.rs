//! Utility functions

/// The ways that spawning `cargo` can fail
#[derive(Debug)]
pub enum ProcessError {
    SpawnError(std::io::Error),
    RunError(std::process::ExitStatus),
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessError::SpawnError(error) => write!(f, "Failed to spawn command: {}", error),
            ProcessError::RunError(exit_status) => write!(
                f,
                "Failed to complete command ({}). There should be an error above",
                exit_status
            ),
        }
    }
}

/// The kinds of package we have
#[derive(Debug, PartialEq, Eq)]
pub enum PackageKind {
    Os,
    Utility,
    NBuild,
}

/// Can this package be tested?
#[derive(Debug, PartialEq, Eq)]
pub enum Testable {
    No,
    All,
    Libs,
}

/// Describes a package in this repository
#[derive(Debug)]
pub struct Package {
    pub name: &'static str,
    pub path: &'static std::path::Path,
    pub kind: PackageKind,
    pub testable: Testable,
    pub output_template: Option<&'static str>,
}

impl Package {
    pub fn output(&self, target: &str, profile: &str) -> Option<String> {
        self.output_template
            .map(|s| s.replace("{target}", target).replace("{profile}", profile))
    }
}

/// Parse an integer, with an optional `0x` prefix.
///
/// Underscores are ignored.
///
/// ```rust
/// # use nbuild::parse_int;
/// assert_eq!(parse_int("0x0000_000A"), Ok(10));
/// assert_eq!(parse_int("000_10"), Ok(10));
/// ```
pub fn parse_int<S>(input: S) -> Result<u32, std::num::ParseIntError>
where
    S: AsRef<str>,
{
    let input = input.as_ref().replace('_', "");
    if let Some(suffix) = input.strip_prefix("0x") {
        u32::from_str_radix(suffix, 16)
    } else {
        input.parse()
    }
}

/// Runs cargo
pub fn cargo<P>(
    commands: &[&str],
    target: Option<&str>,
    manifest_path: P,
) -> Result<(), ProcessError>
where
    P: AsRef<std::path::Path>,
{
    cargo_with_env(commands, target, manifest_path, &[])
}

/// Runs cargo with extra environment variables
pub fn cargo_with_env<P>(
    commands: &[&str],
    target: Option<&str>,
    manifest_path: P,
    environment: &[(&'static str, String)],
) -> Result<(), ProcessError>
where
    P: AsRef<std::path::Path>,
{
    let mut command_line = std::process::Command::new("cargo");
    command_line.stdout(std::process::Stdio::inherit());
    command_line.stderr(std::process::Stdio::inherit());
    command_line.args(commands);
    if let Some(target) = target {
        command_line.arg("--target");
        command_line.arg(target);
    }
    command_line.arg("--manifest-path");
    command_line.arg(manifest_path.as_ref());
    for (k, v) in environment.iter() {
        command_line.env(k, v);
    }

    println!("Running: {:?}", command_line);

    let output = command_line.output().map_err(ProcessError::SpawnError)?;

    if output.status.success() {
        Ok(())
    } else {
        Err(ProcessError::RunError(output.status))
    }
}

/// Make a binary version of an ELF file
pub fn make_bin<P>(path: P) -> Result<std::path::PathBuf, ProcessError>
where
    P: AsRef<std::path::Path>,
{
    let path = path.as_ref();
    println!("Making binary of: {}", path.display());
    let objcopy = tool_path("llvm-objcopy");
    let mut command_line = std::process::Command::new(objcopy);
    command_line.args(["-O", "binary"]);
    command_line.arg(path);
    let output_file = path.with_extension("bin");
    command_line.arg(&output_file);
    println!("Running: {:?}", command_line);
    let output = command_line.output().map_err(ProcessError::SpawnError)?;
    if output.status.success() {
        Ok(output_file)
    } else {
        Err(ProcessError::RunError(output.status))
    }
}

/// Make a binary version of an ELF file
pub fn strip_elf<P1, P2>(input_path: P1, output_path: P2) -> Result<(), ProcessError>
where
    P1: AsRef<std::path::Path>,
    P2: AsRef<std::path::Path>,
{
    let input_path = input_path.as_ref();
    let output_path = output_path.as_ref();

    println!(
        "Stripping {} as {}",
        input_path.display(),
        output_path.display()
    );
    let strip = tool_path("llvm-strip");
    let mut command_line = std::process::Command::new(strip);
    command_line.arg(input_path);
    command_line.arg("-o");
    command_line.arg(output_path);
    println!("Running: {:?}", command_line);
    let output = command_line.output().map_err(ProcessError::SpawnError)?;
    if output.status.success() {
        Ok(())
    } else {
        Err(ProcessError::RunError(output.status))
    }
}

/// Get the path where `llvm-objcopy` and friends live.
pub fn tool_path(tool: &str) -> std::path::PathBuf {
    let output = std::process::Command::new("rustc")
        .arg("--print")
        .arg("target-libdir")
        .output()
        .expect("Failed to run rustc --print target-libdir");
    let sysroot = String::from_utf8(output.stdout).expect("sysroot path isn't UTF-8");
    let mut result: std::path::PathBuf = sysroot.trim().into();
    result.pop();
    result.push("bin");
    result.push(tool);
    result
}

// End of file
