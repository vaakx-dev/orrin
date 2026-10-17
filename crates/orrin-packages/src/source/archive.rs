use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use orrin_core::Result;

use super::command::run;

pub async fn fetch(url: &str, into: &Path) -> Result<PathBuf> {
    let (download, files) = (into.join("download"), into.join("files"));
    std::fs::create_dir_all(&files)?;
    let curl: [&OsStr; 6] =
        ["-fsSL".as_ref(), "--retry".as_ref(), "2".as_ref(), "-o".as_ref(), download.as_os_str(), url.as_ref()];
    run("curl", &curl, None).await?;
    run(tar(), &["-xf".as_ref(), download.as_os_str(), "-C".as_ref(), files.as_os_str()], None).await?;
    Ok(single_folder(&files).unwrap_or(files))
}

fn single_folder(dir: &Path) -> Option<PathBuf> {
    let mut entries = std::fs::read_dir(dir).ok()?.flatten();
    let only = entries.next()?;
    (entries.next().is_none() && only.file_type().ok()?.is_dir()).then(|| only.path())
}

#[cfg(windows)]
fn tar() -> PathBuf {
    let system = std::env::var_os("SystemRoot").map(|root| PathBuf::from(root).join("System32").join("tar.exe"));
    system.filter(|path| path.is_file()).unwrap_or_else(|| PathBuf::from("tar"))
}

#[cfg(not(windows))]
fn tar() -> PathBuf {
    PathBuf::from("tar")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_one_top_folder() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("lib-1.0/src")).unwrap();
        assert_eq!(single_folder(dir.path()), Some(dir.path().join("lib-1.0")));
        std::fs::write(dir.path().join("README"), "x").unwrap();
        assert_eq!(single_folder(dir.path()), None);
    }
}
