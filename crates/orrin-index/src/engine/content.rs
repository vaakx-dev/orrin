use std::sync::Arc;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use orrin_core::api::{Read, ReadOutput};
use orrin_core::model::Encoding;
use orrin_core::service::Store;
use orrin_core::{Error, ErrorCode, Hash, ProjectId, Result, ViewId};

use super::Engine;
use crate::analyse::{Analysis, Kind, MAX_INDEXED, Record, analyse};
use crate::records::{Row, key};
use orrin_core::par::par_map;

pub struct Item {
    pub path: String,
    pub hash: Hash,
    pub size: u64,
    pub bytes: Option<Vec<u8>>,
}

pub struct ViewFile {
    pub hash: Hash,
    pub size: u64,
    pub bytes: Option<Vec<u8>>,
}

pub type Resolved = Vec<(Item, Arc<Record>)>;

impl Engine {
    pub(super) async fn resolve(&self, items: Vec<Item>, view: Option<(&ProjectId, &ViewId)>) -> Result<Resolved> {
        let records = self.records.clone();
        let keyed: Vec<(Item, String)> = items
            .into_iter()
            .map(|item| {
                let key = key(Kind::of(&item.path), &item.hash);
                (item, key)
            })
            .collect();
        let (keyed, cached) = blocking(move || {
            let cached: Vec<_> = keyed.iter().map(|(_, key)| records.get(key)).collect();
            (keyed, cached)
        })
        .await?;
        let mut done = Vec::new();
        let mut misses = Vec::new();
        for ((item, key), record) in keyed.into_iter().zip(cached) {
            match record {
                Some(record) => done.push((item, record)),
                None => misses.push((item, key)),
            }
        }
        if let Some((project, view)) = view {
            for (item, _) in &mut misses {
                if item.bytes.is_none() && wanted(item) && !self.hub.store.has(&item.hash) {
                    item.bytes = self.read_view(project, view, &item.path).await?.and_then(|f| f.bytes);
                }
            }
        }
        done.extend(self.analyse_all(misses).await?);
        Ok(done)
    }

    pub(crate) async fn read_view(&self, project: &ProjectId, view: &ViewId, path: &str) -> Result<Option<ViewFile>> {
        let read = Read {
            project: project.clone(),
            view: Some(view.clone()),
            path: path.to_string(),
            offset: None,
            limit: None,
            encoding: Some(Encoding::Base64),
            anchor: None,
        };
        match self.hub.views.read(read).await {
            Ok(out) => Ok(Some(ViewFile { bytes: decode(&out), hash: out.hash, size: out.size })),
            Err(err) if err.code == ErrorCode::NotFound => Ok(None),
            Err(err) => Err(err),
        }
    }

    async fn analyse_all(&self, misses: Vec<(Item, String)>) -> Result<Resolved> {
        let store = self.hub.store.clone();
        let records = self.records.clone();
        blocking(move || {
            let analysed = par_map(&misses, |(item, _)| analyse_item(&*store, item));
            let mut fresh = Vec::new();
            let mut unread = Vec::new();
            for ((mut item, key), analysis) in misses.into_iter().zip(analysed) {
                item.bytes = None;
                match analysis {
                    Some(Analysis { record, blocks }) => fresh.push((item, Row { key, record, blocks })),
                    None => unread.push((item, Arc::new(Record::default()))),
                }
            }
            let (items, rows): (Vec<Item>, Vec<Row>) = fresh.into_iter().unzip();
            let stored = records.put_all(rows)?;
            Ok(items.into_iter().zip(stored).chain(unread).collect())
        })
        .await?
    }
}

pub async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(work).await.map_err(|e| Error::internal(e.to_string()))
}

pub fn decode(out: &ReadOutput) -> Option<Vec<u8>> {
    let content = out.content.as_ref()?;
    match out.encoding {
        Encoding::Utf8 => Some(content.clone().into_bytes()),
        Encoding::Base64 => STANDARD.decode(content).ok(),
    }
}

fn wanted(item: &Item) -> bool {
    Kind::of(&item.path) != Kind::Skip && item.size <= MAX_INDEXED
}

fn analyse_item(store: &dyn Store, item: &Item) -> Option<Analysis> {
    if !wanted(item) {
        return Some(Analysis::default());
    }
    let kind = Kind::of(&item.path);
    match &item.bytes {
        Some(bytes) => Some(analyse(kind, bytes)),
        None => store.get(&item.hash).ok().map(|bytes| analyse(kind, &bytes)),
    }
}
