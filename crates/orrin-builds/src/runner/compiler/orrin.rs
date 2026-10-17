use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use orrin_core::daemon::DaemonInfo;
use orrin_core::{Home, Result};
use tokio::net::TcpStream;
use tokio::process::Command;

use super::{Env, var};

const SERVER_PORTS: u16 = 40000;
const START: Duration = Duration::from_secs(30);

pub async fn backend(home: &Home, tool: &Path) -> Result<Env> {
    let daemon = DaemonInfo::read(home)?;
    let server = server_port(daemon.port);
    let env = env(&daemon.url(), &home.local_key()?, server);
    start_server(tool, &env, server).await;
    Ok(env)
}

fn env(url: &str, key: &str, server: u16) -> Env {
    vec![
        var("SCCACHE_WEBDAV_ENDPOINT", format!("{url}/cache")),
        var("SCCACHE_WEBDAV_TOKEN", key),
        var("SCCACHE_SERVER_PORT", server.to_string()),
    ]
}

fn server_port(daemon: u16) -> u16 {
    SERVER_PORTS + daemon % 20000
}

async fn start_server(tool: &Path, env: &Env, port: u16) {
    if TcpStream::connect(("127.0.0.1", port)).await.is_ok() {
        return;
    }
    let mut command = Command::new(tool);
    command.arg("--start-server").envs(env.iter().map(|(name, value)| (name, value)));
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    command.creation_flags(windows::Win32::System::Threading::CREATE_NO_WINDOW.0);
    if let Err(err) = tokio::time::timeout(START, command.status()).await {
        tracing::warn!("sccache did not start its server in time: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::super::get;
    use super::*;

    #[test]
    fn points_sccache_at_the_local_server() {
        let env = env("http://127.0.0.1:4520", "secret", server_port(4520));
        assert_eq!(get(&env, "SCCACHE_WEBDAV_ENDPOINT").unwrap(), "http://127.0.0.1:4520/cache");
        assert_eq!(get(&env, "SCCACHE_WEBDAV_TOKEN").unwrap(), "secret");
        assert_eq!(get(&env, "SCCACHE_SERVER_PORT").unwrap(), "44520");
    }
}
