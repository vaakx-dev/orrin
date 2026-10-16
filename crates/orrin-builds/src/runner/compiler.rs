use std::ffi::OsString;
use std::path::{Path, PathBuf};

use orrin_core::Result;
use orrin_core::model::{CompilerCache, Recipe};

use crate::context::Context;
use crate::tools;

pub type Env = Vec<(String, OsString)>;

pub struct Wiring {
    pub env: Env,
    pub note: Option<String>,
}

pub async fn wire(ctx: &Context, recipe: &Recipe, dir: &Path) -> Result<Wiring> {
    let wired = |env| Ok(Wiring { env, note: None });
    let (name, launcher): (&str, fn(Launch) -> Env) = match recipe.cache {
        CompilerCache::None => return wired(Vec::new()),
        CompilerCache::Scons => return wired(scons(&ctx.home.dir("caches/scons")?)),
        CompilerCache::Sccache => ("sccache", sccache),
        CompilerCache::Ccache => ("ccache", ccache),
    };
    let Some(tool) = tools::find(&ctx.hub, name).await else {
        let note = format!("{name} is not installed on this PC; building without a compiler cache");
        return Ok(Wiring { env: Vec::new(), note: Some(note) });
    };
    let cache = ctx.home.dir(&format!("caches/{name}"))?;
    wired(launcher(Launch { tool, cache, dir: dir.to_path_buf(), warm: recipe.warm }))
}

pub struct Launch {
    pub tool: PathBuf,
    pub cache: PathBuf,
    pub dir: PathBuf,
    pub warm: bool,
}

fn sccache(launch: Launch) -> Env {
    let mut env = launchers(&launch.tool);
    env.push(var("RUSTC_WRAPPER", &launch.tool));
    env.push(var("SCCACHE_DIR", &launch.cache));
    env.push(var("SCCACHE_BASEDIRS", &launch.dir));
    if !launch.warm {
        env.push(var("CARGO_INCREMENTAL", "0"));
    }
    env
}

fn ccache(launch: Launch) -> Env {
    let mut env = launchers(&launch.tool);
    env.push(var("CCACHE_DIR", &launch.cache));
    env.push(var("CCACHE_BASEDIR", &launch.dir));
    env.push(var("CCACHE_NOHASHDIR", "1"));
    env
}

fn scons(cache: &Path) -> Env {
    vec![var("ORRIN_SCONS_CACHE", cache)]
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
mod tests {
    use super::*;

    fn get<'a>(env: &'a Env, name: &str) -> Option<&'a OsString> {
        env.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    #[test]
    fn sccache_wraps_rustc_and_shares_the_pc_cache() {
        let launch = |warm| Launch {
            tool: PathBuf::from("/bin/sccache"),
            cache: PathBuf::from("/home/caches/sccache"),
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
        assert_eq!(get(&ccache(launch(false)), "CCACHE_BASEDIR").unwrap(), "/home/runs/job");
        assert_eq!(get(&scons(Path::new("/c")), "ORRIN_SCONS_CACHE").unwrap(), "/c");
    }
}
