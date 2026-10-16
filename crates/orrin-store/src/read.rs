use std::fs::{self, File};
use std::io::{self, Read, Write};

use orrin_core::{Error, Hash, Result};

use crate::disk::DiskStore;
use crate::object::{MAGIC, Manifest, Object, decode};
use crate::zipped;

impl DiskStore {
    pub(crate) fn raw_bytes(&self, hash: &Hash) -> Result<Vec<u8>> {
        fs::read(self.object_path(hash)).map_err(|err| not_found(hash, err))
    }

    pub(crate) fn load(&self, hash: &Hash) -> Result<Object> {
        decode(hash, self.raw_bytes(hash)?)
    }

    pub(crate) fn manifest(&self, hash: &Hash) -> Result<Option<Manifest>> {
        if !self.has_manifest_head(hash)? {
            return Ok(None);
        }
        match self.load(hash)? {
            Object::Composite(manifest) => Ok(Some(manifest)),
            Object::Whole(_) => Ok(None),
        }
    }

    pub(crate) fn object_size(&self, hash: &Hash) -> Result<u64> {
        match self.manifest(hash)? {
            Some(manifest) => Ok(manifest.size()),
            None => Ok(fs::metadata(self.object_path(hash))?.len()),
        }
    }

    pub(crate) fn write_to(&self, hash: &Hash, out: &mut dyn Write) -> Result<()> {
        match self.load(hash)? {
            Object::Whole(bytes) => out.write_all(&bytes)?,
            Object::Composite(Manifest::Chunks { chunks, .. }) => {
                for chunk in &chunks {
                    self.write_to(chunk, out)?;
                }
            }
            Object::Composite(Manifest::Zip { glue, segments, .. }) => zipped::rebuild(self, &glue, &segments, out)?,
        }
        Ok(())
    }

    pub(crate) fn read_all(&self, hash: &Hash) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        self.write_to(hash, &mut bytes)?;
        Ok(bytes)
    }

    fn has_manifest_head(&self, hash: &Hash) -> Result<bool> {
        let file = File::open(self.object_path(hash)).map_err(|err| not_found(hash, err))?;
        let mut head = Vec::with_capacity(MAGIC.len());
        file.take(MAGIC.len() as u64).read_to_end(&mut head)?;
        Ok(head == MAGIC)
    }
}

fn not_found(hash: &Hash, err: io::Error) -> Error {
    match err.kind() {
        io::ErrorKind::NotFound => Error::not_found(format!("object {hash} is not in the store")),
        _ => err.into(),
    }
}
