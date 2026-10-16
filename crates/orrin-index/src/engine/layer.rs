use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use orrin_core::Hash;

use crate::analyse::Record;
use crate::query::Plan;

pub struct FileRef {
    pub id: u32,
    pub hash: Hash,
    pub size: u64,
    pub record: Arc<Record>,
}

#[derive(Default)]
pub struct Layer {
    files: BTreeMap<String, FileRef>,
    paths: HashMap<u32, String>,
    postings: HashMap<u32, Vec<u32>>,
    next: u32,
}

impl Layer {
    pub fn insert(&mut self, path: String, hash: Hash, size: u64, record: Arc<Record>) {
        self.remove(&path);
        let id = self.next;
        self.next += 1;
        for gram in &record.trigrams {
            self.postings.entry(*gram).or_default().push(id);
        }
        self.paths.insert(id, path.clone());
        self.files.insert(path, FileRef { id, hash, size, record });
    }

    pub fn remove(&mut self, path: &str) {
        let Some(file) = self.files.remove(path) else { return };
        self.paths.remove(&file.id);
        for gram in &file.record.trigrams {
            if let Some(ids) = self.postings.get_mut(gram) {
                if let Ok(at) = ids.binary_search(&file.id) {
                    ids.remove(at);
                }
                if ids.is_empty() {
                    self.postings.remove(gram);
                }
            }
        }
    }

    pub fn get(&self, path: &str) -> Option<&FileRef> {
        self.files.get(path)
    }

    pub fn files(&self) -> impl Iterator<Item = (&String, &FileRef)> {
        self.files.iter()
    }

    pub fn hashes(&self) -> HashMap<String, Hash> {
        self.files.iter().map(|(path, file)| (path.clone(), file.hash.clone())).collect()
    }

    pub fn matching(&self, plan: &Plan) -> Vec<(&String, &FileRef)> {
        match self.eval(plan) {
            None => self.files.iter().collect(),
            Some(ids) => ids
                .iter()
                .filter_map(|id| self.paths.get(id))
                .filter_map(|path| self.files.get_key_value(path))
                .collect(),
        }
    }

    fn eval(&self, plan: &Plan) -> Option<Vec<u32>> {
        match plan {
            Plan::All => None,
            Plan::Grams(grams) => Some(self.intersect_grams(grams)),
            Plan::And(parts) => parts.iter().filter_map(|p| self.eval(p)).reduce(|a, b| intersect(&a, &b)),
            Plan::Or(parts) => {
                let mut ids = Vec::new();
                for part in parts {
                    ids.extend(self.eval(part)?);
                }
                ids.sort_unstable();
                ids.dedup();
                Some(ids)
            }
        }
    }

    fn intersect_grams(&self, grams: &[u32]) -> Vec<u32> {
        let mut lists: Vec<&Vec<u32>> = Vec::with_capacity(grams.len());
        for gram in grams {
            match self.postings.get(gram) {
                Some(ids) => lists.push(ids),
                None => return Vec::new(),
            }
        }
        lists.sort_by_key(|ids| ids.len());
        let Some((first, rest)) = lists.split_first() else { return Vec::new() };
        rest.iter().fold(first.to_vec(), |acc, ids| intersect(&acc, ids))
    }
}

fn intersect(a: &[u32], b: &[u32]) -> Vec<u32> {
    a.iter().copied().filter(|id| b.binary_search(id).is_ok()).collect()
}
