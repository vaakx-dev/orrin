use std::path::Path;
use std::process::Command;

use orrin_core::{Error, Result};

pub fn push(repo: &Path, remote: &str, branch_ref: &str) -> Result<()> {
    let spec = format!("{branch_ref}:{branch_ref}");
    git(repo, &["push", "--quiet", "--", remote, &spec]).map_err(|err| {
        if err.message.contains("rejected") || err.message.contains("non-fast-forward") {
            Error::conflict(format!("the remote moved; run git.import first: {}", err.message))
        } else {
            err
        }
    })
}

pub fn fetch(repo: &Path, remote: &str, branch_ref: &str, into: &str) -> Result<()> {
    let spec = format!("+{branch_ref}:{into}");
    git(repo, &["fetch", "--quiet", "--no-tags", "--", remote, &spec])
}

fn git(repo: &Path, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .arg("--git-dir")
        .arg(repo)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(|err| Error::unavailable(format!("could not run git: {err}")))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(Error::internal(format!("git {} failed: {stderr}", args[0])))
}
