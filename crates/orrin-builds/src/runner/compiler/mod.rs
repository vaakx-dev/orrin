mod orrin;

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use orrin_core::Result;
use orrin_core::model::{CompilerCache, Target};

use crate::context::Context;
use crate::tools;

pub type Env = Vec<(String, OsString)>;

pub struct Wiring {
    pub env: Env,
    pub note: Option<String>,
}

const SCCACHE: &str = "sccache";

pub async fn wire(ctx: &Context, target: &Target, dir: &Path) -> Result<Wiring> {
    let unwired = |note| Ok(Wiring { env: Vec::new(), note: Some(note) });
    if target.cache == CompilerCache::None {
        return Ok(Wiring { env: Vec::new(), note: None });
    }
    let Some(tool) = tools::find(&ctx.hub, SCCACHE).await else {
        return unwired(format!("{SCCACHE} is not installed on this PC; building without a compiler cache"));
    };
    let backend = match target.cache {
        CompilerCache::Orrin => match orrin::backend(&ctx.home, &tool).await {
            Ok(env) => env,
            Err(err) => return unwired(format!("the Orrin compiler cache is unreachable ({err}); not caching")),
        },
        _ => vec![var("SCCACHE_DIR", ctx.home.dir(&format!("caches/{SCCACHE}"))?)],
    };
    let env = sccache(Launch { tool, backend, dir: dir.to_path_buf(), warm: target.warm.pool(1).is_some() });
    Ok(Wiring { env, note: None })
}

pub struct Launch {
    pub tool: PathBuf,
    pub backend: Env,
    pub dir: PathBuf,
    pub warm: bool,
}

fn sccache(launch: Launch) -> Env {
    let mut env = launchers(&launch.tool);
    env.push(var("RUSTC_WRAPPER", &launch.tool));
    env.extend(launch.backend);
    env.push(var("SCCACHE_BASEDIRS", &launch.dir));
    if !launch.warm {
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
        let launch = |warm| Launch {
            tool: PathBuf::from("/bin/sccache"),
            backend: vec![var("SCCACHE_DIR", "/home/caches/sccache")],
            dir: PathBuf::from("/home/runs/job"),
            warm,
        };
        let env = sccache(launch(false));
        assert_eq!(get(&env, "RUSTC_WRAPPER").unwrap(), "/bin/sccache");
        assert_eq!(get(&env, "ORRIN_COMPILER_LAUNCHER").unwrap(), "/bin/sccache");
        assert_eq!(get(&env, "SCCACHE_DIR").unwrap(), "/home/caches/sccache");
        assert_eq!(get(&env, "SCCACHE_BASEDIRS").unwrap(), "/home/runs/job");
        assert_eq!(get(&env, "CARGO_INCREMENTAL").unwrap(), "0");
        assert!(get(&sccache(launch(true)), "CARGO_INCREMENTAL").is_none());
    }
}
