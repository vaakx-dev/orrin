mod kind;
pub mod trigrams;

use orrin_core::model::OutlineItem;
use serde::{Deserialize, Serialize};

pub use kind::Kind;

use crate::{outline, symbols};

pub const MAX_INDEXED: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Record {
    #[serde(skip)]
    pub trigrams: Vec<u32>,
    pub text: bool,
    pub symbols: Vec<Sym>,
    pub outline: Vec<OutlineItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sym {
    pub name: String,
    pub kind: String,
    pub line: u32,
    pub container: Option<String>,
    pub depth: u32,
}

pub fn analyse(kind: Kind, bytes: &[u8]) -> Record {
    let text = is_text(bytes);
    let mut record = Record { text, ..Record::default() };
    if text {
        record.trigrams = trigrams::of_content(bytes);
    }
    let source = || String::from_utf8_lossy(bytes);
    match kind {
        Kind::Code(lang) if text => {
            record.symbols = symbols::extract(lang, &source());
            record.outline = symbols::outline(&record.symbols);
        }
        Kind::Markdown if text => record.outline = outline::markdown(&source()),
        Kind::Scene if text => record.outline = outline::godot(&source()),
        Kind::Docx => record.outline = outline::docx(bytes),
        Kind::Pptx => record.outline = outline::pptx(bytes),
        Kind::Xlsx => record.outline = outline::xlsx(bytes),
        _ => {}
    }
    record
}

fn is_text(bytes: &[u8]) -> bool {
    !bytes[..bytes.len().min(8192)].contains(&0)
}
