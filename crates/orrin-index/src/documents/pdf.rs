use std::panic::catch_unwind;

use super::Document;
use super::build::Builder;

pub fn extract(bytes: &[u8]) -> Document {
    let pages = catch_unwind(|| pdf_extract::extract_text_from_mem_by_pages(bytes)).ok().and_then(Result::ok);
    let mut doc = Builder::default();
    for (i, page) in pages.unwrap_or_default().iter().enumerate() {
        let lines: Vec<&str> = page.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
        doc.text(format!("page{}", i + 1), &lines.join("\n"), None);
    }
    doc.finish()
}
