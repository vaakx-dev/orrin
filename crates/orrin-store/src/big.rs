use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::PathBuf;

use fastcdc::v2020::StreamCDC;
use orrin_core::{Error, Hash, Result};
use rayon::prelude::*;

use crate::disk::{DiskStore, place_new};
use crate::object::{Manifest, Piece, offsets};
use crate::seal::seal;
use crate::slices::Slice;
use crate::verified::Verified;

const MIN: usize = 16 * 1024;
const AVG: usize = 64 * 1024;
const MAX: usize = 256 * 1024;

impl DiskStore {
    pub(crate) fn put_big(&self, reader: impl Read) -> Result<Hash> {
        let temp = self.temp_path();
        let written = self.write_big(reader, &temp);
        let (hash, pieces) = written.inspect_err(|_| {
            let _ = fs::remove_file(&temp);
        })?;
        if self.file_path(&hash).is_file() {
            let _ = fs::remove_file(&temp);
            self.touch(&hash);
            return Ok(hash);
        }
        seal(&temp)?;
        place_new(&temp, &self.file_path(&hash))?;
        self.index(&hash, &pieces)?;
        let size = pieces.iter().map(|piece| piece.len).sum();
        self.write_object(&hash, &Manifest::Chunks { size, chunks: pieces }.encode())?;
        Ok(hash)
    }

    pub(crate) fn full_file(&self, hash: &Hash, pieces: &[Piece]) -> Result<PathBuf> {
        let path = self.file_path(hash);
        if path.is_file() {
            return Ok(path);
        }
        let temp = self.temp_path();
        let assembled = self.assemble(hash, pieces, &temp).and_then(|()| Ok(seal(&temp)?));
        assembled.inspect_err(|_| {
            let _ = fs::remove_file(&temp);
        })?;
        place_new(&temp, &path)?;
        self.index(hash, pieces)?;
        pieces.par_iter().for_each(|piece| _ = fs::remove_file(self.object_path(&piece.hash)));
        Ok(path)
    }

    pub(crate) fn point(&self, chunk: &Hash, file: &Hash, offset: u64, len: u64) -> Result<()> {
        self.slices.point(file, &Slice { chunk, offset, len })?;
        let _ = fs::remove_file(self.object_path(chunk));
        Ok(())
    }

    fn write_big(&self, reader: impl Read, temp: &std::path::Path) -> Result<(Hash, Vec<Piece>)> {
        let mut out = BufWriter::new(File::create(temp)?);
        let mut hasher = blake3::Hasher::new();
        let mut pieces = Vec::new();
        for chunk in StreamCDC::new(reader, MIN, AVG, MAX) {
            let chunk = chunk.map_err(|err| Error::internal(format!("chunking failed: {err}")))?;
            hasher.update(&chunk.data);
            out.write_all(&chunk.data)?;
            pieces.push(Piece { hash: Hash::of(&chunk.data), len: chunk.length as u64 });
        }
        out.into_inner().map_err(|err| err.into_error())?;
        Ok((Hash::from_blake3(hasher.finalize()), pieces))
    }

    fn assemble(&self, hash: &Hash, pieces: &[Piece], temp: &std::path::Path) -> Result<()> {
        let mut out = Verified::new(BufWriter::new(File::create(temp)?));
        for piece in pieces {
            let bytes = self.read_all(&piece.hash)?;
            if bytes.len() as u64 != piece.len {
                return Err(Error::internal(format!("chunk {} of {hash} has the wrong size", piece.hash)));
            }
            out.write_all(&bytes)?;
        }
        out.finish(hash)?.into_inner().map_err(|err| err.into_error())?;
        Ok(())
    }

    fn index(&self, file: &Hash, pieces: &[Piece]) -> Result<()> {
        let slices: Vec<Slice> =
            offsets(pieces).map(|(piece, offset)| Slice { chunk: &piece.hash, offset, len: piece.len }).collect();
        self.slices.add(file, &slices)
    }
}
