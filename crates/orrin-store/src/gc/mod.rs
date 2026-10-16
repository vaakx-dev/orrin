mod rescue;

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use orrin_core::model::Snapshot;
use orrin_core::service::GcReport;
use orrin_core::{Hash, Result};

use crate::disk::DiskStore;

const SNAPSHOT_HEAD: &[u8] = br#"{"entries":"#;
const SNAPSHOT_LIMIT: u64 = 256 * 1024 * 1024;

struct Stored {
    hash: Hash,
    path: PathBuf,
    len: u64,
    recent: bool,
}

impl DiskStore {
    pub(crate) fn collect(&self, roots: &[Hash]) -> Result<GcReport> {
        let objects = self.listing(&self.objects_dir())?;
        let files = self.listing(&self.files_dir())?;
        let mut start = roots.to_vec();
        start.extend(roots.iter().flat_map(|root| self.snapshot_entries(root)));
        start.extend(objects.iter().chain(&files).filter(|stored| stored.recent).map(|stored| stored.hash.clone()));
        start.extend(self.reput_recently());
        let live = self.mark(start);
        let dying: Vec<Hash> = files.iter().filter(|f| !live.contains(&f.hash)).map(|f| f.hash.clone()).collect();
        self.rescue(&live, &files, &dying)?;
        let mut report = GcReport::default();
        for stored in objects.iter().chain(&files) {
            if !live.contains(&stored.hash) && fs::remove_file(&stored.path).is_ok() {
                report.removed += 1;
                report.freed += stored.len;
            }
        }
        self.clean_tmp();
        Ok(report)
    }

    pub(crate) fn closure(&self, hash: &Hash) -> Result<Vec<Hash>> {
        let mut stack = self.manifest(hash)?.map(|manifest| manifest.parts()).unwrap_or_default();
        let mut seen = HashSet::new();
        let mut parts = Vec::new();
        while let Some(part) = stack.pop() {
            if seen.insert(part.clone()) {
                if let Ok(Some(manifest)) = self.manifest(&part) {
                    stack.extend(manifest.parts());
                }
                parts.push(part);
            }
        }
        Ok(parts)
    }

    fn mark(&self, mut stack: Vec<Hash>) -> HashSet<Hash> {
        let mut live = HashSet::new();
        while let Some(hash) = stack.pop() {
            if live.insert(hash.clone())
                && let Ok(Some(manifest)) = self.manifest(&hash)
            {
                stack.extend(manifest.parts());
            }
        }
        live
    }

    fn snapshot_entries(&self, root: &Hash) -> Vec<Hash> {
        let small = self.object_size(root).is_ok_and(|size| size <= SNAPSHOT_LIMIT);
        let bytes = if small { self.read_all(root).unwrap_or_default() } else { Vec::new() };
        if !bytes.starts_with(SNAPSHOT_HEAD) {
            return Vec::new();
        }
        Snapshot::from_bytes(&bytes)
            .map(|snapshot| snapshot.entries.into_iter().map(|e| e.hash).collect())
            .unwrap_or_default()
    }

    fn listing(&self, dir: &Path) -> Result<Vec<Stored>> {
        let mut found = Vec::new();
        for fan in fs::read_dir(dir)?.flatten() {
            let head = fan.file_name().to_string_lossy().into_owned();
            for entry in fs::read_dir(fan.path()).into_iter().flatten().flatten() {
                let name = format!("{head}{}", entry.file_name().to_string_lossy());
                let (Ok(hash), Ok(meta)) = (Hash::parse(&name), entry.metadata()) else { continue };
                let recent = self.is_recent(meta.modified().ok());
                found.push(Stored { hash, path: entry.path(), len: meta.len(), recent });
            }
        }
        Ok(found)
    }

    fn clean_tmp(&self) {
        for entry in fs::read_dir(self.tmp_dir()).into_iter().flatten().flatten() {
            if !self.is_recent(entry.metadata().ok().and_then(|meta| meta.modified().ok())) {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    fn is_recent(&self, modified: Option<SystemTime>) -> bool {
        modified.and_then(|at| at.elapsed().ok()).is_none_or(|age| age < self.grace)
    }
}
