use super::Found;

#[cfg(not(windows))]
pub fn scan() -> Vec<Found> {
    Vec::new()
}

#[cfg(windows)]
pub fn scan() -> Vec<Found> {
    let mut found = windows::app_paths();
    found.extend(windows::uninstall());
    found
}

#[cfg(windows)]
mod windows {
    use std::path::{Path, PathBuf};

    use orrin_core::model::ToolSource;
    use winreg::{HKCU, HKLM, RegKey};

    use super::Found;
    use crate::book::normalise;
    use crate::scan::catalog::SPECS;
    use crate::scan::find_exe;

    const APP_PATHS: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\App Paths";
    const UNINSTALL: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall";
    const UNINSTALL_32: &str = r"SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall";

    pub fn app_paths() -> Vec<Found> {
        let mut found = Vec::new();
        for (name, key) in subkeys(&[(HKLM, APP_PATHS), (HKCU, APP_PATHS)]) {
            let Some(path) = text(&key, "").map(|p| PathBuf::from(clean(&p))) else { continue };
            if path.is_file() {
                found.push(Found { name: normalise(&name), path, version: None, source: ToolSource::Registry });
            }
        }
        found
    }

    pub fn uninstall() -> Vec<Found> {
        let mut found = Vec::new();
        for (_, key) in subkeys(&[(HKLM, UNINSTALL), (HKLM, UNINSTALL_32), (HKCU, UNINSTALL)]) {
            let Some(display) = text(&key, "DisplayName").map(|d| d.to_lowercase()) else { continue };
            let Some(spec) = SPECS.iter().find(|s| s.installer.is_some_and(|i| names(&display, i))) else { continue };
            let Some(dir) = install_dir(&key) else { continue };
            if let Some(path) = find_exe(&dir, spec) {
                let version = text(&key, "DisplayVersion");
                found.push(Found { name: spec.name.into(), path, version, source: ToolSource::Registry });
            }
        }
        found
    }

    fn subkeys(roots: &[(&RegKey, &str)]) -> Vec<(String, RegKey)> {
        let mut keys = Vec::new();
        for (root, path) in roots {
            let Ok(parent) = root.open_subkey(path) else { continue };
            for name in parent.enum_keys().flatten() {
                if let Ok(key) = parent.open_subkey(&name) {
                    keys.push((name, key));
                }
            }
        }
        keys
    }

    fn names(display: &str, installer: &str) -> bool {
        display.strip_prefix(installer).is_some_and(|rest| !rest.starts_with(|c: char| c.is_alphanumeric()))
    }

    fn install_dir(key: &RegKey) -> Option<PathBuf> {
        if let Some(dir) = text(key, "InstallLocation").map(|d| PathBuf::from(clean(&d))).filter(|d| d.is_dir()) {
            return Some(dir);
        }
        let icon = text(key, "DisplayIcon")?;
        let icon = clean(icon.split(',').next().unwrap_or(&icon));
        Path::new(&icon).parent().map(Path::to_path_buf)
    }

    fn text(key: &RegKey, name: &str) -> Option<String> {
        key.get_value::<String, _>(name).ok().filter(|v| !v.trim().is_empty())
    }

    fn clean(raw: &str) -> String {
        let trimmed = raw.trim().trim_matches('"');
        expand(trimmed)
    }

    fn expand(text: &str) -> String {
        let mut out = String::new();
        let mut rest = text;
        while let Some(start) = rest.find('%') {
            let Some(len) = rest[start + 1..].find('%') else { break };
            let var = &rest[start + 1..start + 1 + len];
            out.push_str(&rest[..start]);
            out.push_str(&std::env::var(var).unwrap_or_else(|_| format!("%{var}%")));
            rest = &rest[start + len + 2..];
        }
        out.push_str(rest);
        out
    }
}
