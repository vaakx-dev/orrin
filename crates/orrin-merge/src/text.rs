use orrin_core::service::{Merged, Merger};

use crate::diff3::{self, Chunk};
use crate::{extension, file_name};

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

fn line_end(text: &str) -> &'static str {
    if text.contains("\r\n") { "\r\n" } else { "\n" }
}

struct Sides {
    base: String,
    ours: String,
    theirs: String,
    end: &'static str,
}

impl Sides {
    fn new(base: &str, ours: &str, theirs: &str) -> Self {
        let end = if line_end(ours) != line_end(base) { line_end(ours) } else { line_end(theirs) };
        let lf = |text: &str| text.replace("\r\n", "\n");
        Self { base: lf(base), ours: lf(ours), theirs: lf(theirs), end }
    }

    fn merge<'a>(&'a self, resolve: impl FnMut(&Chunk<&'a str>) -> Option<Vec<&'a str>>) -> Result<String, usize> {
        let merged = diff3::merge(&lines(&self.base), &lines(&self.ours), &lines(&self.theirs), resolve)?.concat();
        Ok(if self.end == "\n" { merged } else { merged.replace('\n', self.end) })
    }
}

pub fn merge_lines(base: &str, ours: &str, theirs: &str) -> Result<String, String> {
    let sides = Sides::new(base, ours, theirs);
    let mut clash = None;
    sides
        .merge(|chunk| {
            clash.get_or_insert_with(|| (chunk.base.len(), first_line(chunk.ours), first_line(chunk.theirs)));
            None
        })
        .map_err(|at| {
            let (len, ours, theirs) = clash.unwrap_or_default();
            format!("{}; yours: {ours}, theirs: {theirs}", where_clash(at, len))
        })
}

pub fn merge_with_markers(base: &str, ours: &str, theirs: &str) -> String {
    let sides = Sides::new(base, ours, theirs);
    sides.merge(|chunk| Some(markers(chunk))).unwrap_or_default()
}

fn where_clash(at: usize, len: usize) -> String {
    match len {
        0 => format!("both sides added different lines after line {at}"),
        1 => format!("both sides changed line {} differently", at + 1),
        _ => format!("both sides changed lines {}-{} differently", at + 1, at + len),
    }
}

fn first_line(lines: &[&str]) -> String {
    match lines.iter().map(|line| line.trim()).find(|line| !line.is_empty()) {
        Some(line) => format!("{:?}", line.chars().take(60).collect::<String>()),
        None if lines.is_empty() => "removed them".into(),
        None => "blank lines".into(),
    }
}

fn markers<'a>(chunk: &Chunk<&'a str>) -> Vec<&'a str> {
    let mut out = vec!["<<<<<<< yours\n"];
    side(&mut out, chunk.ours);
    out.push("=======\n");
    side(&mut out, chunk.theirs);
    out.push(">>>>>>> incoming\n");
    out
}

fn side<'a>(out: &mut Vec<&'a str>, lines: &[&'a str]) {
    out.extend_from_slice(lines);
    if lines.last().is_some_and(|line| !line.ends_with('\n')) {
        out.push("\n");
    }
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

    fn markers(&self, base: &[u8], ours: &[u8], theirs: &[u8]) -> Option<Vec<u8>> {
        let (Ok(base), Ok(ours), Ok(theirs)) =
            (std::str::from_utf8(base), std::str::from_utf8(ours), std::str::from_utf8(theirs))
        else {
            return None;
        };
        Some(merge_with_markers(base, ours, theirs).into_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_across_line_endings_and_names_clashes() {
        let merged = merge_lines("a\r\nb\r\nc\r\n", "A\nb\nc\n", "a\r\nb\r\nC\r\n").unwrap();
        assert_eq!(merged, "A\nb\nC\n");
        assert_eq!(merge_lines("a\r\nb\r\nc\r\n", "a\r\nb\r\nC\r\n", "A\r\nb\r\nc\r\n").unwrap(), "A\r\nb\r\nC\r\n");
        let clash = merge_lines("a\nb\n", "a\nX\n", "a\nY\n").unwrap_err();
        assert_eq!(clash, r#"both sides changed line 2 differently; yours: "X", theirs: "Y""#);
        let marked = merge_with_markers("a\r\nb\r\n", "a\r\nX\r\n", "a\r\nY\r\n");
        assert_eq!(marked, "a\r\n<<<<<<< yours\r\nX\r\n=======\r\nY\r\n>>>>>>> incoming\r\n");
    }
}
