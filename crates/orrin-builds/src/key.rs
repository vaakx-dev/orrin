use orrin_core::model::{Recipe, Snapshot, Tool};
use orrin_core::{Hash, Result};
use serde::Serialize;

use crate::recipe;

pub struct Action<'a> {
    pub name: &'a str,
    pub recipe: &'a Recipe,
    pub args: &'a [String],
    pub snapshot: &'a Snapshot,
    pub tools: &'a [(String, Option<Tool>)],
    pub after: &'a [Hash],
}

#[derive(Serialize)]
struct KeyParts<'a> {
    os: &'static str,
    arch: &'static str,
    name: &'a str,
    recipe: &'a Recipe,
    args: &'a [String],
    inputs: Vec<(&'a str, &'a Hash)>,
    tools: Vec<(&'a str, Option<&'a str>)>,
    after: &'a [Hash],
}

pub fn action(action: Action) -> Result<Hash> {
    let parts = KeyParts {
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        name: action.name,
        recipe: action.recipe,
        args: action.args,
        inputs: recipe::inputs(action.recipe, action.snapshot)?
            .into_iter()
            .map(|e| (e.path.as_str(), &e.hash))
            .collect(),
        tools: action.tools.iter().map(|(n, t)| (n.as_str(), t.as_ref().map(tool_identity))).collect(),
        after: action.after,
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

    fn key(recipe: &Recipe, snapshot: &Snapshot, after: &[Hash]) -> Hash {
        action(Action { name: "app", recipe, args: &[], snapshot, tools: &[], after }).unwrap()
    }

    #[test]
    fn key_follows_inputs_and_after_not_other_files() {
        let recipe = Recipe { inputs: vec!["src/**".into()], command: "cc".into(), ..Default::default() };
        let a = key(&recipe, &snapshot("one"), &[]);
        assert_ne!(a, key(&recipe, &snapshot("two"), &[]));
        let mut other = snapshot("one");
        other.entries.retain(|e| e.path != "README.md");
        assert_eq!(a, key(&recipe, &other, &[]));
        assert_ne!(a, key(&recipe, &other, &[Hash::of(b"lib")]));
    }
}
