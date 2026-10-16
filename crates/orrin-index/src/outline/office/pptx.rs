use std::collections::HashMap;

use orrin_core::model::OutlineItem;

use super::xml::{Node, attr, name, walk};
use super::{Archive, archive, part};
use crate::outline::item;

pub fn outline(bytes: &[u8]) -> Vec<OutlineItem> {
    let Some(mut zip) = archive(bytes) else { return Vec::new() };
    slide_order(&mut zip)
        .iter()
        .enumerate()
        .map(|(i, slide)| {
            let number = i as u32 + 1;
            let title = part(&mut zip, slide).and_then(|xml| title(&xml));
            item("slide", title.unwrap_or_else(|| format!("Slide {number}")), 0, Some(number))
        })
        .collect()
}

fn slide_order(zip: &mut Archive) -> Vec<String> {
    let listed = listed_slides(zip);
    if !listed.is_empty() {
        return listed;
    }
    let mut numbered: Vec<(u32, String)> = zip
        .file_names()
        .filter_map(|n| {
            let n = n.ok()?;
            let number = n.strip_prefix("ppt/slides/slide")?.strip_suffix(".xml")?.parse().ok()?;
            Some((number, n.to_string()))
        })
        .collect();
    numbered.sort();
    numbered.into_iter().map(|(_, n)| n).collect()
}

fn listed_slides(zip: &mut Archive) -> Vec<String> {
    let (Some(presentation), Some(rels)) =
        (part(zip, "ppt/presentation.xml"), part(zip, "ppt/_rels/presentation.xml.rels"))
    else {
        return Vec::new();
    };
    let mut targets = HashMap::new();
    walk(&rels, |node| {
        if let Node::Open(start) = node
            && name(start) == "Relationship"
            && let (Some(id), Some(target)) = (attr(start, "Id"), attr(start, "Target"))
        {
            targets.insert(id, target);
        }
    });
    let mut slides = Vec::new();
    walk(&presentation, |node| {
        if let Node::Open(start) = node
            && name(start) == "p:sldId"
            && let Some(target) = attr(start, "r:id").and_then(|id| targets.get(&id))
        {
            slides.push(resolve(target));
        }
    });
    slides
}

fn resolve(target: &str) -> String {
    match target.strip_prefix('/') {
        Some(absolute) => absolute.to_string(),
        None => format!("ppt/{target}"),
    }
}

fn title(xml: &str) -> Option<String> {
    let mut found = None;
    let mut is_title = false;
    let mut in_text = false;
    let mut text = String::new();
    walk(xml, |node| {
        if found.is_some() {
            return;
        }
        match node {
            Node::Open(start) => match name(start) {
                "p:sp" => {
                    is_title = false;
                    text.clear();
                }
                "p:ph" => is_title = matches!(attr(start, "type").as_deref(), Some("title" | "ctrTitle")),
                "a:p" if !text.is_empty() => text.push(' '),
                "a:t" => in_text = true,
                _ => {}
            },
            Node::Close("a:t") => in_text = false,
            Node::Text(chunk) if in_text => text.push_str(chunk),
            Node::Close("p:sp") if is_title && !text.trim().is_empty() => found = Some(text.trim().to_string()),
            _ => {}
        }
    });
    found
}
