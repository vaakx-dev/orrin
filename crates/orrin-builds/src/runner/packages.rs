use std::collections::BTreeSet;
use std::path::PathBuf;

use orrin_core::model::{Manifest, Stream, Target};
use orrin_core::service::PackageNeed;
use orrin_core::{Error, Hash, JobId, Result};

use crate::context::Context;
use crate::plan::Plan;

const OPEN: &str = "{pkg:";

pub type Placed = Vec<(String, PathBuf)>;

pub async fn keys(ctx: &Context, plan: &Plan, target: &Target) -> Result<Vec<Hash>> {
    let mut keys = Vec::new();
    for name in used(&plan.manifest, target) {
        keys.push(ctx.hub.packages.key(need(plan, &name)?).await?);
    }
    Ok(keys)
}

pub async fn ensure(ctx: &Context, plan: &Plan, id: &JobId) -> Result<Placed> {
    let mut placed = Vec::new();
    for name in used(&plan.manifest, &plan.target) {
        let ready = ctx.hub.packages.ensure(need(plan, &name)?).await?;
        ctx.table.log(id, Stream::System, format!("package {name}: {} at {}", ready.note, ready.dir.display()));
        placed.push((name, ready.dir));
    }
    Ok(placed)
}

pub fn expand(text: &str, placed: &Placed) -> String {
    placed
        .iter()
        .fold(text.to_string(), |text, (name, dir)| text.replace(&format!("{OPEN}{name}}}"), &dir.to_string_lossy()))
}

fn need(plan: &Plan, name: &str) -> Result<PackageNeed> {
    let package = plan.manifest.packages.get(name).cloned();
    let package = package.ok_or_else(|| Error::not_found(format!("no package {name} in the manifest")))?;
    Ok(PackageNeed { project: plan.project.clone(), name: name.to_string(), package })
}

fn used(manifest: &Manifest, target: &Target) -> BTreeSet<String> {
    let mut text = serde_json::to_string(target).unwrap_or_default();
    for rule in target.steps.iter().filter_map(|step| manifest.rules.get(&step.rule)) {
        text.push_str(&serde_json::to_string(rule).unwrap_or_default());
    }
    text.split(OPEN).skip(1).filter_map(|rest| rest.split_once('}')).map(|(name, _)| name.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_and_fills_package_placeholders() {
        let manifest = Manifest::parse(
            "[rules.cc]\ncommand = \"cc -I{pkg:b}/include {in}\"\n[targets.app]\ncommand = \"link {pkg:a}/lib\"\nsteps = [{ rule = \"cc\" }]\n",
        )
        .unwrap();
        let names: Vec<String> = used(&manifest, &manifest.targets["app"]).into_iter().collect();
        assert_eq!(names, ["a", "b"]);
        let placed = vec![("a".to_string(), PathBuf::from("/p/a"))];
        assert_eq!(expand("link {pkg:a}/lib {pkg:c}", &placed), "link /p/a/lib {pkg:c}");
    }
}
