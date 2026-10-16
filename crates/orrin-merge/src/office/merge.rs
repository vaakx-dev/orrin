use super::PartMerge;
use super::xml::{self, Split};
use crate::keyed::{self, Items};
use crate::three::{self, Side, both_changed};

pub fn splits<'a>(xml: [&'a str; 3], container: &str) -> Result<[Split<'a>; 3], String> {
    let [base, ours, theirs] = xml.map(|xml| xml::split(xml, container));
    match (base, ours, theirs) {
        (Some(base), Some(ours), Some(theirs)) => Ok([base, ours, theirs]),
        _ => Err(format!("cannot read <{container}>")),
    }
}

pub fn items(split: &Split, key: impl Fn(&str) -> String) -> Items<String> {
    keyed::unique(split.children.iter().map(|child| (key(child), child.to_string())))
}

pub fn merge_items(
    [base, ours, theirs]: [&Split; 3],
    key: impl Fn(&str) -> String,
    merge_child: impl Fn(&str, &str, &str, &str) -> Result<String, String>,
) -> Result<Items<String>, String> {
    keyed::merge(&items(base, &key), &items(ours, &key), &items(theirs, &key), |key, b, o, t| {
        three::merge(key, b, o, t, |b, o, t| merge_child(key, b, o, t))
    })
}

pub fn pick_text<'a>(base: &'a str, ours: &'a str, theirs: &'a str, what: &str) -> Result<&'a str, String> {
    three::pick(Some(base), Some(ours), Some(theirs)).ok().flatten().ok_or_else(|| both_changed(what))
}

pub fn pick_ignoring<'a>(base: &'a str, ours: &'a str, theirs: &'a str, volatile: &str) -> Result<&'a str, String> {
    let [b, o, t] = [base, ours, theirs].map(|xml| xml::without(xml, volatile));
    match three::winner(Some(&b), Some(&o), Some(&t)) {
        Some(Side::Ours) => Ok(ours),
        Some(Side::Theirs) => Ok(theirs),
        None => Err(both_changed(&format!("content outside <{volatile}>"))),
    }
}

pub fn merge_list(xml: [&str; 3], container: &str, key: fn(&str) -> String) -> Result<String, String> {
    let [base, ours, theirs] = splits(xml, container)?;
    let merged = merge_items([&base, &ours, &theirs], key, |key, _, _, _| Err(both_changed(key)))?;
    let head = pick_text(base.head, ours.head, theirs.head, &format!("<{container}> head"))?;
    let tail = pick_text(base.tail, ours.tail, theirs.tail, &format!("<{container}> tail"))?;
    Ok(xml::join(head, merged.iter().map(|(_, child)| child), tail))
}

fn content_type_key(element: &str) -> String {
    match xml::attribute(element, "Extension") {
        Some(extension) => format!("default {}", extension.to_lowercase()),
        None => format!("override {}", xml::attribute(element, "PartName").unwrap_or_default()),
    }
}

fn relationship_key(element: &str) -> String {
    format!("relationship {}", xml::attribute(element, "Id").unwrap_or_default())
}

fn merge_content_types(base: &str, ours: &str, theirs: &str) -> Result<String, String> {
    merge_list([base, ours, theirs], "Types", content_type_key)
}

fn merge_relationships(base: &str, ours: &str, theirs: &str) -> Result<String, String> {
    merge_list([base, ours, theirs], "Relationships", relationship_key)
}

pub fn shared(name: &str) -> Option<PartMerge> {
    if name == "[Content_Types].xml" {
        Some(merge_content_types)
    } else if name.ends_with(".rels") {
        Some(merge_relationships)
    } else {
        None
    }
}
