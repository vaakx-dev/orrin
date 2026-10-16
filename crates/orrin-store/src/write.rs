use std::fs::File;
use std::io::{BufReader, Read, Seek};
use std::path::Path;

use orrin_core::config::BIG_FILE;
use orrin_core::{Error, Hash, Result};

use crate::disk::{DiskStore, touch};
use crate::object::Manifest;
use crate::zipped;

const ZIP_LIMIT: u64 = 512 * 1024 * 1024;

impl DiskStore {
    pub(crate) fn put_bytes(&self, bytes: &[u8]) -> Result<Hash> {
        let hash = Hash::of(bytes);
        if self.exists(&hash) {
            touch(&self.object_path(&hash))?;
            return Ok(hash);
        }
        if !(zipped::looks_zipped(bytes) && self.put_zipped(&hash, bytes)?) {
            self.put_plain_at(&hash, bytes)?;
        }
        Ok(hash)
    }

    pub(crate) fn put_plain(&self, bytes: &[u8]) -> Result<Hash> {
        let hash = Hash::of(bytes);
        self.put_plain_at(&hash, bytes)?;
        Ok(hash)
    }

    pub(crate) fn put_path(&self, path: &Path) -> Result<Hash> {
        let mut file = File::open(path)?;
        let len = file.metadata()?.len();
        let mut head = Vec::new();
        (&mut file).take(4).read_to_end(&mut head)?;
        file.rewind()?;
        if len <= BIG_FILE || (zipped::looks_zipped(&head) && len <= ZIP_LIMIT) {
            let mut bytes = Vec::with_capacity(len as usize);
            file.read_to_end(&mut bytes)?;
            return self.put_bytes(&bytes);
        }
        self.put_stream(BufReader::new(file))
    }

    pub(crate) fn put_raw_object(&self, hash: &Hash, bytes: &[u8]) -> Result<()> {
        if Hash::of(bytes) != *hash {
            Manifest::parse(bytes)
                .map_err(|err| Error::invalid(format!("object {hash} does not match its hash: {err}")))?;
        }
        self.write_object(hash, bytes)
    }

    fn put_plain_at(&self, hash: &Hash, bytes: &[u8]) -> Result<()> {
        if bytes.len() as u64 > BIG_FILE { self.put_chunked(hash, bytes) } else { self.write_object(hash, bytes) }
    }
}
