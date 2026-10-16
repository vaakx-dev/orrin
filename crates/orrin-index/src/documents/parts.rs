use std::collections::HashMap;
use std::io::{Cursor, Read};

use zip::ZipArchive;

use super::xml::{Node, attr, name, walk};

pub type Archive<'a> = ZipArchive<Cursor<&'a [u8]>>;

pub fn archive(bytes: &[u8]) -> Option<Archive<'_>> {
    ZipArchive::new(Cursor::new(bytes)).ok()
}

pub fn part(archive: &mut Archive, name: &str) -> Option<String> {
    let mut file = archive.by_name(name).ok()?;
    let mut text = String::new();
    file.read_to_string(&mut text).ok()?;
    Some(text)
}

pub fn relations(archive: &mut Archive, rels: &str) -> HashMap<String, (String, String)> {
    let mut found = HashMap::new();
    let Some(xml) = part(archive, rels) else { return found };
    walk(&xml, |node| {
        if let Node::Open(start) = node
            && name(start) == "Relationship"
            && let (Some(id), Some(target)) = (attr(start, "Id"), attr(start, "Target"))
        {
            found.insert(id, (target, attr(start, "Type").unwrap_or_default()));
        }
    });
    found
}

pub fn resolve(dir: &str, target: &str) -> String {
    if let Some(absolute) = target.strip_prefix('/') {
        return absolute.to_string();
    }
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for piece in target.split('/') {
        match piece {
            "." | "" => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(piece),
        }
    }
    parts.join("/")
}
