use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use orrin_core::model::{Manifest, PackageBuild, Rule, Stream, Target};
use orrin_core::service::PackageNeed;
use orrin_core::{Error, Hash, JobId, Result};

use crate::context::Context;
use crate::plan::Plan;

const OPEN: &str = "{pkg:";

#[derive(Clone, Debug)]
pub struct Placed {
    pub name: String,
    pub dir: PathBuf,
    pub key: Hash,
}

pub async fn keys(ctx: &Context, plan: &Plan, target: &Target) -> Result<Vec<Hash>> {
    let mut keys = Vec::new();
    for name in used(&plan.manifest, target) {
        keys.push(ctx.hub.packages.key(need(plan, &name)?).await?);
    }
    Ok(keys)
}

pub async fn ensure(ctx: &Context, plan: &Plan, id: &JobId) -> Result<Vec<Placed>> {
    let mut placed = Vec::new();
    for name in used(&plan.manifest, &plan.target) {
        let ready = ctx.hub.packages.ensure(need(plan, &name)?).await?;
        ctx.table.log(id, Stream::System, format!("package {name}: {} at {}", ready.note, ready.dir.display()));
        placed.push(Placed { name, dir: ready.dir, key: ready.key });
    }
    Ok(placed)
}

pub fn expand(text: &str, placed: &[Placed]) -> String {
    placed.iter().fold(text.to_string(), |text, p| text.replace(&token(&p.name), &p.dir.to_string_lossy()))
}

pub fn dirs(placed: &[Placed]) -> BTreeMap<String, String> {
    placed.iter().map(|p| (name(&p.name), p.dir.to_string_lossy().into_owned())).collect()
}

pub fn stand_ins(placed: &[Placed]) -> BTreeMap<String, String> {
    placed.iter().map(|p| (name(&p.name), format!("<pkg {}>", p.key))).collect()
}

fn name(package: &str) -> String {
    format!("pkg:{package}")
}

fn token(package: &str) -> String {
    format!("{{{}}}", name(package))
}

fn need(plan: &Plan, name: &str) -> Result<PackageNeed> {
    let package = plan.manifest.packages.get(name).cloned();
    let package = package.ok_or_else(|| Error::not_found(format!("no package {name} in the manifest")))?;
    let rules = match &package.build {
        Some(PackageBuild::Inline(target)) => rules_of(&plan.manifest, target),
        _ => BTreeMap::new(),
    };
    Ok(PackageNeed { project: plan.project.clone(), name: name.to_string(), package, rules })
}

fn rules_of(manifest: &Manifest, target: &Target) -> BTreeMap<String, Rule> {
    let named = target.steps.iter().filter_map(|step| manifest.rules.get_key_value(&step.rule));
    named.map(|(name, rule)| (name.clone(), rule.clone())).collect()
}

fn used(manifest: &Manifest, target: &Target) -> BTreeSet<String> {
    let mut text = serde_json::to_string(target).unwrap_or_default();
    for rule in rules_of(manifest, target).values() {
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
            "[rules.cc]\ncommand = \"cc -I{pkg:b}/include {in}\"\n[targets.app]\ncommand = \"link {pkg:a}/lib\"\nsteps = [{ rule = \"cc\", vars = { x = \"{pkg:d}\" } }]\n",
        )
        .unwrap();
        let names: Vec<String> = used(&manifest, &manifest.targets["app"]).into_iter().collect();
        assert_eq!(names, ["a", "b", "d"]);
        let placed = vec![Placed { name: "a".into(), dir: PathBuf::from("/p/a"), key: Hash::of(b"a") }];
        assert_eq!(expand("link {pkg:a}/lib {pkg:c}", &placed), "link /p/a/lib {pkg:c}");
        assert_eq!(dirs(&placed)["pkg:a"], "/p/a");
        assert_eq!(stand_ins(&placed)["pkg:a"], format!("<pkg {}>", Hash::of(b"a")));
    }
}
