mod common;

use common::{clean_text, merge};
use orrin_core::service::Merged;

const BASE: &str = "{\n  \"name\": \"app\",\n  \"version\": \"1.0.0\",\n  \"deps\": {\n    \"a\": \"1\"\n  }\n}\n";

#[test]
fn separate_keys_merge_cleanly() {
    let ours = BASE.replace("1.0.0", "1.1.0");
    let theirs = BASE.replace("\"a\": \"1\"", "\"a\": \"1\",\n    \"b\": \"2\"");
    let merged = clean_text(merge("package.json", BASE.as_bytes(), ours.as_bytes(), theirs.as_bytes()));
    let expected = "{\n  \"name\": \"app\",\n  \"version\": \"1.1.0\",\n  \"deps\": {\n    \"a\": \"1\",\n    \"b\": \"2\"\n  }\n}\n";
    assert_eq!(merged, expected);
}

#[test]
fn same_key_changed_twice_conflicts() {
    let ours = BASE.replace("\"app\"", "\"ours\"");
    let theirs = BASE.replace("\"app\"", "\"theirs\"");
    let merged = merge("package.json", BASE.as_bytes(), ours.as_bytes(), theirs.as_bytes());
    assert!(matches!(merged, Merged::Conflict(reason) if reason.contains("$.name")));
}

#[test]
fn invalid_json_falls_back_to_text() {
    let base = "{\n  // a\n  \"x\": 1,\n  // b\n  \"y\": 2\n}\n";
    let ours = base.replace("\"x\": 1", "\"x\": 10");
    let theirs = base.replace("\"y\": 2", "\"y\": 20");
    let merged = clean_text(merge("tsconfig.json", base.as_bytes(), ours.as_bytes(), theirs.as_bytes()));
    assert_eq!(merged, "{\n  // a\n  \"x\": 10,\n  // b\n  \"y\": 20\n}\n");
}
