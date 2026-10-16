use std::collections::{HashMap, HashSet};

use orrin_core::{Hash, Result};

use super::Stored;
use crate::disk::DiskStore;
use crate::object::{Manifest, Piece, offsets};

struct Holder {
    file: Hash,
    offset: u64,
}

impl DiskStore {
    pub(super) fn rescue(&self, live: &HashSet<Hash>, files: &[Stored], dying: &[Hash]) -> Result<()> {
        if dying.is_empty() {
            return Ok(());
        }
        let kept = files.iter().filter(|file| live.contains(&file.hash)).map(|file| &file.hash);
        let mut holders = HashMap::new();
        for file in kept {
            for (piece, offset) in offsets(&self.pieces(file)) {
                holders.entry(piece.hash.clone()).or_insert(Holder { file: file.clone(), offset });
            }
        }
        for file in dying {
            for piece in self.pieces(file).iter().filter(|piece| live.contains(&piece.hash)) {
                self.move_out(piece, file, holders.get(&piece.hash))?;
            }
        }
        Ok(())
    }

    fn move_out(&self, piece: &Piece, dying: &Hash, holder: Option<&Holder>) -> Result<()> {
        let Ok(Some(Manifest::Slice { file, .. })) = self.manifest(&piece.hash) else { return Ok(()) };
        if file != *dying {
            return Ok(());
        }
        match holder {
            Some(holder) => self.point(&piece.hash, &holder.file, holder.offset, piece.len),
            None => self.replace_object(&piece.hash, &self.raw_bytes(&piece.hash)?),
        }
    }

    fn pieces(&self, file: &Hash) -> Vec<Piece> {
        match self.manifest(file) {
            Ok(Some(Manifest::Chunks { chunks, .. })) => chunks,
            _ => Vec::new(),
        }
    }
}
