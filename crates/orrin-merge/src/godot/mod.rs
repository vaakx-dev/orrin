mod scene;

use orrin_core::service::{Merged, Merger};

use crate::{extension, keyed, text, three};

fn merge_section(
    key: &str,
    base: Option<&String>,
    ours: Option<&String>,
    theirs: Option<&String>,
) -> Result<Option<String>, String> {
    three::merge(key, base, ours, theirs, |base, ours, theirs| {
        let line = |text: &str| format!("{text}\n");
        text::merge_lines(&line(base), &line(ours), &line(theirs))
            .map(|merged| merged.trim_end().to_string())
            .map_err(|reason| format!("{key}: {reason}"))
    })
}

fn merge_scene(base: &str, ours: &str, theirs: &str) -> Result<String, String> {
    let parse = |text: &str| scene::sections(&scene::without_load_steps(text));
    let mut merged = keyed::merge(&parse(base), &parse(ours), &parse(theirs), merge_section)?;
    if scene::has_load_steps(ours) {
        scene::set_load_steps(&mut merged);
    }
    Ok(scene::render(&merged))
}

pub struct Godot;

impl Merger for Godot {
    fn name(&self) -> &'static str {
        "godot"
    }

    fn handles(&self, path: &str) -> bool {
        matches!(extension(path).as_str(), "tscn" | "tres")
    }

    fn merge(&self, _path: &str, base: &[u8], ours: &[u8], theirs: &[u8]) -> Merged {
        text::merge_utf8(base, ours, theirs, merge_scene)
    }
}
