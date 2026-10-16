use orrin_core::{Error, Hash, ProjectId, Result, ViewId};

use super::Engine;
use super::content::blocking;
use crate::analyse::Kind;
use crate::documents::{self, Block};
use crate::records::key;

pub struct Loaded {
    pub hash: Hash,
    pub size: u64,
    pub blocks: Vec<Block>,
}

impl Engine {
    pub async fn document(&self, project: &ProjectId, view: Option<&ViewId>, path: &str) -> Result<Loaded> {
        let kind = Kind::of(path);
        let Kind::Document(format) = kind else {
            return Err(Error::invalid(format!("anchors only work in documents, not {path}")));
        };
        let missing = || Error::not_found(format!("no file {path}"));
        let (hash, size, bytes) = match view {
            Some(view) => {
                let file = self.read_view(project, view, path).await?.ok_or_else(missing)?;
                (file.hash, file.size, file.bytes)
            }
            None => {
                let index = self.open(project, None).await?;
                let file = index.read().file(None, path).ok_or_else(missing)?;
                (file.hash, file.size, None)
            }
        };
        let records = self.records.clone();
        let store = self.hub.store.clone();
        blocking(move || {
            let key = key(kind, &hash);
            if let Some(blocks) = records.blocks(&key) {
                return Ok(Loaded { hash, size, blocks });
            }
            let bytes = match bytes {
                Some(bytes) => bytes,
                None => store.get(&hash)?,
            };
            let blocks = documents::extract(format, &bytes).blocks;
            records.put_blocks(&key, &blocks)?;
            Ok(Loaded { hash, size, blocks })
        })
        .await?
    }
}
