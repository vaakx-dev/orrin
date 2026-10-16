use std::collections::HashMap;

use orrin_core::model::OutlineItem;

use super::xml::{Node, attr, name, walk};
use super::{archive, part};
use crate::outline::item;

pub fn outline(bytes: &[u8]) -> Vec<OutlineItem> {
    let Some(mut zip) = archive(bytes) else { return Vec::new() };
    let Some(document) = part(&mut zip, "word/document.xml") else { return Vec::new() };
    let levels = part(&mut zip, "word/styles.xml").map(|s| style_levels(&s)).unwrap_or_default();
    headings(&document, &levels)
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
            "w:outlineLvl" => level = attr(start, "w:val").and_then(|v| v.parse::<u32>().ok()).map(|v| v + 1),
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

fn headings(xml: &str, levels: &HashMap<String, u32>) -> Vec<OutlineItem> {
    let mut items = Vec::new();
    let mut level: Option<u32> = None;
    let mut in_text = false;
    let mut text = String::new();
    walk(xml, |node| match node {
        Node::Open(start) => match name(start) {
            "w:p" => {
                level = None;
                text.clear();
            }
            "w:pStyle" => {
                let style = attr(start, "w:val").unwrap_or_default();
                level = level.or_else(|| levels.get(&style).copied().or_else(|| level_from_name(&style)));
            }
            "w:outlineLvl" => level = attr(start, "w:val").and_then(|v| v.parse::<u32>().ok()).map(|v| v + 1),
            "w:t" => in_text = true,
            _ => {}
        },
        Node::Close("w:t") => in_text = false,
        Node::Text(chunk) if in_text => text.push_str(chunk),
        Node::Close("w:p") => {
            let title = text.trim();
            if let Some(level) = level.filter(|_| !title.is_empty()) {
                let kind = if level == 0 { "title" } else { "heading" };
                items.push(item(kind, title, level.saturating_sub(1), None));
            }
        }
        _ => {}
    });
    items
}

fn level_from_name(name: &str) -> Option<u32> {
    let lower = name.to_ascii_lowercase();
    if lower == "title" {
        return Some(0);
    }
    let level: u32 = lower.strip_prefix("heading")?.trim().parse().ok()?;
    (1..=9).contains(&level).then_some(level)
}
