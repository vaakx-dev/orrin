use std::collections::BTreeMap;

pub type Values = BTreeMap<String, String>;

pub fn fill(text: &str, values: &Values) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let tail = &rest[start + 1..];
        let name = tail.find('}').map(|end| &tail[..end]).filter(|name| is_name(name));
        match name.and_then(|name| values.get(name).map(|value| (name, value))) {
            Some((name, value)) => {
                out.push_str(value);
                rest = &tail[name.len() + 1..];
            }
            None => {
                out.push('{');
                rest = tail;
            }
        }
    }
    out.push_str(rest);
    out
}

fn is_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || "_-:.".contains(c))
}

pub fn native(path: &str) -> String {
    let path = if cfg!(windows) { path.replace('/', "\\") } else { path.to_string() };
    match path.contains(' ') {
        true => format!("\"{path}\""),
        false => path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_known_names_and_leaves_the_rest() {
        let values: Values =
            [("in", "a.cpp"), ("pkg:godot-cpp", "/p/g")].map(|(k, v)| (k.to_string(), v.to_string())).into();
        assert_eq!(fill("cl {in} /I{pkg:godot-cpp}/include {x} {", &values), "cl a.cpp /I/p/g/include {x} {");
        assert_eq!(fill("${in}}", &values), "$a.cpp}");
    }
}
