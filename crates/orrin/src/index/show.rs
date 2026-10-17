use orrin_core::api::{FindOutput, OutlineOutput, SearchOutput};
use orrin_core::model::Tool;

use crate::table::{Table, size};

pub fn found(out: &FindOutput) -> String {
    let mut table = Table::new(&["PATH", "SIZE"]);
    for hit in &out.files {
        table.row(vec![hit.path.clone(), size(hit.size)]);
    }
    with_more(table.render("No files match."), out.truncated)
}

pub fn hits(out: &SearchOutput) -> String {
    let lines: Vec<String> =
        out.hits.iter().map(|h| format!("{}:{}:{}: {}", h.path, h.line, h.column, h.text.trim())).collect();
    match lines.is_empty() {
        true => "No matches.".to_string(),
        false => with_more(lines.join("\n"), out.truncated),
    }
}

pub fn outline(out: &OutlineOutput) -> String {
    if out.items.is_empty() {
        return format!("No outline for {}.", out.path);
    }
    let lines: Vec<String> = out
        .items
        .iter()
        .map(|item| {
            let line = item.line.map(|n| n.to_string()).unwrap_or_default();
            let indent = "  ".repeat(item.depth as usize);
            format!("{line:>5}  {indent}{}  ({})", item.name, item.kind)
        })
        .collect();
    lines.join("\n")
}

pub fn tools(tools: &[Tool]) -> String {
    let mut table = Table::new(&["TOOL", "VERSION", "PATH"]);
    for tool in tools {
        table.row(vec![tool.name.clone(), tool.version.clone().unwrap_or_else(|| "?".into()), tool.path.clone()]);
    }
    table.render("Not found on this PC.")
}

fn with_more(text: String, truncated: bool) -> String {
    match truncated {
        true => format!("{text}\n… more results; narrow it down or raise --limit"),
        false => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orrin_core::Clock;
    use orrin_core::model::{OutlineItem, SearchHit};

    #[test]
    fn search_hits_read_like_grep() {
        let out = SearchOutput {
            clock: Clock(3),
            hits: vec![SearchHit {
                path: "src/main.rs".into(),
                line: 4,
                column: 9,
                text: "  fn main() {".into(),
                anchor: None,
            }],
            truncated: true,
        };
        assert_eq!(hits(&out), "src/main.rs:4:9: fn main() {\n… more results; narrow it down or raise --limit");
    }

    #[test]
    fn outlines_indent_by_depth() {
        let item = |name: &str, depth, line| OutlineItem {
            kind: "heading".into(),
            name: name.into(),
            depth,
            line,
            anchor: None,
        };
        let out = OutlineOutput {
            clock: Clock(1),
            path: "README.md".into(),
            items: vec![item("Intro", 0, Some(1)), item("Setup", 1, Some(12))],
        };
        assert_eq!(outline(&out), "    1  Intro  (heading)\n   12    Setup  (heading)");
    }
}
