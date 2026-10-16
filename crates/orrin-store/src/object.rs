use orrin_core::{Error, Hash, Result};
use serde::{Deserialize, Serialize};

pub const MAGIC: &[u8] = b"orrin-object/v1\n";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Manifest {
    Chunks { size: u64, chunks: Vec<Hash> },
    Zip { size: u64, glue: Hash, segments: Vec<Segment> },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Segment {
    Glue(u64),
    Part { hash: Hash, deflate: Option<u32> },
}

pub enum Object {
    Whole(Vec<u8>),
    Composite(Manifest),
}

impl Manifest {
    pub fn size(&self) -> u64 {
        match self {
            Self::Chunks { size, .. } | Self::Zip { size, .. } => *size,
        }
    }

    pub fn parts(&self) -> Vec<Hash> {
        match self {
            Self::Chunks { chunks, .. } => chunks.clone(),
            Self::Zip { glue, segments, .. } => std::iter::once(glue.clone())
                .chain(segments.iter().filter_map(|segment| match segment {
                    Segment::Part { hash, .. } => Some(hash.clone()),
                    Segment::Glue(_) => None,
                }))
                .collect(),
        }
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = MAGIC.to_vec();
        bytes.extend(serde_json::to_vec(self).unwrap_or_default());
        bytes
    }

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let body = bytes.strip_prefix(MAGIC).ok_or_else(|| Error::invalid("not a manifest"))?;
        Ok(serde_json::from_slice(body)?)
    }
}

pub fn decode(hash: &Hash, bytes: Vec<u8>) -> Result<Object> {
    if !bytes.starts_with(MAGIC) || Hash::of(&bytes) == *hash {
        return Ok(Object::Whole(bytes));
    }
    let manifest = Manifest::parse(&bytes).map_err(|err| Error::internal(format!("corrupt object {hash}: {err}")))?;
    Ok(Object::Composite(manifest))
}
