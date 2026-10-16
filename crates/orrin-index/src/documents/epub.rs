use std::collections::HashMap;

use super::Document;
use super::build::{Builder, squash};
use super::parts::{archive, part, resolve};
use super::xml::{Node, attr, name, walk};

const BLOCKS: &[&str] =
    &["p", "div", "section", "article", "li", "blockquote", "pre", "dt", "dd", "td", "th", "tr", "figcaption", "br"];

struct Chapter<'a> {
    number: usize,
    count: u32,
    level: Option<u32>,
    text: String,
    doc: &'a mut Builder,
}

pub fn extract(bytes: &[u8]) -> Document {
    let Some(mut zip) = archive(bytes) else { return Document::default() };
    let Some(root) = part(&mut zip, "META-INF/container.xml").and_then(|x| rootfile(&x)) else {
        return Document::default();
    };
    let Some(package) = part(&mut zip, &root) else { return Document::default() };
    let dir = root.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    let mut doc = Builder::default();
    for (i, href) in spine(&package).iter().enumerate() {
        if let Some(xhtml) = part(&mut zip, &resolve(dir, href)) {
            let mut chapter = Chapter { number: i + 1, count: 0, level: None, text: String::new(), doc: &mut doc };
            chapter.read(&xhtml);
        }
    }
    doc.finish()
}

fn rootfile(container: &str) -> Option<String> {
    let mut found = None;
    walk(container, |node| {
        if let Node::Open(start) = node
            && local(name(start)) == "rootfile"
        {
            found = found.take().or_else(|| attr(start, "full-path"));
        }
    });
    found
}

fn spine(package: &str) -> Vec<String> {
    let mut manifest = HashMap::new();
    let mut order = Vec::new();
    walk(package, |node| {
        if let Node::Open(start) = node {
            match local(name(start)) {
                "item" => {
                    if let (Some(id), Some(href)) = (attr(start, "id"), attr(start, "href")) {
                        manifest.insert(id, href);
                    }
                }
                "itemref" => order.extend(attr(start, "idref")),
                _ => {}
            }
        }
    });
    order.iter().filter_map(|id| manifest.get(id).cloned()).collect()
}

impl Chapter<'_> {
    fn read(&mut self, xhtml: &str) {
        let mut in_body = false;
        walk(xhtml, |node| match node {
            Node::Open(start) => {
                let tag = local(name(start));
                if tag == "body" {
                    in_body = true;
                } else if let Some(level) = heading(tag) {
                    self.flush();
                    self.level = Some(level);
                } else if BLOCKS.contains(&tag) {
                    self.flush();
                }
            }
            Node::Close(tag) => {
                let tag = local(tag);
                if tag == "body" {
                    in_body = false;
                }
                if heading(tag).is_some() || BLOCKS.contains(&tag) || tag == "body" {
                    self.flush();
                }
            }
            Node::Text(chunk) if in_body => self.text.push_str(chunk),
            _ => {}
        });
    }

    fn flush(&mut self) {
        let text = squash(&self.text);
        self.text.clear();
        if !text.is_empty() {
            self.count += 1;
            self.doc.text(format!("ch{}/p{}", self.number, self.count), &text, self.level);
        }
        self.level = None;
    }
}

fn heading(tag: &str) -> Option<u32> {
    let level: u32 = tag.strip_prefix('h')?.parse().ok()?;
    (1..=6).contains(&level).then_some(level)
}

fn local(tag: &str) -> &str {
    tag.rsplit(':').next().unwrap_or(tag)
}
