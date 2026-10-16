use orrin_core::model::OutlineItem;

use super::item;

pub fn outline(source: &str) -> Vec<OutlineItem> {
    let lines: Vec<&str> = source.lines().collect();
    let mut items = Vec::new();
    let mut fence: Option<&str> = None;
    let mut start = 0;
    if lines.first().is_some_and(|l| l.trim_end() == "---") {
        start = lines.iter().skip(1).position(|l| l.trim_end() == "---").map(|i| i + 2).unwrap_or(0);
    }
    for (i, raw) in lines.iter().enumerate().skip(start) {
        let line = raw.trim_start();
        if let Some(open) = fence {
            if line.starts_with(open) {
                fence = None;
            }
            continue;
        }
        if line.starts_with("```") || line.starts_with("~~~") {
            fence = Some(&line[..3]);
            continue;
        }
        let number = i as u32 + 1;
        if let Some((level, title)) = atx(line) {
            items.push(item("heading", title, level - 1, Some(number)));
        } else if let Some(level) = setext(line) {
            let previous = i.checked_sub(1).map(|p| lines[p].trim()).unwrap_or("");
            let claimed = items.last().is_some_and(|h: &OutlineItem| h.line == Some(number - 1));
            if !previous.is_empty() && !claimed && i > start {
                items.push(item("heading", previous, level - 1, Some(number - 1)));
            }
        }
    }
    items
}

fn atx(line: &str) -> Option<(u32, &str)> {
    let level = line.bytes().take_while(|b| *b == b'#').count();
    let rest = &line[level..];
    let valid = (1..=6).contains(&level) && (rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t'));
    valid.then(|| (level as u32, rest.trim().trim_end_matches('#').trim_end()))
}

fn setext(line: &str) -> Option<u32> {
    let line = line.trim_end();
    if line.len() >= 2 && line.bytes().all(|b| b == b'=') {
        Some(1)
    } else if line.len() >= 2 && line.bytes().all(|b| b == b'-') {
        Some(2)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_skip_code_and_front_matter() {
        let doc = "---\ntitle: x\n---\n# Title\ntext\n```\n# not\n```\n## Part ##\nSub\n---\n";
        let names: Vec<(String, u32)> = outline(doc).into_iter().map(|h| (h.name, h.depth)).collect();
        assert_eq!(names, [("Title".into(), 0), ("Part".into(), 1), ("Sub".into(), 1)]);
    }
}
