use orrin_core::model::OutlineItem;

use super::xml::{Node, attr, name, walk};
use super::{archive, part};
use crate::outline::item;

pub fn outline(bytes: &[u8]) -> Vec<OutlineItem> {
    let Some(workbook) = archive(bytes).and_then(|mut zip| part(&mut zip, "xl/workbook.xml")) else {
        return Vec::new();
    };
    let mut items = Vec::new();
    walk(&workbook, |node| {
        if let Node::Open(start) = node
            && name(start) == "sheet"
            && let Some(sheet) = attr(start, "name")
        {
            let number = items.len() as u32 + 1;
            items.push(item("sheet", sheet, 0, Some(number)));
        }
    });
    items
}
