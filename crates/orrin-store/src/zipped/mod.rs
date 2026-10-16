mod codec;
mod plan;
mod rebuild;

use orrin_core::{Hash, Result};

use crate::disk::DiskStore;
use crate::object::{Manifest, Segment};
use plan::Planned;

pub use rebuild::rebuild;

pub fn looks_zipped(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
}

impl DiskStore {
    pub(crate) fn put_zipped(&self, hash: &Hash, bytes: &[u8]) -> Result<bool> {
        let Some(plan) = plan::plan(bytes) else {
            return Ok(false);
        };
        let glue = self.put_plain(&plan.glue)?;
        let segments = plan
            .segments
            .into_iter()
            .map(|planned| match planned {
                Planned::Glue(len) => Ok(Segment::Glue(len)),
                Planned::Part { bytes, deflate } => Ok(Segment::Part { hash: self.put_plain(&bytes)?, deflate }),
            })
            .collect::<Result<Vec<_>>>()?;
        let mut hasher = blake3::Hasher::new();
        if rebuild(self, &glue, &segments, &mut hasher).is_err() || Hash::from_blake3(hasher.finalize()) != *hash {
            return Ok(false);
        }
        self.write_object(hash, &Manifest::Zip { size: bytes.len() as u64, glue, segments }.encode())?;
        Ok(true)
    }
}
