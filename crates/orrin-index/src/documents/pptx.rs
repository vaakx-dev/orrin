use super::Document;
use super::build::Builder;
use super::parts::{Archive, archive, part, relations, resolve};
use super::xml::{Node, attr, name, walk};

#[derive(Default)]
struct Shape {
    placeholder: Option<String>,
    lines: Vec<String>,
}

pub fn extract(bytes: &[u8]) -> Document {
    let Some(mut zip) = archive(bytes) else { return Document::default() };
    let mut doc = Builder::default();
    for (i, slide) in slide_order(&mut zip).iter().enumerate() {
        let anchor = format!("slide{}", i + 1);
        let drawn = part(&mut zip, slide).map(|xml| shapes(&xml)).unwrap_or_default();
        let title = drawn.iter().find(|s| matches!(s.placeholder.as_deref(), Some("title" | "ctrTitle")));
        let title = title.map(|s| s.lines.join(" ")).unwrap_or_else(|| format!("Slide {}", i + 1));
        doc.item("slide", &title, 0, Some(i as u32 + 1), &anchor);
        let text: Vec<String> = drawn.into_iter().flat_map(|s| s.lines).collect();
        doc.text(anchor.clone(), &text.join("\n"), None);
        let notes = notes_of(&mut zip, slide).map(|xml| shapes(&xml)).unwrap_or_default();
        let notes: Vec<String> =
            notes.into_iter().filter(|s| s.placeholder.as_deref() == Some("body")).flat_map(|s| s.lines).collect();
        doc.text(format!("{anchor}/notes"), &notes.join("\n"), None);
    }
    doc.finish()
}

fn shapes(xml: &str) -> Vec<Shape> {
    let mut shapes = Vec::new();
    let mut current: Option<Shape> = None;
    let mut line = String::new();
    let mut in_text = false;
    walk(xml, |node| match node {
        Node::Open(start) => match name(start) {
            "p:sp" | "p:graphicFrame" => current = Some(Shape::default()),
            "p:ph" => {
                if let Some(shape) = current.as_mut() {
                    shape.placeholder = Some(attr(start, "type").unwrap_or_else(|| "body".into()));
                }
            }
            "a:p" => line.clear(),
            "a:t" => in_text = true,
            "a:br" => line.push(' '),
            _ => {}
        },
        Node::Close("a:t") => in_text = false,
        Node::Text(chunk) if in_text => line.push_str(chunk),
        Node::Close("a:p") => {
            if let Some(shape) = current.as_mut().filter(|_| !line.trim().is_empty()) {
                shape.lines.push(line.trim().to_string());
            }
        }
        Node::Close("p:sp" | "p:graphicFrame") => shapes.extend(current.take()),
        _ => {}
    });
    shapes
}

fn notes_of(zip: &mut Archive, slide: &str) -> Option<String> {
    let (dir, file) = slide.rsplit_once('/')?;
    let rels = relations(zip, &format!("{dir}/_rels/{file}.rels"));
    let (target, _) = rels.values().find(|(_, kind)| kind.ends_with("/notesSlide"))?;
    part(zip, &resolve(dir, target))
}

fn slide_order(zip: &mut Archive) -> Vec<String> {
    let listed = listed_slides(zip);
    if !listed.is_empty() {
        return listed;
    }
    let mut numbered: Vec<(u32, String)> = zip
        .file_names()
        .filter_map(|n| {
            let n = n.ok()?;
            let number = n.strip_prefix("ppt/slides/slide")?.strip_suffix(".xml")?.parse().ok()?;
            Some((number, n.to_string()))
        })
        .collect();
    numbered.sort();
    numbered.into_iter().map(|(_, n)| n).collect()
}

fn listed_slides(zip: &mut Archive) -> Vec<String> {
    let Some(presentation) = part(zip, "ppt/presentation.xml") else { return Vec::new() };
    let targets = relations(zip, "ppt/_rels/presentation.xml.rels");
    let mut slides = Vec::new();
    walk(&presentation, |node| {
        if let Node::Open(start) = node
            && name(start) == "p:sldId"
            && let Some((target, _)) = attr(start, "r:id").and_then(|id| targets.get(&id))
        {
            slides.push(resolve("ppt", target));
        }
    });
    slides
}
