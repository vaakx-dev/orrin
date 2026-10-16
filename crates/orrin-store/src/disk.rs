use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use orrin_core::id::fresh;
use orrin_core::{Hash, Result};

const GRACE: Duration = Duration::from_secs(15 * 60);

pub struct DiskStore {
    root: PathBuf,
    pub(crate) grace: Duration,
}

impl DiskStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        for dir in ["objects", "files", "tmp"] {
            fs::create_dir_all(root.join(dir))?;
        }
        Ok(Self { root, grace: GRACE })
    }

    pub fn with_grace(mut self, grace: Duration) -> Self {
        self.grace = grace;
        self
    }

    pub(crate) fn objects_dir(&self) -> PathBuf {
        self.root.join("objects")
    }

    pub(crate) fn files_dir(&self) -> PathBuf {
        self.root.join("files")
    }

    pub(crate) fn tmp_dir(&self) -> PathBuf {
        self.root.join("tmp")
    }

    pub(crate) fn object_path(&self, hash: &Hash) -> PathBuf {
        fan_out(&self.objects_dir(), hash)
    }

    pub(crate) fn file_path(&self, hash: &Hash) -> PathBuf {
        fan_out(&self.files_dir(), hash)
    }

    pub(crate) fn temp_path(&self) -> PathBuf {
        self.tmp_dir().join(fresh("t"))
    }

    pub(crate) fn exists(&self, hash: &Hash) -> bool {
        self.object_path(hash).is_file()
    }

    pub(crate) fn write_object(&self, hash: &Hash, bytes: &[u8]) -> Result<()> {
        let path = self.object_path(hash);
        if path.is_file() {
            return touch(&path);
        }
        let temp = self.temp_path();
        fs::write(&temp, bytes)?;
        place_new(&temp, &path)
    }
}

fn fan_out(dir: &Path, hash: &Hash) -> PathBuf {
    let (head, tail) = hash.as_str().split_at(2);
    dir.join(head).join(tail)
}

pub(crate) fn place_new(temp: &Path, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::rename(temp, path) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = fs::remove_file(temp);
            if path.is_file() { Ok(()) } else { Err(err.into()) }
        }
    }
}

pub(crate) fn touch(path: &Path) -> Result<()> {
    fs::File::options().write(true).open(path)?.set_modified(SystemTime::now())?;
    Ok(())
}
