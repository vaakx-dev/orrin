use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicU8;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use orrin_core::id::fresh;
use orrin_core::model::Snapshot;
use orrin_core::{Hash, Result};

use crate::slices::Slices;

const GRACE: Duration = Duration::from_secs(15 * 60);
const WAIT_STEP: Duration = Duration::from_millis(10);
const WAIT_STEPS: u32 = 200;

pub struct DiskStore {
    root: PathBuf,
    pub(crate) grace: Duration,
    reput: Mutex<HashMap<Hash, Instant>>,
    pub(crate) sealed: Mutex<HashSet<PathBuf>>,
    pub(crate) clones: AtomicU8,
    pub(crate) snapshots: Mutex<HashMap<Hash, Snapshot>>,
    pub(crate) slices: Slices,
}

impl DiskStore {
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        for dir in ["objects", "files", "tmp"] {
            fs::create_dir_all(root.join(dir))?;
        }
        Ok(Self {
            grace: GRACE,
            reput: Mutex::default(),
            sealed: Mutex::default(),
            clones: AtomicU8::new(0),
            snapshots: Mutex::default(),
            slices: Slices::open(&root.join("slices.db"))?,
            root,
        })
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
        let written =
            fs::metadata(self.object_path(hash)).is_ok_and(|m| m.is_file() && (m.len() > 0 || is_empty(hash)));
        written || self.slices.has(hash)
    }

    pub(crate) fn write_object(&self, hash: &Hash, bytes: &[u8]) -> Result<()> {
        if self.exists(hash) {
            self.touch(hash);
            return Ok(());
        }
        let path = self.object_path(hash);
        match in_dir(&path, || File::create_new(&path)) {
            Ok(mut file) => file.write_all(bytes).map_err(|err| {
                drop(file);
                let _ = fs::remove_file(&path);
                err.into()
            }),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => self.await_object(hash, &path, bytes),
            Err(err) => Err(err.into()),
        }
    }

    fn await_object(&self, hash: &Hash, path: &Path, bytes: &[u8]) -> Result<()> {
        for _ in 0..WAIT_STEPS {
            if fs::metadata(path).is_ok_and(|m| m.len() == bytes.len() as u64) {
                return Ok(());
            }
            std::thread::sleep(WAIT_STEP);
        }
        self.replace_object(hash, bytes)
    }

    pub(crate) fn replace_object(&self, hash: &Hash, bytes: &[u8]) -> Result<()> {
        let temp = self.temp_path();
        fs::write(&temp, bytes)?;
        let path = self.object_path(hash);
        in_dir(&path, || fs::rename(&temp, &path)).inspect_err(|_| {
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

fn is_empty(hash: &Hash) -> bool {
    static EMPTY: OnceLock<Hash> = OnceLock::new();
    hash == EMPTY.get_or_init(|| Hash::of(b""))
}

fn fan_out(dir: &Path, hash: &Hash) -> PathBuf {
    let (head, tail) = hash.as_str().split_at(2);
    dir.join(head).join(tail)
}

pub(crate) fn in_dir<T>(path: &Path, op: impl Fn() -> io::Result<T>) -> io::Result<T> {
    match op() {
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)?;
            }
            op()
        }
        other => other,
    }
}

pub(crate) fn place_new(temp: &Path, path: &Path) -> Result<()> {
    if path.is_file() {
        let _ = fs::remove_file(temp);
        return Ok(());
    }
    match in_dir(path, || fs::rename(temp, path)) {
        Ok(()) => Ok(()),
        Err(err) => {
            let _ = fs::remove_file(temp);
            if path.is_file() { Ok(()) } else { Err(err.into()) }
        }
    }
}
