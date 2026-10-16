use super::PartMerge;
use super::merge::merge_list;
use super::xml;

fn slide_key(element: &str) -> String {
    format!("slide {}", xml::attribute(element, "id").unwrap_or_default())
}

fn merge_presentation(base: &str, ours: &str, theirs: &str) -> Result<String, String> {
    merge_list([base, ours, theirs], "p:sldIdLst", slide_key)
}

pub fn part_merge(name: &str) -> Option<PartMerge> {
    (name == "ppt/presentation.xml").then_some(merge_presentation as PartMerge)
}
