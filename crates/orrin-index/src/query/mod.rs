mod find;
mod locate;
mod paths;
mod plan;
mod search;

use orrin_core::api::{Outline, OutlineOutput};
use orrin_core::model::clean_path;
use orrin_core::{Error, Result};

pub use find::find;
pub use locate::locate;
pub use plan::Plan;
pub use search::search;

use crate::engine::Engine;

pub async fn outline(engine: &Engine, input: Outline) -> Result<OutlineOutput> {
    let path = clean_path(&input.path)?;
    let view = input.view.as_ref();
    let index = engine.open(&input.project, view).await?;
    let state = index.read();
    let file = state.file(view, &path).ok_or_else(|| Error::not_found(format!("no such file: {path}")))?;
    Ok(OutlineOutput { clock: state.clock(view), path, items: file.record.outline.clone() })
}
