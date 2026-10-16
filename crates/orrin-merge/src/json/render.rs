use serde::Serialize;
use serde_json::Serializer;
use serde_json::ser::PrettyFormatter;

use super::value::Json;

fn indent_of(text: &str) -> Option<&str> {
    text.lines().skip(1).find_map(|line| {
        let content = line.trim_start();
        (content.len() < line.len()).then(|| &line[..line.len() - content.len()])
    })
}

pub fn render(json: &Json, like: &str) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let written = match indent_of(like) {
        Some(indent) => {
            let formatter = PrettyFormatter::with_indent(indent.as_bytes());
            json.serialize(&mut Serializer::with_formatter(&mut out, formatter))
        }
        None => json.serialize(&mut Serializer::new(&mut out)),
    };
    written.map_err(|error| error.to_string())?;
    if like.ends_with('\n') {
        out.push(b'\n');
    }
    if like.contains("\r\n") {
        out = String::from_utf8_lossy(&out).replace('\n', "\r\n").into_bytes();
    }
    Ok(out)
}
