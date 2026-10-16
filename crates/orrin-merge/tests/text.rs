mod common;

use common::{clean_text, merge};
use orrin_core::service::Merged;

#[test]
fn separate_edits_merge_cleanly() {
    let base = "fn main() {\n    one();\n    two();\n    three();\n}\n";
    let ours = "fn main() {\n    first();\n    two();\n    three();\n}\n";
    let theirs = "fn main() {\n    one();\n    two();\n    three();\n    four();\n}\n";
    let merged = clean_text(merge("src/main.rs", base.as_bytes(), ours.as_bytes(), theirs.as_bytes()));
    assert_eq!(merged, "fn main() {\n    first();\n    two();\n    three();\n    four();\n}\n");
}

#[test]
fn overlapping_edits_conflict() {
    let merged = merge("notes.md", b"a\nb\nc\n", b"a\nours\nc\n", b"a\ntheirs\nc\n");
    assert!(matches!(merged, Merged::Conflict(_)));
}

#[test]
fn binary_files_are_not_handled() {
    let mergers = orrin_merge::all();
    for path in ["art/hero.png", "models/ship.glb", "scene.blend", "manual.pdf"] {
        assert!(mergers.iter().all(|merger| !merger.handles(path)), "{path}");
    }
}
