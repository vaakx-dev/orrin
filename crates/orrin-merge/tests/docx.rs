mod common;

use common::{clean, merge, unzip, zip};
use orrin_core::service::Merged;

const TYPES: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#;

fn docx(paragraphs: &[(&str, &str)], author: &str) -> Vec<u8> {
    let body: String = paragraphs
        .iter()
        .map(|(id, text)| format!(r#"<w:p w14:paraId="{id}"><w:r><w:t>{text}</w:t></w:r></w:p>"#))
        .collect();
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:w14="http://schemas.microsoft.com/office/word/2010/wordml"><w:body>{body}<w:sectPr/></w:body></w:document>"#
    );
    let core = format!("<cp:coreProperties><dc:creator>{author}</dc:creator></cp:coreProperties>");
    zip(&[
        ("[Content_Types].xml".into(), TYPES.into()),
        ("word/document.xml".into(), document),
        ("docProps/core.xml".into(), core),
    ])
}

#[test]
fn separate_paragraphs_merge_cleanly() {
    let base = docx(&[("1", "one"), ("2", "two")], "base");
    let ours = docx(&[("1", "ONE"), ("2", "two")], "ours");
    let theirs = docx(&[("1", "one"), ("2", "TWO"), ("3", "three")], "theirs");
    let merged = unzip(&clean(merge("report.docx", &base, &ours, &theirs)));
    let expected = unzip(&docx(&[("1", "ONE"), ("2", "TWO"), ("3", "three")], "ours"));
    assert_eq!(merged, expected);
}

#[test]
fn same_paragraph_changed_twice_conflicts() {
    let base = docx(&[("1", "one")], "base");
    let ours = docx(&[("1", "ours")], "base");
    let theirs = docx(&[("1", "theirs")], "base");
    let merged = merge("report.docx", &base, &ours, &theirs);
    assert!(matches!(merged, Merged::Conflict(reason) if reason.contains("paragraph 1")));
}
