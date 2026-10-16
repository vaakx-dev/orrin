use orrin_core::{Error, Hash, Result};
use serde::{Deserialize, Serialize};

pub const MAGIC: &[u8] = b"orrin-object/v1\n";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Manifest {
    Chunks { size: u64, chunks: Vec<Piece> },
    Zip { size: u64, glue: Hash, segments: Vec<Segment> },
    Slice { file: Hash, offset: u64, len: u64 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Piece {
    pub hash: Hash,
    pub len: u64,
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
            Self::Slice { len, .. } => *len,
        }
    }

    pub fn parts(&self) -> Vec<Hash> {
        match self {
            Self::Chunks { chunks, .. } => chunks.iter().map(|piece| piece.hash.clone()).collect(),
            Self::Zip { glue, segments, .. } => std::iter::once(glue.clone())
                .chain(segments.iter().filter_map(|segment| match segment {
                    Segment::Part { hash, .. } => Some(hash.clone()),
                    Segment::Glue(_) => None,
                }))
                .collect(),
            Self::Slice { .. } => Vec::new(),
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

pub fn offsets(pieces: &[Piece]) -> impl Iterator<Item = (&Piece, u64)> {
    pieces.iter().scan(0u64, |at, piece| {
        let start = *at;
        *at += piece.len;
        Some((piece, start))
    })
}

pub fn decode(hash: &Hash, bytes: Vec<u8>) -> Result<Object> {
    if !bytes.starts_with(MAGIC) || Hash::of(&bytes) == *hash {
        return Ok(Object::Whole(bytes));
    }
    let manifest = Manifest::parse(&bytes).map_err(|err| Error::internal(format!("corrupt object {hash}: {err}")))?;
    Ok(Object::Composite(manifest))
}
