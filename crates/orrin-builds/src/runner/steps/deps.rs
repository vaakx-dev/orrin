use std::path::Path;

const MSVC_NOTE: &str = "Note: including file:";

pub fn msvc_line(line: &str) -> Option<&str> {
    line.strip_prefix(MSVC_NOTE).map(str::trim).filter(|path| !path.is_empty())
}

pub fn depfile(text: &str) -> Vec<String> {
    let joined = text.replace("\\\r\n", " ").replace("\\\n", " ");
    let mut deps = Vec::new();
    for line in joined.lines() {
        let Some(at) = separator(line) else { continue };
        deps.extend(words(&line[at + 1..]));
    }
    deps
}

pub fn relative(dir: &Path, path: &str) -> String {
    let path = path.replace('\\', "/");
    let root = dir.to_string_lossy().replace('\\', "/");
    let root = root.trim_end_matches('/');
    let inside = path.get(..root.len()).is_some_and(|head| head.eq_ignore_ascii_case(root))
        && path[root.len()..].starts_with('/');
    match inside {
        true => path[root.len() + 1..].to_string(),
        false => path.strip_prefix("./").unwrap_or(&path).to_string(),
    }
}

fn separator(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    (0..bytes.len()).find(|&i| bytes[i] == b':' && bytes.get(i + 1).is_none_or(|next| next.is_ascii_whitespace()))
}

fn words(text: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&' ') => word.push(chars.next().unwrap()),
            c if c.is_whitespace() => {
                if !word.is_empty() {
                    words.push(std::mem::take(&mut word));
                }
            }
            c => word.push(c),
        }
    }
    if !word.is_empty() {
        words.push(word);
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_msvc_notes_and_gcc_depfiles() {
        assert_eq!(msvc_line("Note: including file:   C:\\w\\0\\game\\a.h"), Some("C:\\w\\0\\game\\a.h"));
        assert_eq!(msvc_line("game.cpp"), None);
        let text = "C:/w/out/a.o: src/a.cpp src/a.h \\\n  C:/sdk/my\\ dir/b.h\nsrc/a.h:\n";
        assert_eq!(depfile(text), ["src/a.cpp", "src/a.h", "C:/sdk/my dir/b.h"]);
    }

    #[test]
    fn makes_paths_inside_the_folder_relative() {
        let dir = Path::new("C:\\warm\\0");
        assert_eq!(relative(dir, "c:\\WARM\\0\\game\\a.h"), "game/a.h");
        assert_eq!(relative(dir, "C:\\warm\\01\\a.h"), "C:/warm/01/a.h");
        assert_eq!(relative(dir, "./src/a.h"), "src/a.h");
    }
}
