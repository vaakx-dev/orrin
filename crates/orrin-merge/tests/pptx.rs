mod common;

use common::{clean, merge, unzip, zip};
use orrin_core::service::Merged;

fn pptx(slides: &[&str]) -> Vec<u8> {
    let numbered = || slides.iter().enumerate().map(|(index, title)| (index + 1, title));
    let overrides: String = numbered()
        .map(|(n, _)| format!(r#"<Override PartName="/ppt/slides/slide{n}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#))
        .collect();
    let ids: String = numbered().map(|(n, _)| format!(r#"<p:sldId id="{}" r:id="rId{n}"/>"#, 255 + n)).collect();
    let relationships: String = numbered()
        .map(|(n, _)| format!(r#"<Relationship Id="rId{n}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{n}.xml"/>"#))
        .collect();
    let mut parts = vec![
        (
            "[Content_Types].xml".to_string(),
            format!(
                r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/>{overrides}</Types>"#
            ),
        ),
        (
            "ppt/presentation.xml".to_string(),
            format!(
                r#"<p:presentation xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx="9144000" cy="6858000"/></p:presentation>"#
            ),
        ),
        (
            "ppt/_rels/presentation.xml.rels".to_string(),
            format!(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{relationships}</Relationships>"#
            ),
        ),
    ];
    parts.extend(numbered().map(|(n, title)| {
        (format!("ppt/slides/slide{n}.xml"), format!("<p:sld><p:cSld><a:t>{title}</a:t></p:cSld></p:sld>"))
    }));
    zip(&parts)
}

#[test]
fn edited_and_added_slides_merge_cleanly() {
    let base = pptx(&["Intro"]);
    let ours = pptx(&["Welcome"]);
    let theirs = pptx(&["Intro", "Plan"]);
    let merged = unzip(&clean(merge("deck.pptx", &base, &ours, &theirs)));
    assert_eq!(merged, unzip(&pptx(&["Welcome", "Plan"])));
}

#[test]
fn same_slide_changed_twice_conflicts() {
    let merged = merge("deck.pptx", &pptx(&["Intro"]), &pptx(&["Ours"]), &pptx(&["Theirs"]));
    assert!(matches!(merged, Merged::Conflict(reason) if reason.contains("slide1.xml")));
}
