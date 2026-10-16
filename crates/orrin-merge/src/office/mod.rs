mod docx;
mod merge;
mod package;
mod pptx;
mod xlsx;
mod xml;

use orrin_core::service::{Merged, Merger};

use crate::keyed::{self, Items};
use crate::{extension, three};

pub type Parts = Items<Vec<u8>>;

pub type PartMerge = fn(&str, &str, &str) -> Result<String, String>;

pub struct Office {
    name: &'static str,
    extensions: &'static [&'static str],
    volatile: &'static [&'static str],
    part_merge: fn(&str) -> Option<PartMerge>,
    prepare: fn(Parts) -> Result<Parts, String>,
}

impl Office {
    fn is_volatile(&self, name: &str) -> bool {
        name.starts_with("docProps/") || self.volatile.contains(&name)
    }

    fn merge_part(&self, name: &str, base: &[u8], ours: &[u8], theirs: &[u8]) -> Result<Vec<u8>, String> {
        if self.is_volatile(name) {
            return Ok(ours.to_vec());
        }
        let merge = merge::shared(name).or_else(|| (self.part_merge)(name)).ok_or_else(|| three::both_changed(name))?;
        let text = |bytes| std::str::from_utf8(bytes).map_err(|_| format!("{name} is not UTF-8"));
        merge(text(base)?, text(ours)?, text(theirs)?)
            .map(String::into_bytes)
            .map_err(|reason| format!("{name}: {reason}"))
    }

    fn merge_package(&self, base: &[u8], ours: &[u8], theirs: &[u8]) -> Result<Vec<u8>, String> {
        let open = |bytes| package::read(bytes).and_then(self.prepare);
        let parts = keyed::merge(&open(base)?, &open(ours)?, &open(theirs)?, |name, b, o, t| {
            three::merge(name, b, o, t, |b, o, t| self.merge_part(name, b, o, t))
        })?;
        package::write(&parts)
    }
}

impl Merger for Office {
    fn name(&self) -> &'static str {
        self.name
    }

    fn handles(&self, path: &str) -> bool {
        self.extensions.contains(&extension(path).as_str())
    }

    fn merge(&self, _path: &str, base: &[u8], ours: &[u8], theirs: &[u8]) -> Merged {
        match self.merge_package(base, ours, theirs) {
            Ok(bytes) => Merged::Clean(bytes),
            Err(reason) => Merged::Conflict(reason),
        }
    }
}

pub fn docx() -> Office {
    Office {
        name: "docx",
        extensions: &["docx", "docm"],
        volatile: &["word/settings.xml"],
        part_merge: docx::part_merge,
        prepare: Ok,
    }
}

pub fn pptx() -> Office {
    Office {
        name: "pptx",
        extensions: &["pptx", "pptm"],
        volatile: &["ppt/viewProps.xml"],
        part_merge: pptx::part_merge,
        prepare: Ok,
    }
}

pub fn xlsx() -> Office {
    Office {
        name: "xlsx",
        extensions: &["xlsx", "xlsm"],
        volatile: &[],
        part_merge: xlsx::part_merge,
        prepare: xlsx::prepare,
    }
}
