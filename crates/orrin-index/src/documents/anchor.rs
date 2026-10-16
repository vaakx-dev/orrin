use super::Block;

pub fn select<'a>(blocks: &'a [Block], anchor: &str) -> Vec<&'a Block> {
    let anchor = anchor.trim();
    if anchor.is_empty() {
        return blocks.iter().collect();
    }
    if let Some(at) = blocks.iter().position(|b| b.anchor == anchor)
        && let Some(level) = blocks[at].level
    {
        let rest = &blocks[at + 1..];
        let end = rest.iter().position(|b| b.level.is_some_and(|l| l <= level)).unwrap_or(rest.len());
        return blocks[at..=at + end].iter().collect();
    }
    blocks.iter().filter(|b| within(&b.anchor, anchor)).collect()
}

pub fn render(blocks: &[&Block]) -> String {
    match blocks {
        [one] => one.text.clone(),
        many => many.iter().map(|b| format!("[{}] {}", b.anchor, b.text)).collect::<Vec<_>>().join("\n"),
    }
}

fn within(block: &str, anchor: &str) -> bool {
    block == anchor || block.strip_prefix(anchor).is_some_and(|rest| rest.starts_with(['/', '!']))
}
