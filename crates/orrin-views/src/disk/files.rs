use std::fs;
use std::path::{Path, PathBuf};

use orrin_core::Result;
use orrin_core::id::fresh;

const TEMP_MARK: &str = ".orrin-tmp-";

pub fn is_temp(rel: &str) -> bool {
    rel.contains(TEMP_MARK)
}

pub fn write_fresh(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = temp_beside(path);
    fs::write(&tmp, bytes)?;
    replace(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

pub fn remove_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err.into()),
        _ => Ok(()),
    }
}

pub fn remove_any(path: &Path) {
    let _ = if path.is_dir() { fs::remove_dir_all(path) } else { fs::remove_file(path) };
}

pub fn retired(root: &Path) -> PathBuf {
    root.with_extension("landed")
}

pub fn retire(root: &Path) {
    if let Err(err) = fs::rename(root, retired(root)) {
        tracing::warn!("could not move landed view folder {} aside: {err}", root.display());
    }
}

fn replace(tmp: &Path, path: &Path) -> Result<()> {
    remove_if_present(path)?;
    fs::rename(tmp, path)?;
    Ok(())
}

fn temp_beside(path: &Path) -> PathBuf {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    path.with_file_name(format!(".{name}{TEMP_MARK}{}", fresh("w")))
}

#[cfg(unix)]
pub fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(perms.mode() | 0o111);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
pub fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}

pub fn is_sealed(path: &Path) -> bool {
    fs::OpenOptions::new().write(true).open(path).is_err_and(|err| err.kind() == std::io::ErrorKind::PermissionDenied)
}

#[cfg(windows)]
pub fn is_locked(path: &Path) -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    match fs::OpenOptions::new().read(true).share_mode(0).open(path) {
        Ok(_) => false,
        Err(err) => matches!(err.raw_os_error(), Some(32) | Some(33)),
    }
}

#[cfg(not(windows))]
pub fn is_locked(_path: &Path) -> bool {
    false
}
