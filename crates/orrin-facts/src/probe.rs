use std::path::Path;
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;

use regex::Regex;
use tokio::process::Command;

const TIMEOUT: Duration = Duration::from_secs(8);

pub enum Probe {
    Version(String),
    Silent,
    Broken,
}

pub async fn probe(path: &Path, args: &[&str]) -> Probe {
    let mut command = Command::new(path);
    command.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x0800_0000);
    let Ok(Ok(output)) = tokio::time::timeout(TIMEOUT, command.output()).await else { return Probe::Broken };
    let text = format!("{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    match parse_version(&text) {
        Some(version) => Probe::Version(version),
        None if output.status.success() => Probe::Silent,
        None => Probe::Broken,
    }
}

pub fn parse_version(text: &str) -> Option<String> {
    static VERSION: OnceLock<Regex> = OnceLock::new();
    let pattern = VERSION.get_or_init(|| Regex::new(r"\d+(?:\.\d+)+").expect("version pattern"));
    pattern.find(text).map(|m| m.as_str().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_version_lines() {
        assert_eq!(parse_version("git version 2.45.1.windows.1").as_deref(), Some("2.45.1"));
        assert_eq!(parse_version("Blender 4.2.0\n\tbuild date").as_deref(), Some("4.2.0"));
        assert_eq!(parse_version("4.3.stable.official.77dcf97d8").as_deref(), Some("4.3"));
        assert_eq!(parse_version("no digits"), None);
    }
}
