mod orrin;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use orrin_core::Result;
use orrin_core::model::{CompilerCache, Target};

use crate::context::Context;
use crate::tools;

pub type Env = Vec<(String, OsString)>;

#[derive(Default)]
pub struct Wiring {
    pub tool: Option<PathBuf>,
    pub env: Env,
    pub note: Option<String>,
}

const SCCACHE: &str = "sccache";

pub async fn wire(ctx: &Context, target: &Target, dir: &Path) -> Result<Wiring> {
    let mut wiring = backend(ctx, target.cache, dir).await?;
    if let Some(tool) = &wiring.tool {
        wiring.env = sccache(tool, std::mem::take(&mut wiring.env), target.warm.pool(1).is_some());
    }
    Ok(wiring)
}

pub async fn backend(ctx: &Context, cache: CompilerCache, dir: &Path) -> Result<Wiring> {
    let unwired = |note| Ok(Wiring { note: Some(note), ..Default::default() });
    if cache == CompilerCache::None {
        return Ok(Wiring::default());
    }
    let Some(tool) = tools::find(&ctx.hub, SCCACHE).await else {
        return unwired(format!("{SCCACHE} is not installed on this PC; building without a compiler cache"));
    };
    let mut env = match cache {
        CompilerCache::Orrin => match orrin::backend(&ctx.home, &tool).await {
            Ok(env) => env,
            Err(err) => return unwired(format!("the Orrin compiler cache is unreachable ({err}); not caching")),
        },
        _ => vec![var("SCCACHE_DIR", ctx.home.dir(&format!("caches/{SCCACHE}"))?)],
    };
    env.push(var("SCCACHE_BASEDIRS", dir));
    Ok(Wiring { tool: Some(tool), env, note: None })
}

pub fn wrap(command: &str, tool: Option<&Path>) -> String {
    match tool {
        Some(tool) => format!("\"{}\" {}", tool.display(), command.trim_start()),
        None => command.to_string(),
    }
}

fn sccache(tool: &Path, backend: Env, warm: bool) -> Env {
    let mut env = launchers(tool);
    env.push(var("RUSTC_WRAPPER", tool));
    env.extend(backend);
    if !warm {
        env.push(var("CARGO_INCREMENTAL", "0"));
    }
    env
}

fn launchers(tool: &Path) -> Env {
    vec![
        var("ORRIN_COMPILER_LAUNCHER", tool),
        var("CMAKE_C_COMPILER_LAUNCHER", tool),
        var("CMAKE_CXX_COMPILER_LAUNCHER", tool),
    ]
}

fn var(name: &str, value: impl AsRef<std::ffi::OsStr>) -> (String, OsString) {
    (name.to_string(), value.as_ref().to_os_string())
}

#[cfg(test)]
fn get<'a>(env: &'a Env, name: &str) -> Option<&'a OsString> {
    env.iter().find(|(n, _)| n == name).map(|(_, v)| v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sccache_wraps_rustc_and_shares_the_pc_cache() {
        let tool = Path::new("/bin/sccache");
        let backend = || vec![var("SCCACHE_DIR", "/home/caches/sccache")];
        let env = sccache(tool, backend(), false);
        assert_eq!(get(&env, "RUSTC_WRAPPER").unwrap(), "/bin/sccache");
        assert_eq!(get(&env, "ORRIN_COMPILER_LAUNCHER").unwrap(), "/bin/sccache");
        assert_eq!(get(&env, "SCCACHE_DIR").unwrap(), "/home/caches/sccache");
        assert_eq!(get(&env, "CARGO_INCREMENTAL").unwrap(), "0");
        assert!(get(&sccache(tool, backend(), true), "CARGO_INCREMENTAL").is_none());
    }

    #[test]
    fn wraps_a_rule_command_with_the_cache() {
        assert_eq!(wrap(" cl /c a.cpp", Some(Path::new("/bin/sccache"))), "\"/bin/sccache\" cl /c a.cpp");
        assert_eq!(wrap("cl /c a.cpp", None), "cl /c a.cpp");
    }
}
