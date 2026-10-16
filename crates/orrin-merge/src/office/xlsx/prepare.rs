use crate::office::Parts;
use crate::office::xml;

const SHARED_STRINGS: &str = "xl/sharedStrings.xml";
const CALC_CHAIN: &str = "xl/calcChain.xml";

pub fn is_sheet(name: &str) -> bool {
    name.starts_with("xl/worksheets/") && name.ends_with(".xml")
}

fn shared_strings(parts: &Parts) -> Result<Vec<String>, String> {
    let Some((_, data)) = parts.iter().find(|(name, _)| name == SHARED_STRINGS) else {
        return Ok(Vec::new());
    };
    let text = std::str::from_utf8(data).map_err(|_| format!("{SHARED_STRINGS} is not UTF-8"))?;
    let table = xml::split(text, "sst").ok_or_else(|| format!("cannot read {SHARED_STRINGS}"))?;
    Ok(table.children.iter().map(|item| xml::inner(item).to_string()).collect())
}

fn shared_text<'a>(cell: &str, strings: &'a [String]) -> Option<&'a String> {
    let (_, rest) = cell.split_once("<v>")?;
    let (value, _) = rest.split_once("</v>")?;
    strings.get(value.trim().parse::<usize>().ok()?)
}

fn inline(cell: &str, strings: &[String]) -> String {
    let start = &cell[..cell.find('>').map_or(0, |at| at + 1)];
    match (xml::attribute(cell, "t").as_deref(), shared_text(cell, strings)) {
        (Some("s"), Some(text)) => {
            format!("{}<is>{text}</is></c>", start.replace(" t=\"s\"", " t=\"inlineStr\""))
        }
        _ => cell.to_string(),
    }
}

fn prepare_row(row: &str, strings: &[String]) -> String {
    let row = xml::without_attribute(row, "spans");
    match xml::split(&row, "row") {
        Some(cells) => xml::join(cells.head, cells.children.iter().map(|cell| inline(cell, strings)), cells.tail),
        None => row.clone(),
    }
}

fn prepare_sheet(sheet: &str, strings: &[String]) -> String {
    let sheet = xml::without(sheet, "dimension");
    match xml::split(&sheet, "sheetData") {
        Some(rows) => xml::join(rows.head, rows.children.iter().map(|row| prepare_row(row, strings)), rows.tail),
        None => sheet.clone(),
    }
}

fn is_dropped_relationship(element: &str) -> bool {
    let target = xml::attribute(element, "Target").unwrap_or_default();
    target.ends_with("sharedStrings.xml") || target.ends_with("calcChain.xml")
}

fn is_dropped_override(element: &str) -> bool {
    let part = xml::attribute(element, "PartName").unwrap_or_default();
    part == format!("/{SHARED_STRINGS}") || part == format!("/{CALC_CHAIN}")
}

fn full_calc_on_load(workbook: &str) -> String {
    if workbook.contains("fullCalcOnLoad") {
        return workbook.to_string();
    }
    workbook.replacen("<calcPr", "<calcPr fullCalcOnLoad=\"1\"", 1)
}

type Rewrite = fn(&str, &[String]) -> String;

fn rewrite_for(name: &str) -> Option<Rewrite> {
    match name {
        "[Content_Types].xml" => Some(|text, _| xml::retain(text, "Types", |element| !is_dropped_override(element))),
        "xl/_rels/workbook.xml.rels" => {
            Some(|text, _| xml::retain(text, "Relationships", |element| !is_dropped_relationship(element)))
        }
        "xl/workbook.xml" => Some(|text, _| full_calc_on_load(text)),
        name if is_sheet(name) => Some(prepare_sheet),
        _ => None,
    }
}

pub fn prepare(parts: Parts) -> Result<Parts, String> {
    let strings = shared_strings(&parts)?;
    parts
        .into_iter()
        .filter(|(name, _)| name != SHARED_STRINGS && name != CALC_CHAIN)
        .map(|(name, data)| {
            let Some(rewrite) = rewrite_for(&name) else {
                return Ok((name, data));
            };
            let text = std::str::from_utf8(&data).map_err(|_| format!("{name} is not UTF-8"))?;
            let rewritten = rewrite(text, &strings).into_bytes();
            Ok((name, rewritten))
        })
        .collect()
}
