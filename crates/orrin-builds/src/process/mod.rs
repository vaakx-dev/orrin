#[cfg(not(windows))]
mod tree_other;
#[cfg(windows)]
mod tree_windows;

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use orrin_core::Result;
use orrin_core::model::Stream;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, watch};

#[cfg(not(windows))]
use tree_other::Tree;
#[cfg(windows)]
use tree_windows::Tree;

#[derive(Clone, Copy, Debug, Default)]
#[cfg_attr(not(windows), allow(dead_code))]
pub struct Limits {
    pub cpu_percent: Option<u32>,
    pub memory_mb: Option<u64>,
}

pub struct Spec {
    pub command: String,
    pub args: Vec<String>,
    pub dir: PathBuf,
    pub env: Vec<(String, OsString)>,
    pub limits: Limits,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Exit {
    Code(Option<i32>),
    Cancelled,
}

type Line = (Stream, String);

pub async fn run(
    spec: Spec,
    mut cancel: watch::Receiver<bool>,
    mut on_line: impl FnMut(Stream, String),
) -> Result<Exit> {
    let mut command = shell(&spec);
    command.current_dir(&spec.dir).envs(spec.env.iter().map(|(k, v)| (k, v)));
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true);
    let (mut child, tree) = Tree::spawn(&mut command, spec.limits)?;
    let (tx, mut rx) = mpsc::unbounded_channel::<Line>();
    if let Some(out) = child.stdout.take() {
        tokio::spawn(read_lines(out, Stream::Stdout, tx.clone()));
    }
    if let Some(err) = child.stderr.take() {
        tokio::spawn(read_lines(err, Stream::Stderr, tx.clone()));
    }
    drop(tx);
    let exit = loop {
        tokio::select! {
            status = child.wait() => break Exit::Code(status?.code()),
            Some((stream, text)) = rx.recv() => on_line(stream, text),
            _ = cancelled(&mut cancel) => {
                tree.kill();
                let _ = child.start_kill();
                let _ = child.wait().await;
                break Exit::Cancelled;
            }
        }
    };
    tree.kill();
    let drain = async {
        while let Some((stream, text)) = rx.recv().await {
            on_line(stream, text);
        }
    };
    let _ = tokio::time::timeout(Duration::from_secs(2), drain).await;
    Ok(exit)
}

async fn cancelled(cancel: &mut watch::Receiver<bool>) {
    let _ = cancel.wait_for(|c| *c).await;
}

async fn read_lines(source: impl AsyncRead + Unpin, stream: Stream, tx: mpsc::UnboundedSender<Line>) {
    let mut reader = BufReader::new(source);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => return,
            Ok(_) => {
                let text = String::from_utf8_lossy(&buf).trim_end_matches(['\r', '\n']).to_string();
                if tx.send((stream, text)).is_err() {
                    return;
                }
            }
        }
    }
}

pub fn join_args(args: &[String]) -> String {
    args.iter().map(|arg| quote(arg)).collect::<Vec<_>>().join(" ")
}

#[cfg(windows)]
fn shell(spec: &Spec) -> Command {
    let mut line = spec.command.clone();
    if !spec.args.is_empty() {
        line.push(' ');
        line.push_str(&join_args(&spec.args));
    }
    let mut command = Command::new(std::env::var_os("ComSpec").unwrap_or_else(|| "cmd".into()));
    command.args(["/D", "/S", "/C"]).raw_arg(format!("\"{line}\""));
    command
}

#[cfg(windows)]
fn quote(arg: &str) -> String {
    match arg.is_empty() || arg.contains([' ', '\t', '"', '&', '|', '<', '>', '^']) {
        true => format!("\"{}\"", arg.replace('"', "\"\"")),
        false => arg.to_string(),
    }
}

#[cfg(not(windows))]
fn quote(arg: &str) -> String {
    let plain = !arg.is_empty() && arg.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:,+@%".contains(c));
    match plain {
        true => arg.to_string(),
        false => format!("'{}'", arg.replace('\'', "'\\''")),
    }
}

#[cfg(not(windows))]
fn shell(spec: &Spec) -> Command {
    let line = match spec.args.is_empty() {
        true => spec.command.clone(),
        false => format!("{} \"$@\"", spec.command),
    };
    let mut command = Command::new("sh");
    command.arg("-c").arg(line).arg("sh").args(&spec.args);
    command
}
