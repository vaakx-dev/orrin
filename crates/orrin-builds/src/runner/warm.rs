use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

use orrin_core::model::{Entry, clean_path};
use orrin_core::par::par_map;
use orrin_core::service::{Materialise, Store};
use orrin_core::{Hash, Result};
use serde::{Deserialize, Serialize};

use super::folder::{self, SCREENSHOTS};

type Busy = Arc<Mutex<HashSet<PathBuf>>>;

#[derive(Default)]
pub struct Pool(Busy);

pub struct Lease {
    busy: Busy,
    dir: PathBuf,
}

impl Pool {
    pub fn lease(&self, root: &Path) -> Lease {
        let mut busy = self.0.lock().unwrap();
        let dir = (0u32..).map(|n| root.join(n.to_string())).find(|dir| !busy.contains(dir)).unwrap();
        busy.insert(dir.clone());
        Lease { busy: self.0.clone(), dir }
    }
}

impl Lease {
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        self.busy.lock().unwrap().remove(&self.dir);
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct Seen {
    hash: Hash,
    size: u64,
    modified: u64,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Synced {
    pub written: usize,
    pub kept: usize,
    pub removed: usize,
}

pub fn sync(store: &dyn Store, dir: &Path, files: &[Entry]) -> Result<Synced> {
    std::fs::create_dir_all(dir)?;
    let state = dir.with_extension("json");
    let old: BTreeMap<String, Seen> =
        std::fs::read(&state).ok().and_then(|bytes| serde_json::from_slice(&bytes).ok()).unwrap_or_default();
    let wanted: HashSet<&str> = files.iter().map(|e| e.path.as_str()).collect();
    let mut removed = 0;
    for path in old.keys().filter(|path| !wanted.contains(path.as_str())) {
        removed += remove_file(dir, path)? as usize;
    }
    let (kept, stale): (Vec<&Entry>, Vec<&Entry>) =
        files.iter().partition(|entry| old.get(&entry.path).is_some_and(|seen| unchanged(dir, entry, seen)));
    let written = par_map(&stale, |entry| -> Result<(String, Seen)> {
        let target = folder::write(store, dir, entry, Materialise::Copy)?;
        Ok((entry.path.clone(), seen(&target, &entry.hash)?))
    });
    let mut next: BTreeMap<String, Seen> = kept.iter().map(|e| (e.path.clone(), old[&e.path].clone())).collect();
    for item in written {
        let (path, seen) = item?;
        next.insert(path, seen);
    }
    let temp = state.with_extension("json.tmp");
    std::fs::write(&temp, serde_json::to_vec(&next)?)?;
    std::fs::rename(temp, state)?;
    let screenshots = dir.join(SCREENSHOTS);
    if screenshots.exists() {
        std::fs::remove_dir_all(&screenshots)?;
    }
    std::fs::create_dir_all(screenshots)?;
    Ok(Synced { written: stale.len(), kept: kept.len(), removed })
}

fn unchanged(dir: &Path, entry: &Entry, seen: &Seen) -> bool {
    let Ok(path) = clean_path(&entry.path) else { return false };
    seen.hash == entry.hash
        && stamp(&dir.join(path)).is_some_and(|(size, modified)| (size, modified) == (seen.size, seen.modified))
}

fn seen(path: &Path, hash: &Hash) -> Result<Seen> {
    let (size, modified) =
        stamp(path).ok_or_else(|| orrin_core::Error::internal(format!("{} vanished", path.display())))?;
    Ok(Seen { hash: hash.clone(), size, modified })
}

fn stamp(path: &Path) -> Option<(u64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos() as u64;
    Some((meta.len(), modified))
}

fn remove_file(dir: &Path, path: &str) -> Result<bool> {
    let target = dir.join(clean_path(path)?);
    match std::fs::remove_file(&target) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err.into()),
    }
    let mut parent = target.parent();
    while let Some(folder) = parent.filter(|p| *p != dir) {
        if std::fs::remove_dir(folder).is_err() {
            break;
        }
        parent = folder.parent();
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::time::SystemTime;

    use orrin_core::model::Mode;

    use super::*;

    #[derive(Default)]
    struct Blobs(Mutex<HashMap<Hash, Vec<u8>>>);

    impl Blobs {
        fn entry(&self, path: &str, body: &str) -> Entry {
            let hash = Hash::of(body.as_bytes());
            self.0.lock().unwrap().insert(hash.clone(), body.as_bytes().to_vec());
            Entry { path: path.into(), hash, size: body.len() as u64, mode: Mode::default() }
        }
    }

    impl orrin_core::service::Service for Blobs {}

    #[rustfmt::skip]
    impl Store for Blobs {
        fn put(&self, _: &[u8]) -> Result<Hash> { unimplemented!() }
        fn put_file(&self, _: &Path) -> Result<Hash> { unimplemented!() }
        fn get(&self, hash: &Hash) -> Result<Vec<u8>> { Ok(self.0.lock().unwrap()[hash].clone()) }
        fn size(&self, _: &Hash) -> Result<u64> { unimplemented!() }
        fn has(&self, _: &Hash) -> bool { unimplemented!() }
        fn missing(&self, _: &[Hash]) -> Vec<Hash> { unimplemented!() }
        fn materialise(&self, hash: &Hash, path: &Path, _: Materialise) -> Result<()> { Ok(std::fs::write(path, self.get(hash)?)?) }
        fn gc(&self, _: &[Hash]) -> Result<orrin_core::service::GcReport> { unimplemented!() }
        fn parts(&self, _: &Hash) -> Result<Vec<Hash>> { unimplemented!() }
        fn raw(&self, _: &Hash) -> Result<Vec<u8>> { unimplemented!() }
        fn put_raw(&self, _: &Hash, _: &[u8]) -> Result<()> { unimplemented!() }
    }

    fn modified(path: &Path) -> SystemTime {
        std::fs::metadata(path).unwrap().modified().unwrap()
    }

    #[test]
    fn sync_rewrites_only_changed_files_and_deletes_removed_ones() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("0");
        let blobs = Blobs::default();
        let first = [blobs.entry("src/a.rs", "a"), blobs.entry("src/b.rs", "b"), blobs.entry("old/c.rs", "c")];
        assert_eq!(sync(&blobs, &dir, &first).unwrap().written, 3);
        std::fs::write(dir.join("target.o"), "built").unwrap();
        let kept_at = modified(&dir.join("src/a.rs"));
        std::thread::sleep(std::time::Duration::from_millis(20));

        let second = [blobs.entry("src/a.rs", "a"), blobs.entry("src/b.rs", "b2")];
        assert_eq!(sync(&blobs, &dir, &second).unwrap(), Synced { written: 1, kept: 1, removed: 1 });
        assert_eq!(modified(&dir.join("src/a.rs")), kept_at);
        assert_eq!(std::fs::read_to_string(dir.join("src/b.rs")).unwrap(), "b2");
        assert!(!dir.join("old").exists());
        assert!(dir.join("target.o").exists());

        let pool = Pool::default();
        let one = pool.lease(root.path());
        let two = pool.lease(root.path());
        assert_ne!(one.dir(), two.dir());
        drop(one);
        assert_eq!(pool.lease(root.path()).dir(), dir);
    }
}
