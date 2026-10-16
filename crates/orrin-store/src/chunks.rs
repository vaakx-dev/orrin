use std::io::Read;

use fastcdc::v2020::{FastCDC, StreamCDC};
use orrin_core::{Error, Hash, Result};

use crate::disk::DiskStore;
use crate::object::Manifest;

const MIN: usize = 16 * 1024;
const AVG: usize = 64 * 1024;
const MAX: usize = 256 * 1024;

impl DiskStore {
    pub(crate) fn put_chunked(&self, hash: &Hash, bytes: &[u8]) -> Result<()> {
        let chunks = FastCDC::new(bytes, MIN, AVG, MAX)
            .map(|chunk| self.put_chunk(&bytes[chunk.offset..chunk.offset + chunk.length]))
            .collect::<Result<Vec<_>>>()?;
        self.write_object(hash, &Manifest::Chunks { size: bytes.len() as u64, chunks }.encode())
    }

    pub(crate) fn put_stream(&self, reader: impl Read) -> Result<Hash> {
        let mut hasher = blake3::Hasher::new();
        let mut chunks = Vec::new();
        let mut size = 0;
        for chunk in StreamCDC::new(reader, MIN, AVG, MAX) {
            let chunk = chunk.map_err(|err| Error::internal(format!("chunking failed: {err}")))?;
            hasher.update(&chunk.data);
            size += chunk.length as u64;
            chunks.push(self.put_chunk(&chunk.data)?);
        }
        let hash = Hash::from_blake3(hasher.finalize());
        self.write_object(&hash, &Manifest::Chunks { size, chunks }.encode())?;
        Ok(hash)
    }

    fn put_chunk(&self, bytes: &[u8]) -> Result<Hash> {
        let hash = Hash::of(bytes);
        self.write_object(&hash, bytes)?;
        Ok(hash)
    }
}
