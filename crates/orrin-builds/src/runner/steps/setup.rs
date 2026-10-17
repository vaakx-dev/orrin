use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::Path;

use orrin_core::model::Stream;
use orrin_core::{Error, Hash, Home, Result};
use tokio::sync::watch;

use crate::process::{self, Exit, Spec};

pub type Delta = BTreeMap<String, String>;

pub async fn capture(
    home: &Home,
    line: &str,
    dir: &Path,
    env: &[(String, OsString)],
    cancel: watch::Receiver<bool>,
) -> Result<(Delta, bool)> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let given = env.iter().find(|(name, _)| name == "PATH").map_or(&path, |(_, value)| value);
    let cached =
        home.dir("builds/setup")?.join(Hash::of(format!("{line}\n{}", given.to_string_lossy()).as_bytes()).as_str());
    if let Some(delta) = std::fs::read(&cached).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()) {
        return Ok((delta, false));
    }
    let spec = Spec {
        command: dump(line),
        args: Vec::new(),
        dir: dir.to_path_buf(),
        env: env.to_vec(),
        limits: Default::default(),
    };
    let mut printed = Vec::new();
    let exit = process::run(spec, cancel, |stream, text| {
        if stream == Stream::Stdout {
            printed.push(text);
        }
    })
    .await?;
    if exit != Exit::Code(Some(0)) {
        return Err(Error::invalid(format!("setup `{line}` failed: {exit:?}")));
    }
    let delta = delta(&base(env), &printed);
    std::fs::write(&cached, serde_json::to_vec(&delta)?)?;
    Ok((delta, true))
}

fn base(env: &[(String, OsString)]) -> BTreeMap<String, String> {
    let mut base: BTreeMap<String, String> = std::env::vars_os()
        .map(|(name, value)| (fold(&name.to_string_lossy()), value.to_string_lossy().into_owned()))
        .collect();
    base.extend(env.iter().map(|(name, value)| (fold(name), value.to_string_lossy().into_owned())));
    base
}

fn delta(base: &BTreeMap<String, String>, printed: &[String]) -> Delta {
    let pairs = printed.iter().filter_map(|line| line.split_once('=')).filter(|(name, _)| !name.is_empty());
    pairs
        .filter(|(name, value)| base.get(&fold(name)).is_none_or(|old| old != value))
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .collect()
}

fn fold(name: &str) -> String {
    if cfg!(windows) { name.to_uppercase() } else { name.to_string() }
}

#[cfg(windows)]
fn dump(line: &str) -> String {
    format!("call {line} >nul 2>&1 && set")
}

#[cfg(not(windows))]
fn dump(line: &str) -> String {
    format!("{line} >/dev/null 2>&1 && env")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_what_the_setup_changed() {
        let base: BTreeMap<String, String> =
            [("PATH", "/bin"), ("HOME", "/h")].map(|(k, v)| (fold(k), v.to_string())).into();
        let printed = ["PATH=/sdk:/bin", "HOME=/h", "INCLUDE=/sdk/inc", "=C:=C:\\"].map(String::from);
        let delta = delta(&base, &printed);
        assert_eq!(delta.keys().collect::<Vec<_>>(), ["INCLUDE", "PATH"]);
    }
}
