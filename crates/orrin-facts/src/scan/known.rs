use std::path::{Component, Path, PathBuf};

use orrin_core::model::ToolSource;

use super::{Found, newest_first, prefixed};

pub fn scan() -> Vec<Found> {
    let mut found = Vec::new();
    for (name, pattern) in files() {
        if let Some(path) = expand(&pattern).into_iter().find(|p| p.is_file()) {
            found.push(Found { name: name.into(), path, version: None, source: ToolSource::KnownDir });
        }
    }
    let godot = godot_dirs().iter().flat_map(|d| expand(d)).flat_map(|d| prefixed(&d, "godot")).next();
    if let Some(path) = godot {
        found.push(Found { name: "godot".into(), path, version: None, source: ToolSource::KnownDir });
    }
    found
}

struct Roots {
    programs: Option<PathBuf>,
    programs_x86: Option<PathBuf>,
    local: Option<PathBuf>,
    home: Option<PathBuf>,
}

fn roots() -> Roots {
    let env = |name: &str| std::env::var_os(name).map(PathBuf::from);
    Roots {
        programs: env("ProgramFiles"),
        programs_x86: env("ProgramFiles(x86)"),
        local: env("LOCALAPPDATA"),
        home: dirs::home_dir(),
    }
}

fn under(root: &Option<PathBuf>, rest: &str) -> Option<PathBuf> {
    root.as_ref().map(|r| r.join(rest))
}

fn files() -> Vec<(&'static str, PathBuf)> {
    let r = roots();
    let listed = if cfg!(windows) {
        vec![
            ("blender", under(&r.programs, "Blender Foundation/Blender*/blender.exe")),
            ("blender", under(&r.programs_x86, "Steam/steamapps/common/Blender/blender.exe")),
            ("git", under(&r.programs, "Git/cmd/git.exe")),
            ("python", under(&r.local, "Programs/Python/Python3*/python.exe")),
            ("python", under(&r.programs, "Python3*/python.exe")),
            ("node", under(&r.programs, "nodejs/node.exe")),
            ("bun", under(&r.home, ".bun/bin/bun.exe")),
            ("cargo", under(&r.home, ".cargo/bin/cargo.exe")),
            ("rustc", under(&r.home, ".cargo/bin/rustc.exe")),
            ("cmake", under(&r.programs, "CMake/bin/cmake.exe")),
            ("7z", under(&r.programs, "7-Zip/7z.exe")),
        ]
    } else {
        vec![
            ("blender", Some(PathBuf::from("/Applications/Blender.app/Contents/MacOS/Blender"))),
            ("bun", under(&r.home, ".bun/bin/bun")),
            ("cargo", under(&r.home, ".cargo/bin/cargo")),
            ("rustc", under(&r.home, ".cargo/bin/rustc")),
        ]
    };
    listed.into_iter().filter_map(|(name, path)| Some((name, path?))).collect()
}

fn godot_dirs() -> Vec<PathBuf> {
    let r = roots();
    let listed = if cfg!(windows) {
        vec![
            under(&r.programs, "Godot*"),
            under(&r.local, "Programs/Godot*"),
            under(&r.home, "Godot*"),
            under(&r.home, "scoop/apps/godot/current"),
            under(&r.programs_x86, "Steam/steamapps/common/Godot Engine"),
            under(&r.local, "Microsoft/WinGet/Packages/GodotEngine.GodotEngine*"),
        ]
    } else {
        vec![Some(PathBuf::from("/Applications/Godot*.app/Contents/MacOS")), under(&r.home, "Godot*")]
    };
    listed.into_iter().flatten().collect()
}

fn expand(pattern: &Path) -> Vec<PathBuf> {
    let mut current = vec![PathBuf::new()];
    for part in pattern.components() {
        let text = part.as_os_str().to_string_lossy();
        let wild = matches!(part, Component::Normal(_)) && text.contains('*');
        current = if wild {
            current.iter().flat_map(|dir| matching(dir, &text)).collect()
        } else {
            current.into_iter().map(|dir| dir.join(part)).collect()
        };
    }
    newest_first(&mut current);
    current.into_iter().filter(|p| p.exists()).collect()
}

fn matching(dir: &Path, pattern: &str) -> Vec<PathBuf> {
    let (head, tail) = pattern.split_once('*').unwrap_or((pattern, ""));
    let (head, tail) = (head.to_lowercase(), tail.to_lowercase());
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    entries
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.len() >= head.len() + tail.len() && name.starts_with(&head) && name.ends_with(&tail)
        })
        .map(|e| e.path())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_wildcards_newest_first() {
        let dir = tempfile::tempdir().unwrap();
        for version in ["Blender 3.6", "Blender 4.2", "Other"] {
            std::fs::create_dir_all(dir.path().join(version)).unwrap();
            std::fs::write(dir.path().join(version).join("blender.exe"), "").unwrap();
        }
        let found = expand(&dir.path().join("Blender*").join("blender.exe"));
        assert_eq!(found.len(), 2);
        assert!(found[0].to_string_lossy().contains("4.2"));
    }
}
