use std::fs;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use orrin_core::api::{Create, Delete, Move, Patch, Touched, Written};
use orrin_core::model::{Encoding, clean_path};
use orrin_core::{Error, Hash, Result};

use crate::content::text::apply;
use crate::disk::files::write_fresh;
use crate::inner::Inner;
use crate::slot::Slot;

impl Inner {
    pub async fn patch(&self, input: Patch) -> Result<Written> {
        let slot = self.open_slot(&input.project, &input.view).await?;
        let path = clean_path(&input.path)?;
        let record = slot.record.lock().await;
        let bytes = fs::read(slot.root.join(&path)).map_err(|_| Error::not_found(format!("no file {path}")))?;
        let text = String::from_utf8(bytes).map_err(|_| Error::invalid(format!("{path} is not text")))?;
        let next = apply(&text, &input.edits)?;
        let hash = self.write_file(&slot, &path, next.as_bytes())?;
        drop(record);
        let clock = self.touched(&slot, vec![path.clone()]).await;
        Ok(Written { clock, path, hash, size: next.len() as u64 })
    }

    pub async fn create_file(&self, input: Create) -> Result<Written> {
        let slot = self.open_slot(&input.project, &input.view).await?;
        let path = clean_path(&input.path)?;
        let bytes = match input.encoding {
            Encoding::Utf8 => input.content.into_bytes(),
            Encoding::Base64 => {
                STANDARD.decode(&input.content).map_err(|e| Error::invalid(format!("bad base64: {e}")))?
            }
        };
        let record = slot.record.lock().await;
        let abs = slot.root.join(&path);
        if abs.is_dir() || (abs.exists() && !input.overwrite) {
            return Err(Error::conflict(format!("{path} already exists")));
        }
        let hash = self.write_file(&slot, &path, &bytes)?;
        drop(record);
        let clock = self.touched(&slot, vec![path.clone()]).await;
        Ok(Written { clock, path, hash, size: bytes.len() as u64 })
    }

    pub async fn move_file(&self, input: Move) -> Result<Touched> {
        let slot = self.open_slot(&input.project, &input.view).await?;
        let (from, to) = (clean_path(&input.from)?, clean_path(&input.to)?);
        let record = slot.record.lock().await;
        let (src, dst) = (slot.root.join(&from), slot.root.join(&to));
        if !src.exists() {
            return Err(Error::not_found(format!("no file {from}")));
        }
        if dst.exists() {
            return Err(Error::conflict(format!("{to} already exists")));
        }
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::rename(&src, &dst)?;
        if dst.is_dir() {
            slot.stats().forget_under(&from);
        } else {
            slot.stats().rename(&from, &to);
        }
        drop(record);
        let paths = vec![from, to];
        Ok(Touched { clock: self.touched(&slot, paths.clone()).await, paths })
    }

    pub async fn delete(&self, input: Delete) -> Result<Touched> {
        let slot = self.open_slot(&input.project, &input.view).await?;
        let path = clean_path(&input.path)?;
        let record = slot.record.lock().await;
        let abs = slot.root.join(&path);
        let meta = fs::symlink_metadata(&abs).map_err(|_| Error::not_found(format!("no file {path}")))?;
        let paths = if meta.is_dir() {
            fs::remove_dir_all(&abs)?;
            let gone = slot.stats().forget_under(&path);
            if gone.is_empty() { vec![path] } else { gone }
        } else {
            fs::remove_file(&abs)?;
            slot.stats().forget(&path);
            vec![path]
        };
        drop(record);
        Ok(Touched { clock: self.touched(&slot, paths.clone()).await, paths })
    }

    fn write_file(&self, slot: &Slot, path: &str, bytes: &[u8]) -> Result<Hash> {
        let abs = slot.root.join(path);
        write_fresh(&abs, bytes)?;
        let hash = self.hub.store.put(bytes)?;
        slot.stats().remember_path(path, &abs, hash.clone())?;
        Ok(hash)
    }
}
