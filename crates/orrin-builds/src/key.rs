use orrin_core::model::{Recipe, Snapshot, Tool};
use orrin_core::{Hash, Result};
use serde::Serialize;

use crate::recipe;

#[derive(Serialize)]
struct KeyParts<'a> {
    os: &'static str,
    arch: &'static str,
    name: &'a str,
    recipe: &'a Recipe,
    args: &'a [String],
    inputs: Vec<(&'a str, &'a Hash)>,
    tools: Vec<(&'a str, Option<&'a str>)>,
}

pub fn action(
    name: &str,
    recipe: &Recipe,
    args: &[String],
    snapshot: &Snapshot,
    tools: &[(String, Option<Tool>)],
) -> Result<Hash> {
    let parts = KeyParts {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        name,
        recipe,
        args,
        inputs: recipe::inputs(recipe, snapshot)?.into_iter().map(|e| (e.path.as_str(), &e.hash)).collect(),
        tools: tools.iter().map(|(n, t)| (n.as_str(), t.as_ref().map(tool_identity))).collect(),
    };
    Ok(Hash::of(&serde_json::to_vec(&parts)?))
}

fn tool_identity(tool: &Tool) -> &str {
    tool.version.as_deref().unwrap_or(&tool.path)
}

#[cfg(test)]
mod tests {
    use orrin_core::model::Entry;

    use super::*;

    fn snapshot(text: &str) -> Snapshot {
        let entry = |path: &str, body: &str| Entry {
            path: path.into(),
            hash: Hash::of(body.as_bytes()),
            size: 1,
            mode: Default::default(),
        };
        Snapshot::new(vec![entry("src/a.c", text), entry("README.md", "readme")])
    }

    #[test]
    fn key_follows_inputs_not_other_files() {
        let recipe = Recipe { inputs: vec!["src/**".into()], command: "cc".into(), ..Default::default() };
        let a = action("app", &recipe, &[], &snapshot("one"), &[]).unwrap();
        let b = action("app", &recipe, &[], &snapshot("two"), &[]).unwrap();
        assert_ne!(a, b);
        let mut other = snapshot("one");
        other.entries.retain(|e| e.path != "README.md");
        assert_eq!(a, action("app", &recipe, &[], &other, &[]).unwrap());
    }
}
