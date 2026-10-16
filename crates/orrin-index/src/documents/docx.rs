use std::collections::HashMap;

use super::Document;
use super::build::Builder;
use super::parts::{archive, part};
use super::xml::{Node, attr, name, walk};

#[derive(Default)]
struct Paragraph {
    number: u32,
    level: Option<u32>,
    text: String,
}

pub fn extract(bytes: &[u8]) -> Document {
    let Some(mut zip) = archive(bytes) else { return Document::default() };
    let Some(document) = part(&mut zip, "word/document.xml") else { return Document::default() };
    let levels = part(&mut zip, "word/styles.xml").map(|s| style_levels(&s)).unwrap_or_default();
    paragraphs(&document, &levels)
}

fn paragraphs(xml: &str, levels: &HashMap<String, u32>) -> Document {
    let mut doc = Builder::default();
    let mut open: Vec<Paragraph> = Vec::new();
    let mut count = 0;
    let mut in_run = false;
    let mut in_text = false;
    walk(xml, |node| {
        let current = open.last_mut();
        match node {
            Node::Open(start) => match (name(start), current) {
                ("w:p", _) => {
                    count += 1;
                    open.push(Paragraph { number: count, ..Paragraph::default() });
                }
                ("w:pStyle", Some(p)) => {
                    let style = attr(start, "w:val").unwrap_or_default();
                    p.level = p.level.or_else(|| levels.get(&style).copied().or_else(|| level_from_name(&style)));
                }
                ("w:outlineLvl", Some(p)) => p.level = outline_level(start),
                ("w:r", _) => in_run = true,
                ("w:t", _) => in_text = true,
                ("w:tab", Some(p)) if in_run => p.text.push('\t'),
                ("w:br" | "w:cr", Some(p)) if in_run => p.text.push('\n'),
                _ => {}
            },
            Node::Close("w:r") => in_run = false,
            Node::Close("w:t") => in_text = false,
            Node::Text(chunk) if in_text => {
                if let Some(p) = current {
                    p.text.push_str(chunk);
                }
            }
            Node::Close("w:p") => {
                if let Some(p) = open.pop() {
                    doc.text(format!("p{}", p.number), &p.text, p.level);
                }
            }
            _ => {}
        }
    });
    doc.finish()
}

fn style_levels(xml: &str) -> HashMap<String, u32> {
    let mut levels = HashMap::new();
    let mut id = None;
    let mut level = None;
    walk(xml, |node| match node {
        Node::Open(start) => match name(start) {
            "w:style" => {
                id = attr(start, "w:styleId");
                level = None;
            }
            "w:name" => level = level.or_else(|| attr(start, "w:val").and_then(|n| level_from_name(&n))),
            "w:outlineLvl" => level = outline_level(start),
            _ => {}
        },
        Node::Close("w:style") => {
            if let (Some(id), Some(level)) = (id.take(), level.take()) {
                levels.insert(id, level);
            }
        }
        _ => {}
    });
    levels
}

fn outline_level(start: &quick_xml::events::BytesStart) -> Option<u32> {
    attr(start, "w:val").and_then(|v| v.parse::<u32>().ok()).filter(|v| *v < 9).map(|v| v + 1)
}

fn level_from_name(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    if lower == "title" {
        return Some(0);
    }
    let level: u32 = lower.strip_prefix("heading")?.trim().parse().ok()?;
    (1..=9).contains(&level).then_some(level)
}
