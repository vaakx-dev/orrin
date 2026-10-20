use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use orrin_core::api::{Read, ReadOutput};
use orrin_core::model::{Encoding, clean_path};
use orrin_core::{Clock, Error, Hash, Result};

use crate::content::text::{is_binary, lines};
use crate::inner::Inner;

impl Inner {
    pub async fn read(&self, input: Read) -> Result<ReadOutput> {
        if input.anchor.is_some() {
            return self.hub.index.read_anchor(input).await;
        }
        let path = clean_path(&input.path)?;
        let (bytes, clock) = match &input.view {
            Some(id) => {
                let slot = self.open_slot(&input.project, id).await?;
                let bytes =
                    std::fs::read(slot.root.join(&path)).map_err(|_| Error::not_found(format!("no file {path}")))?;
                (bytes, self.hub.index.clock(&input.project, Some(id)).await)
            }
            None => {
                let project = self.hub.projects.get(&input.project).await?;
                let snapshot = self.hub.store.snapshot(&project.main)?;
                let entry = snapshot.get(&path).ok_or_else(|| Error::not_found(format!("no file {path}")))?;
                (self.hub.store.get(&entry.hash)?, self.hub.index.clock(&input.project, None).await)
            }
        };
        Ok(render(path, bytes, &input, clock))
    }
}

fn render(path: String, bytes: Vec<u8>, input: &Read, clock: Clock) -> ReadOutput {
    let hash = Hash::of(&bytes);
    let size = bytes.len() as u64;
    let binary = is_binary(&bytes);
    let mut out =
        ReadOutput { clock, path, hash, size, binary, total_lines: None, encoding: Encoding::Utf8, content: None };
    if input.encoding == Some(Encoding::Base64) {
        out.encoding = Encoding::Base64;
        out.content = Some(STANDARD.encode(&bytes));
    } else if !binary {
        let (text, total) = lines(&String::from_utf8_lossy(&bytes), input.offset, input.limit);
        out.total_lines = Some(total);
        out.content = Some(text);
    }
    out
}
