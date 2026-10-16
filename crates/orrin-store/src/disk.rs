use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::AtomicU8;
use std::time::{Duration, Instant};

use orrin_core::id::fresh;
use orrin_core::{Hash, Result};

const GRACE: Duration = Duration::from_secs(15 * 60);

pub struct DiskStore {
    root: PathBuf,
    pub(crate) grace: Duration,
    reput: Mutex<HashMap<Hash, Instant>>,
    pub(crate) sealed: Mutex<HashSet<PathBuf>>,
    pub(crate) clones: AtomicU8,
}

impl DiskStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        for dir in ["objects", "files", "tmp"] {
            fs::create_dir_all(root.join(dir))?;
        }
        Ok(Self { root, grace: GRACE, reput: Mutex::default(), sealed: Mutex::default(), clones: AtomicU8::new(0) })
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
        if self.exists(hash) {
            self.touch(hash);
            return Ok(());
        }
        let temp = self.temp_path();
        fs::write(&temp, bytes)?;
        place_new(&temp, &self.object_path(hash))
    }

    pub(crate) fn replace_object(&self, hash: &Hash, bytes: &[u8]) -> Result<()> {
        let temp = self.temp_path();
        fs::write(&temp, bytes)?;
        let path = self.object_path(hash);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&temp, &path).inspect_err(|_| {
            let _ = fs::remove_file(&temp);
        })?;
        self.sealed.lock().unwrap_or_else(|e| e.into_inner()).remove(&path);
        Ok(())
    }

    pub(crate) fn touch(&self, hash: &Hash) {
        self.reput.lock().unwrap_or_else(|e| e.into_inner()).insert(hash.clone(), Instant::now());
    }

    pub(crate) fn reput_recently(&self) -> HashSet<Hash> {
        let mut reput = self.reput.lock().unwrap_or_else(|e| e.into_inner());
        reput.retain(|_, at| at.elapsed() < self.grace);
        reput.keys().cloned().collect()
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
    if path.is_file() {
        let _ = fs::remove_file(temp);
        return Ok(());
    }
    match fs::rename(temp, path) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = fs::remove_file(temp);
            if path.is_file() { Ok(()) } else { Err(err.into()) }
        }
    }
}
