#[cfg_attr(not(windows), allow(dead_code))]
pub struct Spec {
    pub name: &'static str,
    pub exes: &'static [&'static str],
    pub probe: Option<&'static [&'static str]>,
    pub installer: Option<&'static str>,
    pub subdirs: &'static [&'static str],
    pub prefix: Option<&'static str>,
}

const VERSION: Option<&[&str]> = Some(&["--version"]);

const fn tool(name: &'static str, probe: Option<&'static [&'static str]>) -> Spec {
    Spec { name, exes: &[], probe, installer: None, subdirs: &[""], prefix: None }
}

pub const SPECS: &[Spec] = &[
    Spec { installer: Some("git"), subdirs: &["cmd", "bin"], ..tool("git", VERSION) },
    Spec { installer: Some("git"), subdirs: &["bin"], ..tool("bash", VERSION) },
    Spec { exes: &["python", "python3"], installer: Some("python 3"), ..tool("python", VERSION) },
    tool("py", VERSION),
    tool("pip", VERSION),
    tool("uv", VERSION),
    Spec { installer: Some("node.js"), ..tool("node", VERSION) },
    tool("npm", VERSION),
    tool("pnpm", VERSION),
    tool("yarn", VERSION),
    tool("bun", VERSION),
    tool("deno", VERSION),
    tool("cargo", VERSION),
    tool("rustc", VERSION),
    tool("go", Some(&["version"])),
    tool("java", Some(&["-version"])),
    tool("dotnet", VERSION),
    Spec { installer: Some("cmake"), subdirs: &["bin"], ..tool("cmake", VERSION) },
    tool("ninja", VERSION),
    tool("make", VERSION),
    tool("scons", VERSION),
    tool("sccache", VERSION),
    tool("ccache", VERSION),
    tool("gcc", VERSION),
    tool("clang", VERSION),
    tool("cl", None),
    tool("msbuild", Some(&["-version", "-nologo"])),
    Spec { exes: &["godot", "godot4"], installer: Some("godot"), prefix: Some("godot"), ..tool("godot", VERSION) },
    Spec { installer: Some("blender"), ..tool("blender", VERSION) },
    tool("ffmpeg", Some(&["-version"])),
    Spec { installer: Some("7-zip"), ..tool("7z", None) },
    tool("gh", VERSION),
];

impl Spec {
    pub fn exes(&self) -> Vec<&'static str> {
        if self.exes.is_empty() { vec![self.name] } else { self.exes.to_vec() }
    }
}

pub fn spec(name: &str) -> Option<&'static Spec> {
    SPECS.iter().find(|s| s.name == name)
}
