mod common;

use std::collections::BTreeMap;

use common::{clean, merge, unzip, zip};
use orrin_core::service::Merged;

const MAIN: &str = "http://schemas.openxmlformats.org/spreadsheetml/2006/main";
const RELS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

fn row_of(reference: &str) -> u32 {
    reference.trim_start_matches(char::is_alphabetic).parse().unwrap()
}

fn sheet(cells: &[(&str, &str)], strings: &[&str], active: &str) -> String {
    let mut rows: BTreeMap<u32, String> = BTreeMap::new();
    for (reference, text) in cells {
        let index = strings.iter().position(|s| s == text).unwrap();
        rows.entry(row_of(reference)).or_default().push_str(&format!(r#"<c r="{reference}" t="s"><v>{index}</v></c>"#));
    }
    let rows: String = rows.iter().map(|(n, cells)| format!(r#"<row r="{n}" spans="1:3">{cells}</row>"#)).collect();
    format!(
        r#"<worksheet xmlns="{MAIN}"><dimension ref="A1:C3"/><sheetViews><sheetView workbookViewId="0"><selection activeCell="{active}"/></sheetView></sheetViews><sheetData>{rows}</sheetData></worksheet>"#
    )
}

fn xlsx(cells: &[(&str, &str)], active: &str) -> Vec<u8> {
    let mut strings: Vec<&str> = cells.iter().map(|(_, text)| *text).collect();
    strings.dedup();
    strings.reverse();
    let items: String = strings.iter().map(|text| format!("<si><t>{text}</t></si>")).collect();
    let types = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Override PartName="/xl/worksheets/sheet1.xml" ContentType="sheet"/><Override PartName="/xl/sharedStrings.xml" ContentType="strings"/><Override PartName="/xl/calcChain.xml" ContentType="chain"/></Types>"#;
    let relationships = format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{RELS}/worksheet" Target="worksheets/sheet1.xml"/><Relationship Id="rId2" Type="{RELS}/sharedStrings" Target="sharedStrings.xml"/><Relationship Id="rId3" Type="{RELS}/calcChain" Target="calcChain.xml"/></Relationships>"#
    );
    zip(&[
        ("[Content_Types].xml".into(), types.into()),
        (
            "xl/workbook.xml".into(),
            format!(
                r#"<workbook xmlns="{MAIN}"><sheets><sheet name="Sheet1" sheetId="1"/></sheets><calcPr calcId="191029"/></workbook>"#
            ),
        ),
        ("xl/_rels/workbook.xml.rels".into(), relationships),
        ("xl/worksheets/sheet1.xml".into(), sheet(cells, &strings, active)),
        ("xl/sharedStrings.xml".into(), format!(r#"<sst xmlns="{MAIN}">{items}</sst>"#)),
        ("xl/calcChain.xml".into(), format!(r#"<calcChain xmlns="{MAIN}"><c r="A1"/></calcChain>"#)),
    ])
}

#[test]
fn separate_cells_merge_cleanly() {
    let base = xlsx(&[("A1", "a"), ("B2", "b")], "A1");
    let ours = xlsx(&[("A1", "x"), ("B2", "b")], "B2");
    let theirs = xlsx(&[("A1", "a"), ("B2", "y"), ("C3", "z")], "C3");
    let merged = unzip(&clean(merge("budget.xlsx", &base, &ours, &theirs)));
    assert!(!merged.contains_key("xl/sharedStrings.xml"));
    assert!(!merged.contains_key("xl/calcChain.xml"));
    assert!(!merged["xl/_rels/workbook.xml.rels"].contains("sharedStrings"));
    assert!(!merged["[Content_Types].xml"].contains("calcChain"));
    assert!(merged["xl/workbook.xml"].contains(r#"<calcPr fullCalcOnLoad="1" calcId="191029"/>"#));
    let rows = concat!(
        r#"<sheetData><row r="1"><c r="A1" t="inlineStr"><is><t>x</t></is></c></row>"#,
        r#"<row r="2"><c r="B2" t="inlineStr"><is><t>y</t></is></c></row>"#,
        r#"<row r="3"><c r="C3" t="inlineStr"><is><t>z</t></is></c></row></sheetData>"#,
    );
    assert!(merged["xl/worksheets/sheet1.xml"].contains(rows), "{}", merged["xl/worksheets/sheet1.xml"]);
}

#[test]
fn same_cell_changed_twice_conflicts() {
    let base = xlsx(&[("A1", "a")], "A1");
    let ours = xlsx(&[("A1", "ours")], "A1");
    let theirs = xlsx(&[("A1", "theirs")], "A1");
    let merged = merge("budget.xlsx", &base, &ours, &theirs);
    assert!(matches!(merged, Merged::Conflict(reason) if reason.contains("cell A1")));
}
