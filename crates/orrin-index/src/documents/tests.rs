use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;

use super::*;

fn zipped(parts: &[(&str, &str)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, body) in parts {
        writer.start_file(*name, SimpleFileOptions::default()).unwrap();
        writer.write_all(body.as_bytes()).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

fn anchored(doc: &Document) -> Vec<(&str, &str)> {
    doc.blocks.iter().map(|b| (b.anchor.as_str(), b.text.as_str())).collect()
}

fn outline(doc: &Document) -> Vec<(&str, u32, &str)> {
    doc.outline.iter().map(|i| (i.name.as_str(), i.depth, i.anchor.as_deref().unwrap_or(""))).collect()
}

fn pdf(pages: &[&str]) -> Vec<u8> {
    let count = pages.len();
    let kids: Vec<String> = (0..count).map(|i| format!("{} 0 R", 4 + i * 2)).collect();
    let mut objects = vec![
        "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
        format!("<< /Type /Pages /Kids [{}] /Count {count} >>", kids.join(" ")),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string(),
    ];
    for (i, text) in pages.iter().enumerate() {
        let content = format!("BT /F1 12 Tf 20 100 Td ({text}) Tj ET");
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 300 200] /Contents {} 0 R /Resources << /Font << /F1 3 0 R >> >> >>",
            5 + i * 2
        ));
        objects.push(format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()));
    }
    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend(format!("{} 0 obj\n{body}\nendobj\n", i + 1).bytes());
    }
    let xref = out.len();
    out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
    for offset in offsets {
        out.extend(format!("{offset:010} 00000 n \n").bytes());
    }
    out.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).bytes());
    out
}

#[test]
fn docx_paragraphs_and_sections() {
    let styles = r#"<w:styles><w:style w:styleId="Berschrift2"><w:name w:val="heading 2"/></w:style></w:styles>"#;
    let document = r#"<w:document><w:body>
        <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Intro &amp; goals</w:t></w:r></w:p>
        <w:p><w:r><w:t>Body</w:t><w:tab/><w:t>text</w:t></w:r></w:p>
        <w:p><w:pPr><w:pStyle w:val="Berschrift2"/></w:pPr><w:r><w:t>Detail</w:t></w:r></w:p>
        <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Next</w:t></w:r></w:p>
    </w:body></w:document>"#;
    let doc = extract(Format::Docx, &zipped(&[("word/document.xml", document), ("word/styles.xml", styles)]));
    assert_eq!(anchored(&doc), [("p1", "Intro & goals"), ("p2", "Body\ttext"), ("p3", "Detail"), ("p4", "Next")]);
    assert_eq!(outline(&doc), [("Intro & goals", 0, "p1"), ("Detail", 1, "p3"), ("Next", 0, "p4")]);
    assert_eq!(render(&select(&doc.blocks, "p1")), "[p1] Intro & goals\n[p2] Body\ttext\n[p3] Detail");
    assert_eq!(render(&select(&doc.blocks, "p2")), "Body\ttext");
}

#[test]
fn pptx_slides_and_notes() {
    let slide = |title: &str, body: &str| {
        format!(
            r#"<p:sld><p:sp><p:nvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><a:p><a:r><a:t>{title}</a:t></a:r></a:p></p:sp><p:sp><a:p><a:r><a:t>{body}</a:t></a:r></a:p></p:sp></p:sld>"#
        )
    };
    let notes = r#"<p:notes><p:sp><p:nvPr><p:ph type="sldNum"/></p:nvPr><a:p><a:r><a:t>2</a:t></a:r></a:p></p:sp><p:sp><p:nvPr><p:ph type="body"/></p:nvPr><a:p><a:r><a:t>Say hello</a:t></a:r></a:p></p:sp></p:notes>"#;
    let rels = r#"<Relationships><Relationship Id="rId1" Type="http://x/notesSlide" Target="../notesSlides/notesSlide1.xml"/></Relationships>"#;
    let deck = zipped(&[
        ("ppt/slides/slide2.xml", &slide("Second", "Roadmap")),
        ("ppt/slides/slide1.xml", &slide("First", "Welcome")),
        ("ppt/slides/_rels/slide2.xml.rels", rels),
        ("ppt/notesSlides/notesSlide1.xml", notes),
    ]);
    let doc = extract(Format::Pptx, &deck);
    assert_eq!(outline(&doc), [("First", 0, "slide1"), ("Second", 0, "slide2")]);
    assert_eq!(
        anchored(&doc),
        [("slide1", "First\nWelcome"), ("slide2", "Second\nRoadmap"), ("slide2/notes", "Say hello")]
    );
    assert_eq!(select(&doc.blocks, "slide2").len(), 2);
}

