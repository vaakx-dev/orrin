use super::Document;
use super::build::Builder;
use super::parts::{archive, part};
use super::xml::{Node, attr, name, walk};

struct Paragraph {
    number: u32,
    level: Option<u32>,
    text: String,
}

pub fn extract(bytes: &[u8]) -> Document {
    let Some(content) = archive(bytes).and_then(|mut zip| part(&mut zip, "content.xml")) else {
        return Document::default();
    };
    let mut doc = Builder::default();
    let mut open: Vec<Paragraph> = Vec::new();
    let mut count = 0;
    walk(&content, |node| match node {
        Node::Open(start) => match name(start) {
            tag @ ("text:p" | "text:h") => {
                count += 1;
                let level = (tag == "text:h")
                    .then(|| attr(start, "text:outline-level").and_then(|l| l.parse().ok()).unwrap_or(1));
                open.push(Paragraph { number: count, level, text: String::new() });
            }
            "text:s" => {
                let spaces = attr(start, "text:c").and_then(|c| c.parse().ok()).unwrap_or(1);
                push(&mut open, &" ".repeat(spaces));
            }
            "text:tab" => push(&mut open, "\t"),
            "text:line-break" => push(&mut open, "\n"),
            _ => {}
        },
        Node::Text(chunk) => push(&mut open, chunk),
        Node::Close("text:p" | "text:h") => {
            if let Some(p) = open.pop() {
                doc.text(format!("p{}", p.number), &p.text, p.level);
            }
        }
        _ => {}
    });
    doc.finish()
}

fn push(open: &mut [Paragraph], text: &str) {
    if let Some(p) = open.last_mut() {
        p.text.push_str(text);
    }
}
