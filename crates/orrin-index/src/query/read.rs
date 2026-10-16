use orrin_core::api::{Read, ReadOutput};
use orrin_core::model::{Encoding, clean_path};
use orrin_core::{Error, Result};

use crate::documents::{render, select};
use crate::engine::Engine;

pub async fn read_anchor(engine: &Engine, input: Read) -> Result<ReadOutput> {
    let path = clean_path(&input.path)?;
    let view = input.view.as_ref();
    let anchor = input.anchor.as_deref().unwrap_or_default();
    let doc = engine.document(&input.project, view, &path).await?;
    let picked = select(&doc.blocks, anchor);
    if picked.is_empty() && !anchor.trim().is_empty() {
        return Err(Error::not_found(format!("no {anchor} in {path}")));
    }
    let text = render(&picked);
    let all: Vec<&str> = text.split_inclusive('\n').collect();
    let start = input.offset.unwrap_or(1).max(1) as usize - 1;
    let take = input.limit.map_or(usize::MAX, |l| l as usize);
    Ok(ReadOutput {
        clock: engine.clock(&input.project, view).await,
        path,
        hash: doc.hash,
        size: doc.size,
        binary: false,
        total_lines: Some(all.len() as u32),
        encoding: Encoding::Utf8,
        content: Some(all.iter().skip(start).take(take).copied().collect()),
    })
}
