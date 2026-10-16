mod docx;
mod pptx;
mod xlsx;
mod xml;

use std::io::{Cursor, Read};

use zip::ZipArchive;

pub use docx::outline as docx;
pub use pptx::outline as pptx;
pub use xlsx::outline as xlsx;

type Archive<'a> = ZipArchive<Cursor<&'a [u8]>>;

fn archive(bytes: &[u8]) -> Option<Archive<'_>> {
    ZipArchive::new(Cursor::new(bytes)).ok()
}

fn part(archive: &mut Archive, name: &str) -> Option<String> {
    let mut file = archive.by_name(name).ok()?;
    let mut text = String::new();
    file.read_to_string(&mut text).ok()?;
    Some(text)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

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

    fn names(items: Vec<orrin_core::model::OutlineItem>) -> Vec<(String, u32)> {
        items.into_iter().map(|i| (i.name, i.depth)).collect()
    }

    #[test]
    fn office_outlines() {
        let styles = r#"<w:styles><w:style w:styleId="Berschrift2"><w:name w:val="heading 2"/></w:style></w:styles>"#;
        let document = r#"<w:document><w:body>
            <w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Intro &amp; goals</w:t></w:r></w:p>
            <w:p><w:r><w:t>Body</w:t></w:r></w:p>
            <w:p><w:pPr><w:pStyle w:val="Berschrift2"/></w:pPr><w:r><w:t>Detail</w:t></w:r></w:p>
        </w:body></w:document>"#;
        let doc = zipped(&[("word/document.xml", document), ("word/styles.xml", styles)]);
        assert_eq!(names(docx(&doc)), [("Intro & goals".into(), 0), ("Detail".into(), 1)]);

        let slide = |title: &str| {
            format!(
                r#"<p:sld><p:sp><p:nvSpPr><p:nvPr><p:ph type="title"/></p:nvPr></p:nvSpPr><a:p><a:r><a:t>{title}</a:t></a:r></a:p></p:sp></p:sld>"#
            )
        };
        let deck = zipped(&[("ppt/slides/slide2.xml", &slide("Second")), ("ppt/slides/slide1.xml", &slide("First"))]);
        assert_eq!(names(pptx(&deck)), [("First".into(), 0), ("Second".into(), 0)]);

        let book = zipped(&[(
            "xl/workbook.xml",
            r#"<workbook><sheets><sheet name="Data"/><sheet name="Chart"/></sheets></workbook>"#,
        )]);
        assert_eq!(names(xlsx(&book)), [("Data".into(), 0), ("Chart".into(), 0)]);
    }
}
