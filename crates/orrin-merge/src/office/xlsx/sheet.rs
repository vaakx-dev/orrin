use crate::office::merge::{merge_items, pick_ignoring, pick_text, splits};
use crate::office::xml;
use crate::three::both_changed;

fn row_key(row: &str) -> String {
    format!("row {}", xml::attribute(row, "r").unwrap_or_default())
}

fn cell_key(cell: &str) -> String {
    format!("cell {}", xml::attribute(cell, "r").unwrap_or_default())
}

fn row_number(key: &str) -> u32 {
    key.trim_start_matches(|c: char| !c.is_ascii_digit()).parse().unwrap_or(0)
}

fn column(key: &str) -> u32 {
    let reference = key.trim_start_matches("cell ");
    reference
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .fold(0, |n, c| n * 26 + (c.to_ascii_uppercase() as u32 - 'A' as u32 + 1))
}

fn merge_cell(key: &str, _base: &str, ours: &str, theirs: &str) -> Result<String, String> {
    let [ours, theirs] = [ours, theirs].map(|cell| xml::without(cell, "v"));
    if ours.contains("<f") && ours == theirs {
        return Ok(ours);
    }
    Err(both_changed(key))
}

fn merge_row(key: &str, base: &str, ours: &str, theirs: &str) -> Result<String, String> {
    let [base, ours, theirs] = splits([base, ours, theirs], "row")?;
    let mut cells = merge_items([&base, &ours, &theirs], cell_key, merge_cell)?;
    cells.sort_by_key(|(key, _)| column(key));
    let head = pick_text(base.head, ours.head, theirs.head, key)?;
    let tail = pick_text(base.tail, ours.tail, theirs.tail, key)?;
    Ok(xml::join(head, cells.iter().map(|(_, cell)| cell), tail))
}

pub fn merge_sheet(base: &str, ours: &str, theirs: &str) -> Result<String, String> {
    let [base, ours, theirs] = splits([base, ours, theirs], "sheetData")?;
    let mut rows = merge_items([&base, &ours, &theirs], row_key, merge_row)?;
    rows.sort_by_key(|(key, _)| row_number(key));
    let head = pick_ignoring(base.head, ours.head, theirs.head, "sheetViews")?;
    let tail = pick_text(base.tail, ours.tail, theirs.tail, "content after <sheetData>")?;
    Ok(xml::join(head, rows.iter().map(|(_, row)| row), tail))
}
