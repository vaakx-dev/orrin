use std::io::Cursor;

use zip::{CompressionMethod, ZipArchive};

use super::codec::{deflate, inflate, level_hint};

const MARKERS: [&str; 2] = ["[Content_Types].xml", "mimetype"];

pub struct Plan {
    pub glue: Vec<u8>,
    pub segments: Vec<Planned>,
}

pub enum Planned {
    Glue(u64),
    Part { bytes: Vec<u8>, deflate: Option<u32> },
}

struct Span {
    start: usize,
    end: usize,
    deflated: bool,
    flags: u16,
}

pub fn plan(bytes: &[u8]) -> Option<Plan> {
    let mut spans = spans(bytes)?;
    spans.sort_by_key(|span| span.start);
    let mut plan = Plan { glue: Vec::new(), segments: Vec::new() };
    let mut at = 0;
    for span in spans {
        if span.start < at || span.end > bytes.len() {
            return None;
        }
        plan.push_glue(&bytes[at..span.start]);
        if span.end > span.start {
            plan.segments.push(part(&bytes[span.start..span.end], &span));
        }
        at = span.end;
    }
    plan.push_glue(&bytes[at..]);
    Some(plan)
}

impl Plan {
    fn push_glue(&mut self, bytes: &[u8]) {
        if !bytes.is_empty() {
            self.glue.extend_from_slice(bytes);
            self.segments.push(Planned::Glue(bytes.len() as u64));
        }
    }
}

fn spans(bytes: &[u8]) -> Option<Vec<Span>> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).ok()?;
    if !MARKERS.iter().any(|name| archive.index_for_name(name).is_some()) {
        return None;
    }
    (0..archive.len())
        .map(|index| {
            let file = archive.by_index_raw(index).ok()?;
            let start = usize::try_from(file.data_start()?).ok()?;
            let end = start.checked_add(usize::try_from(file.compressed_size()).ok()?)?;
            let flags_at = usize::try_from(file.header_start()).ok()? + 6;
            let flags = u16::from_le_bytes(bytes.get(flags_at..flags_at + 2)?.try_into().ok()?);
            Some(Span { start, end, deflated: file.compression() == CompressionMethod::Deflated, flags })
        })
        .collect()
}

fn part(raw: &[u8], span: &Span) -> Planned {
    let level = level_hint(span.flags);
    let inflated = span.deflated.then(|| inflate(raw).ok()).flatten();
    match inflated {
        Some(bytes) if deflate(&bytes, level).is_ok_and(|again| again == raw) => {
            Planned::Part { bytes, deflate: Some(level) }
        }
        _ => Planned::Part { bytes: raw.to_vec(), deflate: None },
    }
}
