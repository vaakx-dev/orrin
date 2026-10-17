use std::ffi::OsStr;
use std::path::Path;

use orrin_core::{Error, Result};
use tokio::process::Command;

pub async fn run(program: impl AsRef<OsStr>, args: &[&OsStr], dir: Option<&Path>) -> Result<()> {
    let program = program.as_ref();
    let mut command = Command::new(program);
    command.args(args).env("GIT_TERMINAL_PROMPT", "0").stdin(std::process::Stdio::null());
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    let shown = || {
        let parts: Vec<_> = std::iter::once(program).chain(args.iter().copied()).map(OsStr::to_string_lossy).collect();
        parts.join(" ")
    };
    let output =
        command.output().await.map_err(|err| Error::unavailable(format!("could not run {}: {err}", shown())))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(Error::unavailable(format!("{} failed: {}", shown(), stderr.trim())))
}
