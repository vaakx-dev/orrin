use orrin_core::api::TextEdit;
use orrin_core::{Error, Result};

pub fn is_binary(bytes: &[u8]) -> bool {
    bytes[..bytes.len().min(8000)].contains(&0) || std::str::from_utf8(bytes).is_err()
}

pub fn lines(text: &str, offset: Option<u32>, limit: Option<u32>) -> (String, u32) {
    let all: Vec<&str> = text.split_inclusive('\n').collect();
    let start = offset.unwrap_or(1).max(1) as usize - 1;
    let take = limit.map_or(usize::MAX, |l| l as usize);
    let picked: String = all.iter().skip(start).take(take).copied().collect();
    (picked, all.len() as u32)
}

pub fn apply(text: &str, edits: &[TextEdit]) -> Result<String> {
    let crlf = text.contains("\r\n");
    edits.iter().try_fold(text.to_string(), |current, edit| apply_one(&current, edit, crlf))
}

fn apply_one(text: &str, edit: &TextEdit, crlf: bool) -> Result<String> {
    if edit.old.is_empty() {
        return Err(Error::invalid("edit has empty old text"));
    }
    let (old, new) = if crlf && !text.contains(&edit.old) {
        (to_crlf(&edit.old), to_crlf(&edit.new))
    } else {
        (edit.old.clone(), edit.new.clone())
    };
    match text.matches(&old).count() {
        0 => Err(Error::conflict(format!("old text not found: {}", preview(&edit.old)))),
        1 => Ok(text.replacen(&old, &new, 1)),
        _ if edit.all => Ok(text.replace(&old, &new)),
        n => Err(Error::invalid(format!("old text found {n} times; add context or set all: {}", preview(&edit.old)))),
    }
}

fn to_crlf(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\n', "\r\n")
}

fn preview(text: &str) -> String {
    text.chars().take(80).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(old: &str, new: &str, all: bool) -> TextEdit {
        TextEdit { old: old.into(), new: new.into(), all }
    }

    #[test]
    fn applies_edits_and_rejects_ambiguity() {
        assert_eq!(apply("a b a", &[edit("b", "c", false)]).unwrap(), "a c a");
        assert!(apply("a b a", &[edit("a", "x", false)]).is_err());
        assert_eq!(apply("a b a", &[edit("a", "x", true)]).unwrap(), "x b x");
        assert_eq!(apply("one\r\ntwo\r\n", &[edit("one\ntwo", "1\n2", false)]).unwrap(), "1\r\n2\r\n");
        assert!(apply("abc", &[edit("zzz", "y", false)]).is_err());
    }

    #[test]
    fn slices_lines() {
        assert_eq!(lines("a\nb\nc", Some(2), Some(1)), ("b\n".to_string(), 3));
    }
}
