mod prepare;
mod sheet;

use super::PartMerge;
use super::merge::pick_ignoring;

pub use prepare::prepare;

fn merge_workbook(base: &str, ours: &str, theirs: &str) -> Result<String, String> {
    pick_ignoring(base, ours, theirs, "bookViews").map(str::to_string)
}

pub fn part_merge(name: &str) -> Option<PartMerge> {
    if prepare::is_sheet(name) {
        Some(sheet::merge_sheet)
    } else if name == "xl/workbook.xml" {
        Some(merge_workbook)
    } else {
        None
    }
}
