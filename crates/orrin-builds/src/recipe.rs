use std::collections::BTreeMap;

use globset::{Glob, GlobSet, GlobSetBuilder};
use orrin_core::model::{Entry, MANIFEST_PATH, Recipe, Snapshot};
use orrin_core::service::Store;
use orrin_core::{Error, Result};

pub fn load_all(store: &dyn Store, snapshot: &Snapshot) -> Result<BTreeMap<String, Recipe>> {
    let entry =
        snapshot.get(MANIFEST_PATH).ok_or_else(|| Error::not_found(format!("no {MANIFEST_PATH} in project")))?;
    let text = String::from_utf8(store.get(&entry.hash)?)
        .map_err(|_| Error::invalid(format!("{MANIFEST_PATH} is not text")))?;
    parse(&text)
}

pub fn parse(text: &str) -> Result<BTreeMap<String, Recipe>> {
    let mut table: toml::Table = toml::from_str(text)?;
    match table.remove("recipes") {
        Some(recipes) => Ok(recipes.try_into()?),
        None => Ok(BTreeMap::new()),
    }
}

pub fn check_after(recipes: &BTreeMap<String, Recipe>, name: &str) -> Result<()> {
    let mut path = Vec::new();
    visit(recipes, name, &mut path)
}

fn visit<'a>(recipes: &'a BTreeMap<String, Recipe>, name: &'a str, path: &mut Vec<&'a str>) -> Result<()> {
    if path.contains(&name) {
        return Err(Error::invalid(format!("recipes depend on each other in a loop: {} -> {name}", path.join(" -> "))));
    }
    let recipe = recipes.get(name).ok_or_else(|| Error::not_found(format!("no recipe {name}")))?;
    path.push(name);
    for dep in &recipe.after {
        visit(recipes, dep, path)?;
    }
    path.pop();
    Ok(())
}

pub fn inputs<'a>(recipe: &Recipe, snapshot: &'a Snapshot) -> Result<Vec<&'a Entry>> {
    if recipe.inputs.is_empty() {
        return Ok(snapshot.entries.iter().collect());
    }
    let globs = globs(&recipe.inputs)?;
    Ok(snapshot.entries.iter().filter(|e| globs.is_match(&e.path)).collect())
}

pub fn globs(patterns: &[String]) -> Result<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = Glob::new(pattern).map_err(|err| Error::invalid(format!("bad glob {pattern}: {err}")))?;
        builder.add(glob);
    }
    builder.build().map_err(|err| Error::invalid(err.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_recipes_and_ignores_other_sections() {
        let text =
            "[project]\nname = \"x\"\n[recipes.app]\ninputs = [\"src/**\"]\ncommand = \"make\"\nneeds = [\"make\"]\n";
        let recipes = parse(text).unwrap();
        assert_eq!(recipes["app"].command, "make");
        assert!(parse("[recipes.app]\nnope = 1\n").is_err());
    }

    #[test]
    fn after_must_name_recipes_without_loops() {
        let text = "[recipes.app]\nafter = [\"lib\"]\ncache = \"sccache\"\nwarm = true\n[recipes.lib]\n";
        let recipes = parse(text).unwrap();
        assert_eq!(recipes["app"].cache, orrin_core::model::CompilerCache::Sccache);
        assert!(check_after(&recipes, "app").is_ok());
        let looped = parse("[recipes.a]\nafter = [\"b\"]\n[recipes.b]\nafter = [\"a\"]\n").unwrap();
        assert!(check_after(&looped, "a").is_err());
        assert!(check_after(&parse("[recipes.a]\nafter = [\"x\"]\n").unwrap(), "a").is_err());
    }
}
