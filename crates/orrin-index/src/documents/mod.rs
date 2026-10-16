mod anchor;
mod build;
mod docx;
mod epub;
mod odt;
mod parts;
mod pdf;
mod pptx;
mod xlsx;
mod xml;

#[cfg(test)]
mod tests;

use orrin_core::model::OutlineItem;
use serde::{Deserialize, Serialize};

pub use anchor::{render, select};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Docx,
    Pptx,
    Xlsx,
    Odt,
    Epub,
    Pdf,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub anchor: String,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub level: Option<u32>,
}

#[derive(Default)]
pub struct Document {
    pub blocks: Vec<Block>,
    pub outline: Vec<OutlineItem>,
}

impl Format {
    pub fn from_extension(ext: &str) -> Option<Self> {
        Some(match ext {
            "docx" => Format::Docx,
            "pptx" => Format::Pptx,
            "xlsx" => Format::Xlsx,
            "odt" => Format::Odt,
            "epub" => Format::Epub,
            "pdf" => Format::Pdf,
            _ => return None,
        })
    }

    pub fn key(self) -> &'static str {
        match self {
            Format::Docx => "docx",
            Format::Pptx => "pptx",
            Format::Xlsx => "xlsx",
            Format::Odt => "odt",
            Format::Epub => "epub",
            Format::Pdf => "pdf",
        }
    }
}

pub fn extract(format: Format, bytes: &[u8]) -> Document {
    match format {
        Format::Docx => docx::extract(bytes),
        Format::Pptx => pptx::extract(bytes),
        Format::Xlsx => xlsx::extract(bytes),
        Format::Odt => odt::extract(bytes),
        Format::Epub => epub::extract(bytes),
        Format::Pdf => pdf::extract(bytes),
    }
}

pub fn lines(blocks: &[Block]) -> impl Iterator<Item = (Option<&str>, &str)> {
    blocks.iter().flat_map(|b| b.text.lines().map(move |line| (Some(b.anchor.as_str()), line)))
}

pub fn plain(blocks: &[Block]) -> String {
    blocks.iter().map(|b| b.text.as_str()).collect::<Vec<_>>().join("\n")
}
