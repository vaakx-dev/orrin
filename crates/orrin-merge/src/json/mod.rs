mod merge;
mod render;
mod value;

use orrin_core::service::{Merged, Merger};

use crate::{extension, text};
use value::Json;

pub struct JsonMerger;

impl Merger for JsonMerger {
    fn name(&self) -> &'static str {
        "json"
    }

    fn handles(&self, path: &str) -> bool {
        extension(path) == "json"
    }

    fn merge(&self, _path: &str, base: &[u8], ours: &[u8], theirs: &[u8]) -> Merged {
        let parse = |bytes: &[u8]| serde_json::from_slice::<Json>(bytes).ok();
        let (Some(b), Some(o), Some(t)) = (parse(base), parse(ours), parse(theirs)) else {
            return text::merge_bytes(base, ours, theirs);
        };
        let merged =
            merge::merge("$", &b, &o, &t).and_then(|json| render::render(&json, &String::from_utf8_lossy(ours)));
        match merged {
            Ok(bytes) => Merged::Clean(bytes),
            Err(reason) => Merged::Conflict(reason),
        }
    }
}
