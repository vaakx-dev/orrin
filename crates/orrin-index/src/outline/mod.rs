mod godot;
mod markdown;

pub use godot::outline as godot;
pub use markdown::outline as markdown;

use orrin_core::model::OutlineItem;

fn item(kind: &str, name: impl Into<String>, depth: u32, line: Option<u32>) -> OutlineItem {
    OutlineItem { kind: kind.to_string(), name: name.into(), depth, line, anchor: None }
}
