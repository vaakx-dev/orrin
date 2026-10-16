use super::PartMerge;
use super::merge::merge_list;
use super::xml;
use crate::keyed::content_key;

fn body_key(element: &str) -> String {
    let tag = xml::tag(element).unwrap_or_default();
    match (tag.as_str(), xml::attribute(element, "w14:paraId")) {
        ("w:p", Some(id)) => format!("paragraph {id}"),
        ("w:sectPr", _) => "section properties".into(),
        _ => content_key(&tag, element),
    }
}

fn merge_document(base: &str, ours: &str, theirs: &str) -> Result<String, String> {
    merge_list([base, ours, theirs], "w:body", body_key)
}

pub fn part_merge(name: &str) -> Option<PartMerge> {
    (name == "word/document.xml").then_some(merge_document as PartMerge)
}
