use std::io::Write;

use orrin_core::{Error, Hash, Result};

use super::codec::deflate;
use crate::disk::DiskStore;
use crate::object::Segment;

pub fn rebuild(store: &DiskStore, glue: &Hash, segments: &[Segment], out: &mut dyn Write) -> Result<()> {
    let glue_bytes = store.read_all(glue)?;
    let mut at = 0usize;
    for segment in segments {
        match segment {
            Segment::Glue(len) => {
                let end = at.saturating_add(*len as usize);
                let bytes = glue_bytes.get(at..end).ok_or_else(|| Error::internal(format!("glue {glue} is short")))?;
                out.write_all(bytes)?;
                at = end;
            }
            Segment::Part { hash, deflate: None } => store.write_to(hash, out)?,
            Segment::Part { hash, deflate: Some(level) } => out.write_all(&deflate(&store.read_all(hash)?, *level)?)?,
        }
    }
    Ok(())
}
