use orrin_core::model::{Entry, MANIFEST_PATH, Manifest, Snapshot, Target};
use orrin_core::service::Store;
use orrin_core::{Error, Result};

use crate::paths;

pub fn load(store: &dyn Store, snapshot: &Snapshot) -> Result<Manifest> {
    let entry =
        snapshot.get(MANIFEST_PATH).ok_or_else(|| Error::not_found(format!("no {MANIFEST_PATH} in project")))?;
    let text = String::from_utf8(store.get(&entry.hash)?)
        .map_err(|_| Error::invalid(format!("{MANIFEST_PATH} is not text")))?;
    Manifest::parse(&text)
}

pub fn target<'a>(manifest: &'a Manifest, name: &str) -> Result<&'a Target> {
    manifest.targets.get(name).ok_or_else(|| Error::not_found(format!("no target {name}")))
}

pub fn check_after(manifest: &Manifest, name: &str) -> Result<()> {
    let mut path = Vec::new();
    visit(manifest, name, &mut path)
}

fn visit<'a>(manifest: &'a Manifest, name: &'a str, path: &mut Vec<&'a str>) -> Result<()> {
    if path.contains(&name) {
        return Err(Error::invalid(format!("targets depend on each other in a loop: {} -> {name}", path.join(" -> "))));
    }
    let target = target(manifest, name)?;
    path.push(name);
    for dep in &target.after {
        visit(manifest, dep, path)?;
    }
    path.pop();
    Ok(())
}

pub fn inputs<'a>(target: &Target, snapshot: &'a Snapshot) -> Result<Vec<&'a Entry>> {
    if target.inputs.is_empty() {
        return Ok(snapshot.entries.iter().collect());
    }
    let globs = paths::globs(&target.inputs)?;
    Ok(snapshot.entries.iter().filter(|e| globs.is_match(&e.path)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn after_must_name_targets_without_loops() {
        let manifest = Manifest::parse("[targets.app]\nafter = [\"lib\"]\n[targets.lib]\n").unwrap();
        assert!(check_after(&manifest, "app").is_ok());
        let looped = Manifest::parse("[targets.a]\nafter = [\"b\"]\n[targets.b]\nafter = [\"a\"]\n").unwrap();
        assert!(check_after(&looped, "a").is_err());
        assert!(check_after(&Manifest::parse("[targets.a]\nafter = [\"x\"]\n").unwrap(), "a").is_err());
    }
}
