use std::path::{Path, PathBuf};

use orrin_core::{Error, Result};

pub fn check_deletable(folder: &Path, orrin_home: &Path, user_home: Option<&Path>) -> Result<()> {
    let folder = real(folder);
    let refuse = |why: &str| Err(Error::invalid(format!("will not delete {}: {why}", folder.display())));
    if folder.parent().is_none() {
        return refuse("it is a drive root");
    }
    if user_home.is_some_and(|home| real(home) == folder) {
        return refuse("it is your home folder");
    }
    if real(orrin_home).starts_with(&folder) {
        return refuse("it holds the Orrin home");
    }
    Ok(())
}

pub fn user_home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

pub async fn delete(folder: PathBuf) -> Result<()> {
    let removed = tokio::task::spawn_blocking(move || std::fs::remove_dir_all(folder));
    match removed.await.map_err(|err| Error::internal(err.to_string()))? {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(err.into()),
        _ => Ok(()),
    }
}

fn real(path: &Path) -> PathBuf {
    path.canonicalize().or_else(|_| std::path::absolute(path)).unwrap_or_else(|_| path.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_roots_homes_and_folders_holding_orrin() {
        let dir = tempfile::tempdir().unwrap();
        let (project, orrin) = (dir.path().join("game"), dir.path().join(".orrin"));
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&orrin).unwrap();
        let root = project.ancestors().last().unwrap();
        assert!(check_deletable(&project, &orrin, Some(dir.path())).is_ok());
        assert!(check_deletable(root, &orrin, None).is_err());
        assert!(check_deletable(dir.path(), &orrin, None).is_err());
        assert!(check_deletable(&project, &orrin, Some(&project)).is_err());
    }
}
