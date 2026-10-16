use orrin_core::service::{Merged, Merger};

use crate::{diff3, extension, file_name};

const EXTENSIONS: &[&str] = &[
    "txt",
    "md",
    "markdown",
    "rst",
    "adoc",
    "rs",
    "ts",
    "tsx",
    "js",
    "jsx",
    "mjs",
    "cjs",
    "py",
    "pyi",
    "c",
    "h",
    "cc",
    "cpp",
    "cxx",
    "hpp",
    "hh",
    "cs",
    "java",
    "kt",
    "kts",
    "go",
    "rb",
    "php",
    "lua",
    "swift",
    "zig",
    "gd",
    "gdshader",
    "shader",
    "glsl",
    "hlsl",
    "wgsl",
    "html",
    "htm",
    "css",
    "scss",
    "less",
    "vue",
    "svelte",
    "xml",
    "svg",
    "yaml",
    "yml",
    "toml",
    "ini",
    "cfg",
    "conf",
    "sh",
    "bash",
    "zsh",
    "bat",
    "cmd",
    "ps1",
    "csv",
    "tsv",
    "sql",
    "cmake",
    "godot",
    "import",
    "jsonc",
    "json5",
    "env",
    "lock",
    "gradle",
    "properties",
    "tex",
    "srt",
    "vtt",
];

const NAMES: &[&str] =
    &["makefile", "dockerfile", "license", "readme", ".gitignore", ".gitattributes", ".editorconfig"];

fn lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

pub fn merge_lines(base: &str, ours: &str, theirs: &str) -> Result<String, String> {
    diff3::merge(&lines(base), &lines(ours), &lines(theirs), |_| None)
        .map(|merged| merged.concat())
        .map_err(|at| format!("overlapping changes near line {}", at + 1))
}

pub fn merge_utf8(
    base: &[u8],
    ours: &[u8],
    theirs: &[u8],
    merge: impl Fn(&str, &str, &str) -> Result<String, String>,
) -> Merged {
    let (Ok(base), Ok(ours), Ok(theirs)) =
        (std::str::from_utf8(base), std::str::from_utf8(ours), std::str::from_utf8(theirs))
    else {
        return Merged::Conflict("not UTF-8 text".into());
    };
    match merge(base, ours, theirs) {
        Ok(merged) => Merged::Clean(merged.into_bytes()),
        Err(reason) => Merged::Conflict(reason),
    }
}

pub fn merge_bytes(base: &[u8], ours: &[u8], theirs: &[u8]) -> Merged {
    merge_utf8(base, ours, theirs, merge_lines)
}

pub struct Text;

impl Merger for Text {
    fn name(&self) -> &'static str {
        "text"
    }

    fn handles(&self, path: &str) -> bool {
        EXTENSIONS.contains(&extension(path).as_str()) || NAMES.contains(&file_name(path).as_str())
    }

    fn merge(&self, _path: &str, base: &[u8], ours: &[u8], theirs: &[u8]) -> Merged {
        merge_bytes(base, ours, theirs)
    }
}
