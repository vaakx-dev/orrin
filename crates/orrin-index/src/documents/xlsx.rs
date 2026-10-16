use super::Document;
use super::build::Builder;
use super::parts::{archive, part, relations, resolve};
use super::xml::{Node, attr, name, walk};

#[derive(Default)]
struct Cell {
    reference: String,
    kind: String,
    value: String,
}

pub fn extract(bytes: &[u8]) -> Document {
    let Some(mut zip) = archive(bytes) else { return Document::default() };
    let Some(workbook) = part(&mut zip, "xl/workbook.xml") else { return Document::default() };
    let targets = relations(&mut zip, "xl/_rels/workbook.xml.rels");
    let shared = part(&mut zip, "xl/sharedStrings.xml").map(|x| shared_strings(&x)).unwrap_or_default();
    let mut sheets = Vec::new();
    walk(&workbook, |node| {
        if let Node::Open(start) = node
            && name(start) == "sheet"
            && let Some(sheet) = attr(start, "name")
        {
            let number = sheets.len() + 1;
            let path = attr(start, "r:id")
                .and_then(|id| targets.get(&id))
                .map(|(target, _)| resolve("xl", target))
                .unwrap_or_else(|| format!("xl/worksheets/sheet{number}.xml"));
            sheets.push((sheet, path));
        }
    });
    let mut doc = Builder::default();
    for (i, (sheet, path)) in sheets.iter().enumerate() {
        doc.item("sheet", sheet, 0, Some(i as u32 + 1), sheet);
        for cell in part(&mut zip, path).map(|x| cells(&x)).unwrap_or_default() {
            let value = match cell.kind.as_str() {
                "s" => cell.value.trim().parse::<usize>().ok().and_then(|i| shared.get(i)).cloned().unwrap_or_default(),
                _ => cell.value,
            };
            doc.text(format!("{sheet}!{}", cell.reference), &value, None);
        }
    }
    doc.finish()
}

fn shared_strings(xml: &str) -> Vec<String> {
    let mut strings = Vec::new();
    let mut in_text = false;
    let mut phonetic = false;
    walk(xml, |node| match node {
        Node::Open(start) => match name(start) {
            "si" => strings.push(String::new()),
            "rPh" => phonetic = true,
            "t" => in_text = !phonetic,
            _ => {}
        },
        Node::Close("rPh") => phonetic = false,
        Node::Close("t") => in_text = false,
        Node::Text(chunk) if in_text => {
            if let Some(last) = strings.last_mut() {
                last.push_str(chunk);
            }
        }
        _ => {}
    });
    strings
}

fn cells(xml: &str) -> Vec<Cell> {
    let mut cells = Vec::new();
    let mut current: Option<Cell> = None;
    let mut capture = false;
    walk(xml, |node| match node {
        Node::Open(start) => match name(start) {
            "c" => {
                current = Some(Cell {
                    reference: attr(start, "r").unwrap_or_default(),
                    kind: attr(start, "t").unwrap_or_default(),
                    value: String::new(),
                })
            }
            "v" | "t" => capture = current.is_some(),
            _ => {}
        },
        Node::Close("v" | "t") => capture = false,
        Node::Text(chunk) if capture => {
            if let Some(cell) = current.as_mut() {
                cell.value.push_str(chunk);
            }
        }
        Node::Close("c") => cells.extend(current.take().filter(|c| !c.reference.is_empty())),
        _ => {}
    });
    cells
}
