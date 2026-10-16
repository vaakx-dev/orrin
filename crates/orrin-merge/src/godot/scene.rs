use crate::keyed::{self, Items};

const TAGS: &[&str] =
    &["gd_scene", "gd_resource", "ext_resource", "sub_resource", "node", "connection", "editable", "resource"];

const LOAD_STEPS: &str = " load_steps=";

pub const HEAD: &str = "head";

fn tag(line: &str) -> &str {
    line.strip_prefix('[').and_then(|rest| rest.split([' ', ']']).next()).unwrap_or_default()
}

fn is_header(line: &str) -> bool {
    line.trim_end().ends_with(']') && TAGS.contains(&tag(line))
}

fn attribute<'a>(line: &'a str, name: &str) -> &'a str {
    let Some(at) = line.find(&format!(" {name}=")) else {
        return "";
    };
    let value = &line[at + name.len() + 2..];
    match value.strip_prefix('"') {
        Some(quoted) => quoted.split('"').next(),
        None => value.split([' ', ']']).next(),
    }
    .unwrap_or_default()
}

fn key(line: &str) -> String {
    let attr = |name: &str| attribute(line, name);
    match tag(line) {
        "gd_scene" | "gd_resource" => HEAD.into(),
        "ext_resource" => format!("ext {}", attr("id")),
        "sub_resource" => format!("sub {}", attr("id")),
        "node" => format!("node {}/{}", attr("parent"), attr("name")),
        "connection" => format!("connection {} {} {} {}", attr("signal"), attr("from"), attr("to"), attr("method")),
        "editable" => format!("editable {}", attr("path")),
        _ => line.trim_end().to_string(),
    }
}

pub fn sections(text: &str) -> Items<String> {
    let mut sections: Vec<Vec<&str>> = Vec::new();
    for line in text.lines() {
        match sections.last_mut() {
            Some(section) if !is_header(line) => section.push(line),
            _ => sections.push(vec![line]),
        }
    }
    keyed::unique(sections.into_iter().map(|lines| (key(lines[0]), lines.join("\n").trim_end().to_string())))
}

pub fn render(sections: &Items<String>) -> String {
    let texts: Vec<&str> = sections.iter().map(|(_, text)| text.as_str()).collect();
    texts.join("\n\n") + "\n"
}

fn first_line(text: &str) -> &str {
    text.lines().next().unwrap_or_default()
}

pub fn has_load_steps(text: &str) -> bool {
    first_line(text).contains(LOAD_STEPS)
}

pub fn without_load_steps(text: &str) -> String {
    let Some(start) = first_line(text).find(LOAD_STEPS) else {
        return text.to_string();
    };
    let digits = &text[start + LOAD_STEPS.len()..];
    let end = text.len() - digits.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    format!("{}{}", &text[..start], &text[end..])
}

pub fn set_load_steps(sections: &mut Items<String>) {
    let resources = sections.iter().filter(|(key, _)| key.starts_with("ext ") || key.starts_with("sub ")).count();
    let Some((_, head)) = sections.iter_mut().find(|(key, _)| key == HEAD) else {
        return;
    };
    if let Some(at) = head.find([' ', ']']) {
        head.insert_str(at, &format!("{LOAD_STEPS}{}", resources + 1));
    }
}