#[test]
fn xlsx_cells_with_sheet_anchors() {
    let book = zipped(&[
        (
            "xl/workbook.xml",
            r#"<workbook><sheets><sheet name="Data" r:id="rId1"/><sheet name="Notes" r:id="rId2"/></sheets></workbook>"#,
        ),
        (
            "xl/_rels/workbook.xml.rels",
            r#"<Relationships><Relationship Id="rId1" Target="worksheets/a.xml"/><Relationship Id="rId2" Target="/xl/worksheets/b.xml"/></Relationships>"#,
        ),
        ("xl/sharedStrings.xml", r#"<sst><si><t>Revenue</t></si><si><r><t>Q</t></r><r><t>1</t></r></si></sst>"#),
        (
            "xl/worksheets/a.xml",
            r#"<worksheet><sheetData><row r="1"><c r="A1" t="s"><v>0</v></c><c r="B1"><f>1+1</f><v>42</v></c></row></sheetData></worksheet>"#,
        ),
        (
            "xl/worksheets/b.xml",
            r#"<worksheet><sheetData><row r="3"><c r="C3" t="inlineStr"><is><t>See Q1</t></is></c><c r="D3" t="s"><v>1</v></c></row></sheetData></worksheet>"#,
        ),
    ]);
    let doc = extract(Format::Xlsx, &book);
    assert_eq!(anchored(&doc), [("Data!A1", "Revenue"), ("Data!B1", "42"), ("Notes!C3", "See Q1"), ("Notes!D3", "Q1")]);
    assert_eq!(outline(&doc), [("Data", 0, "Data"), ("Notes", 0, "Notes")]);
    assert_eq!(render(&select(&doc.blocks, "Data")), "[Data!A1] Revenue\n[Data!B1] 42");
}

#[test]
fn odt_and_epub_paragraphs() {
    let content = r#"<office:document-content><office:body><office:text>
        <text:h text:outline-level="1">Plan</text:h><text:p>Ship<text:s text:c="2"/>it</text:p>
    </office:text></office:body></office:document-content>"#;
    let doc = extract(Format::Odt, &zipped(&[("content.xml", content)]));
    assert_eq!(anchored(&doc), [("p1", "Plan"), ("p2", "Ship  it")]);
    assert_eq!(outline(&doc), [("Plan", 0, "p1")]);

    let book = zipped(&[
        (
            "META-INF/container.xml",
            r#"<container><rootfiles><rootfile full-path="OEBPS/book.opf"/></rootfiles></container>"#,
        ),
        (
            "OEBPS/book.opf",
            r#"<package><manifest><item id="c1" href="text/one.xhtml"/><item id="c2" href="text/two.xhtml"/></manifest><spine><itemref idref="c2"/><itemref idref="c1"/></spine></package>"#,
        ),
        (
            "OEBPS/text/one.xhtml",
            r#"<html><head><title>x</title></head><body><h1>Later</h1><p>The   end.</p></body></html>"#,
        ),
        ("OEBPS/text/two.xhtml", r#"<html><body><h2>Start</h2><div><p>Once upon</p><p>a time</p></div></body></html>"#),
    ]);
    let doc = extract(Format::Epub, &book);
    assert_eq!(
        anchored(&doc),
        [
            ("ch1/p1", "Start"),
            ("ch1/p2", "Once upon"),
            ("ch1/p3", "a time"),
            ("ch2/p1", "Later"),
            ("ch2/p2", "The end.")
        ]
    );
    assert_eq!(outline(&doc), [("Start", 1, "ch1/p1"), ("Later", 0, "ch2/p1")]);
}

#[test]
fn pdf_pages() {
    let doc = extract(Format::Pdf, &pdf(&["Quarterly revenue", "Hiring plan"]));
    assert_eq!(anchored(&doc), [("page1", "Quarterly revenue"), ("page2", "Hiring plan")]);
    assert!(extract(Format::Pdf, b"not a pdf").blocks.is_empty());
}
