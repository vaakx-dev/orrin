mod diff3;
mod godot;
mod json;
mod keyed;
mod office;
mod text;
mod three;

use std::sync::Arc;

use orrin_core::service::Merger;

fn file_name(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_lowercase()
}

fn extension(path: &str) -> String {
    let name = file_name(path);
    name.rsplit_once('.').map(|(_, extension)| extension.to_string()).unwrap_or_default()
}

pub fn all() -> Vec<Arc<dyn Merger>> {
    vec![
        Arc::new(godot::Godot),
        Arc::new(json::JsonMerger),
        Arc::new(office::docx()),
        Arc::new(office::pptx()),
        Arc::new(office::xlsx()),
        Arc::new(text::Text),
    ]
}
