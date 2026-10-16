use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};

use crate::diff3::{self, Chunk};

pub type Items<V> = Vec<(String, V)>;

const CONTENT_KEY: char = '#';

pub fn content_key(kind: &str, content: &str) -> String {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{CONTENT_KEY}{kind} {:016x}", hasher.finish())
}

pub fn unique<V>(entries: impl IntoIterator<Item = (String, V)>) -> Items<V> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    entries
        .into_iter()
        .map(|(key, value)| {
            let count = counts.entry(key.clone()).or_default();
            *count += 1;
            match *count {
                1 => (key, value),
                n => (format!("{key} {CONTENT_KEY}{n}"), value),
            }
        })
        .collect()
}

fn union<'a>(chunk: &Chunk<&'a str>) -> Option<Vec<&'a str>> {
    let keys = chunk.ours.iter().chain(chunk.theirs).chain(chunk.base);
    keys.clone().all(|key| !key.starts_with(CONTENT_KEY)).then(|| keys.copied().collect())
}

fn keys<V>(items: &[(String, V)]) -> Vec<&str> {
    items.iter().map(|(key, _)| key.as_str()).collect()
}

fn lookup<V>(items: &[(String, V)]) -> HashMap<&str, &V> {
    items.iter().map(|(key, value)| (key.as_str(), value)).collect()
}

pub fn merge<V>(
    base: &[(String, V)],
    ours: &[(String, V)],
    theirs: &[(String, V)],
    mut value: impl FnMut(&str, Option<&V>, Option<&V>, Option<&V>) -> Result<Option<V>, String>,
) -> Result<Items<V>, String> {
    let order = diff3::merge(&keys(base), &keys(ours), &keys(theirs), union).map_err(|at| {
        let near = base.get(at).map_or("the end", |(key, _)| key.as_str());
        format!("order changed on both sides near {near}")
    })?;
    let (b, o, t) = (lookup(base), lookup(ours), lookup(theirs));
    let leftovers = keys(base).into_iter().chain(keys(ours)).chain(keys(theirs));
    let mut seen = HashSet::new();
    let mut merged = Vec::new();
    for key in order.into_iter().chain(leftovers) {
        if !seen.insert(key) {
            continue;
        }
        let picked = value(key, b.get(key).copied(), o.get(key).copied(), t.get(key).copied())?;
        if let Some(picked) = picked {
            merged.push((key.to_string(), picked));
        }
    }
    Ok(merged)
}
