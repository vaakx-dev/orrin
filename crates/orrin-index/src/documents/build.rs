use orrin_core::model::OutlineItem;

use super::{Block, Document};

#[derive(Default)]
pub struct Builder {
    doc: Document,
}

impl Builder {
    pub fn text(&mut self, anchor: String, text: &str, level: Option<u32>) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        if let Some(level) = level {
            let kind = if level == 0 { "title" } else { "heading" };
            let name = text.lines().next().unwrap_or(text);
            self.item(kind, name, level.saturating_sub(1), None, &anchor);
        }
        self.doc.blocks.push(Block { anchor, text: text.to_string(), level });
    }

    pub fn item(&mut self, kind: &str, name: &str, depth: u32, line: Option<u32>, anchor: &str) {
        self.doc.outline.push(OutlineItem {
            kind: kind.to_string(),
            name: name.trim().to_string(),
            depth,
            line,
            anchor: Some(anchor.to_string()),
        });
    }

    pub fn finish(self) -> Document {
        self.doc
    }
}

pub fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
