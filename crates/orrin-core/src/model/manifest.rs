use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

use super::JobKind;
use crate::error::Result;

pub const MANIFEST_PATH: &str = "orrin.toml";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Manifest {
    pub include: Vec<String>,
    pub ignore: Vec<String>,
    pub rules: BTreeMap<String, Rule>,
    pub targets: BTreeMap<String, Target>,
    pub packages: BTreeMap<String, Package>,
}

impl Manifest {
    pub fn parse(text: &str) -> Result<Self> {
        Ok(toml::from_str(text)?)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Rule {
    pub setup: Option<String>,
    pub command: String,
    pub deps: Deps,
    pub output: String,
    pub outputs: Vec<String>,
    pub cache: CompilerCache,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Deps {
    #[default]
    None,
    Msvc,
    Gcc,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Target {
    pub after: Vec<String>,
    pub inputs: Vec<String>,
    pub command: String,
    pub outputs: Vec<String>,
    pub steps: Vec<Step>,
    pub needs: Vec<String>,
    pub cpu: Option<u32>,
    pub ram_mb: Option<u64>,
    pub gpu: bool,
    pub warm: Warm,
    pub share: Vec<String>,
    pub cache: CompilerCache,
    pub kind: Option<JobKind>,
    pub link_outputs: bool,
    pub scratch: Vec<String>,
    pub deliver: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Step {
    pub rule: String,
    #[serde(deserialize_with = "one_or_many")]
    pub files: Vec<String>,
    #[serde(rename = "in", deserialize_with = "one_or_many")]
    pub input: Vec<String>,
    pub out: Option<String>,
    pub vars: BTreeMap<String, String>,
}

pub const PREVIOUS: &str = "@previous";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Package {
    pub git: Option<String>,
    pub tag: Option<String>,
    pub rev: Option<String>,
    pub url: Option<String>,
    pub path: Option<String>,
    pub build: Option<PackageBuild>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PackageBuild {
    Target(String),
    Inline(Box<Target>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Warm {
    On(bool),
    Pool(u32),
}

impl Default for Warm {
    fn default() -> Self {
        Self::On(false)
    }
}

impl Warm {
    pub fn pool(self, default: u32) -> Option<u32> {
        match self {
            Self::On(true) => Some(default.max(1)),
            Self::Pool(size) if size > 0 => Some(size),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompilerCache {
    #[default]
    None,
    Orrin,
    Sccache,
}

fn one_or_many<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }
    Ok(match OneOrMany::deserialize(deserializer)? {
        OneOrMany::One(one) => vec![one],
        OneOrMany::Many(many) => many,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_rules_targets_and_packages() {
        let text = r#"
[rules.cxx]
setup = "vsdevcmd -arch=x64"
command = "cl /c {flags} /showIncludes /Fo{out} {in}"
deps = "msvc"
output = ".build/{path}.obj"
outputs = ["{out.path}.pdb"]
cache = "orrin"

[targets.game]
after = ["gen"]
steps = [
  { rule = "cxx", files = "game/**/*.cpp", vars = { flags = "/O2" } },
  { rule = "dll", in = "@previous", out = "bin/x.dll" },
]
cpu = 12
warm = 2
kind = "build"
deliver = true

[targets.gen]
command = "gen"

[packages.godot-cpp]
git = "https://example.com/godot-cpp.git"
tag = "v1"
build = "lib"

[packages.inline]
path = "../x"
build = { command = "make" }
"#;
        let manifest = Manifest::parse(text).unwrap();
        assert_eq!(manifest.rules["cxx"].deps, Deps::Msvc);
        assert_eq!(manifest.rules["cxx"].cache, CompilerCache::Orrin);
        assert_eq!(manifest.rules["cxx"].outputs, ["{out.path}.pdb"]);
        let game = &manifest.targets["game"];
        assert_eq!(game.steps[0].files, ["game/**/*.cpp"]);
        assert_eq!(game.steps[1].input, [PREVIOUS]);
        assert_eq!(game.warm, Warm::Pool(2));
        assert!(game.deliver && !manifest.targets["gen"].deliver);
        assert_eq!(manifest.packages["godot-cpp"].build, Some(PackageBuild::Target("lib".into())));
        assert!(matches!(&manifest.packages["inline"].build, Some(PackageBuild::Inline(t)) if t.command == "make"));
        assert!(Manifest::parse("[project]\nname = \"x\"\n").unwrap().targets.is_empty());
        assert!(Manifest::parse("[targets.app]\nnope = 1\n").is_err());
    }
}
