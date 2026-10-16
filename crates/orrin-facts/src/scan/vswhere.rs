use std::path::{Path, PathBuf};
use std::process::Command;

use orrin_core::model::ToolSource;
use serde_json::Value;

use super::{Found, newest_first};

pub fn scan() -> Vec<Found> {
    let Some(vswhere) = locate() else { return Vec::new() };
    let mut command = Command::new(vswhere);
    command.args(["-latest", "-products", "*", "-format", "json", "-utf8"]);
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut command, 0x0800_0000);
    let Ok(output) = command.output() else { return Vec::new() };
    let Ok(Value::Array(installs)) = serde_json::from_slice::<Value>(&output.stdout) else { return Vec::new() };
    installs.iter().flat_map(tools).collect()
}

fn locate() -> Option<PathBuf> {
    let root = std::env::var_os("ProgramFiles(x86)")?;
    let path = under(Path::new(&root), "Microsoft Visual Studio/Installer/vswhere.exe");
    path.is_file().then_some(path)
}

fn tools(install: &Value) -> Vec<Found> {
    let Some(root) = install["installationPath"].as_str().map(PathBuf::from) else { return Vec::new() };
    let version = install["installationVersion"].as_str().map(str::to_string);
    let found = |name: &str, path: PathBuf, version: Option<String>| {
        path.is_file().then(|| Found { name: name.into(), path, version, source: ToolSource::KnownDir })
    };
    let mut tools = Vec::new();
    tools.extend(found("msbuild", under(&root, "MSBuild/Current/Bin/MSBuild.exe"), None));
    tools.extend(found("vsdevcmd", under(&root, "Common7/Tools/VsDevCmd.bat"), version));
    if let Some(msvc) = newest(&under(&root, "VC/Tools/MSVC")) {
        let tag = msvc.file_name().map(|n| n.to_string_lossy().into_owned());
        tools.extend(found("cl", under(&msvc, "bin/Hostx64/x64/cl.exe"), tag));
    }
    tools
}

fn under(root: &Path, relative: &str) -> PathBuf {
    relative.split('/').fold(root.to_path_buf(), |path, part| path.join(part))
}

fn newest(dir: &Path) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> =
        std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    newest_first(&mut dirs);
    dirs.into_iter().next()
}
