//! Utility functions

/// The ways that spawning `cargo` can fail
#[derive(Debug)]
pub enum CargoError {
    SpawnError(std::io::Error),
    RunError(std::process::ExitStatus),
}

impl std::fmt::Display for CargoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CargoError::SpawnError(error) => write!(f, "Failed to spawn `cargo`: {}", error),
            CargoError::RunError(exit_status) => write!(
                f,
                "Failed to complete `cargo` command ({}). There should be an error above",
                exit_status
            ),
        }
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
        u32::from_str_radix(&input, 10)
    }
}

/// Runs cargo
pub fn cargo<P>(commands: &[&str], target: Option<&str>, manifest_path: P) -> Result<(), CargoError>
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

    println!("Running: {:?}", command_line);

    let output = command_line.output().map_err(CargoError::SpawnError)?;

    if output.status.success() {
        Ok(())
    } else {
        Err(CargoError::RunError(output.status))
    }
}
