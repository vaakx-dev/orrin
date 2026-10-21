use std::cmp::Reverse;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use orrin_core::model::Entry;
use orrin_core::service::Store;
use orrin_core::{Hash, Result, now_ms};
use tokio::sync::Notify;

use super::sync::{self, State, Synced};

const TRASH: &str = ".trash-";

#[derive(Clone, Default)]
pub struct Pool(Arc<Inner>);

#[derive(Default)]
struct Inner {
    roots: Mutex<HashMap<PathBuf, Folders>>,
    freed: Notify,
}

#[derive(Default)]
struct Folders {
    size: usize,
    slots: BTreeMap<u32, Slot>,
}

struct Slot {
    state: Option<State>,
    used: u64,
}

pub struct Lease {
    pool: Pool,
    root: PathBuf,
    number: u32,
    dir: PathBuf,
    state: State,
    pub rewrite: usize,
    pub folders: usize,
}

impl Pool {
    pub async fn lease(&self, root: &Path, files: &[Entry], size: usize) -> Lease {
        loop {
            let freed = self.0.freed.notified();
            tokio::pin!(freed);
            freed.as_mut().enable();
            if let Some(lease) = self.try_lease(root, files, size.max(1)) {
                return lease;
            }
            freed.await;
        }
    }

    pub fn is_full(&self, root: &Path, size: usize) -> bool {
        let mut roots = self.0.roots.lock().unwrap();
        let folders = folders(&mut roots, root);
        folders.slots.len() >= size && folders.slots.values().all(|slot| slot.state.is_none())
    }

    pub fn lease_built(&self, root: &Path, key: &Hash) -> Option<Lease> {
        let mut roots = self.0.roots.lock().unwrap();
        let folders = folders(&mut roots, root);
        let number = folders
            .slots
            .iter()
            .find(|(_, slot)| slot.state.as_ref().is_some_and(|state| state.built.as_ref() == Some(key)))
            .map(|(n, _)| *n)?;
        let count = folders.slots.len();
        Some(self.take(root, folders, number, 0, count))
    }

    pub fn forget_under(&self, dir: &Path) {
        self.0.roots.lock().unwrap().retain(|root, _| !root.starts_with(dir));
    }

    fn try_lease(&self, root: &Path, files: &[Entry], size: usize) -> Option<Lease> {
        let mut roots = self.0.roots.lock().unwrap();
        let folders = folders(&mut roots, root);
        folders.size = size;
        let free = folders.slots.iter().filter_map(|(n, slot)| Some((*n, slot.state.as_ref()?, slot.used)));
        let closest = free.map(|(n, state, used)| (state.rewrite(files), Reverse(used), n)).min();
        let (rewrite, number) = match closest {
            Some((rewrite, _, number)) => (rewrite, number),
            None if folders.slots.len() < size => (files.len(), (0..).find(|n| !folders.slots.contains_key(n))?),
            None => return None,
        };
        folders.slots.entry(number).or_insert_with(|| Slot { state: Some(State::default()), used: now_ms() });
        let count = folders.slots.len();
        let lease = self.take(root, folders, number, rewrite, count);
        evict(root, folders);
        Some(lease)
    }

    fn take(&self, root: &Path, folders: &mut Folders, number: u32, rewrite: usize, count: usize) -> Lease {
        let state = folders.slots.get_mut(&number).and_then(|slot| slot.state.take()).unwrap_or_default();
        Lease {
            pool: self.clone(),
            root: root.to_path_buf(),
            number,
            dir: root.join(number.to_string()),
            state,
            rewrite,
            folders: count,
        }
    }
}

impl Lease {
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn number(&self) -> u32 {
        self.number
    }

    pub fn sync(&mut self, store: &dyn Store, files: &[Entry], linked: &HashSet<String>) -> Result<Synced> {
        self.state.built = None;
        let synced = sync::sync(store, &self.dir, files, linked, &mut self.state.files);
        self.state.save(&self.dir)?;
        synced
    }

    pub fn built(&mut self, key: &Hash) {
        self.state.built = Some(key.clone());
        if let Err(err) = self.state.save(&self.dir) {
            tracing::warn!("could not save warm folder state {}: {err}", self.dir.display());
        }
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        let mut roots = self.pool.0.roots.lock().unwrap();
        let folders = folders(&mut roots, &self.root);
        let state = std::mem::take(&mut self.state);
        folders.slots.insert(self.number, Slot { state: Some(state), used: now_ms() });
        evict(&self.root, folders);
        drop(roots);
        self.pool.0.freed.notify_waiters();
    }
}

fn folders<'a>(roots: &'a mut HashMap<PathBuf, Folders>, root: &Path) -> &'a mut Folders {
    roots.entry(root.to_path_buf()).or_insert_with(|| load(root))
}

fn evict(root: &Path, folders: &mut Folders) {
    while folders.size > 0 && folders.slots.len() > folders.size {
        let oldest = folders.slots.iter().filter(|(_, slot)| slot.state.is_some()).min_by_key(|(_, slot)| slot.used);
        let Some((&number, _)) = oldest else { return };
        folders.slots.remove(&number);
        let dir = root.join(number.to_string());
        let _ = std::fs::remove_file(sync::state_path(&dir));
        let trash = root.join(format!("{TRASH}{number}-{}", now_ms()));
        if std::fs::rename(&dir, &trash).is_ok() {
            std::thread::spawn(move || std::fs::remove_dir_all(trash));
        }
    }
}

fn load(root: &Path) -> Folders {
    let mut slots = BTreeMap::new();
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(TRASH) {
            std::thread::spawn(move || std::fs::remove_dir_all(entry.path()));
            continue;
        }
        let Ok(number) = name.parse::<u32>() else { continue };
        if entry.path().is_dir() {
            let (state, used) = State::load(&entry.path());
            slots.insert(number, Slot { state: Some(state), used });
        }
    }
    Folders { size: 0, slots }
}

#[cfg(test)]
mod tests {
    use orrin_core::model::Mode;

    use super::*;

    fn entry(path: &str, body: &str) -> Entry {
        Entry { path: path.into(), hash: Hash::of(body.as_bytes()), size: 1, mode: Mode::default() }
    }

    fn remember(lease: &mut Lease, files: &[Entry]) {
        lease.state.files = files.iter().map(|e| (e.path.clone(), sync::seen_as(&e.hash))).collect();
    }

    #[tokio::test]
    async fn picks_the_closest_free_folder_and_evicts_beyond_the_pool() {
        let root = tempfile::tempdir().unwrap();
        let pool = Pool::default();
        let old = [entry("a", "1"), entry("b", "1")];
        let new = [entry("a", "2"), entry("b", "2")];
        let mut first = pool.lease(root.path(), &old, 2).await;
        let mut second = pool.lease(root.path(), &old, 2).await;
        assert_ne!(first.number(), second.number());
        assert!(pool.is_full(root.path(), 2));
        remember(&mut first, &old);
        remember(&mut second, &new);
        let newer = second.number();
        drop((first, second));

        let lease = pool.lease(root.path(), &new, 2).await;
        assert_eq!((lease.number(), lease.rewrite), (newer, 0));
        drop(lease);
        let lease = pool.lease(root.path(), &old, 1).await;
        assert_eq!(lease.rewrite, 0);
        assert_eq!(pool.0.roots.lock().unwrap()[root.path()].slots.len(), 1);
    }
}
