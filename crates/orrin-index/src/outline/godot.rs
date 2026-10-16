use orrin_core::model::OutlineItem;

use super::item;

pub fn outline(source: &str) -> Vec<OutlineItem> {
    source
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let header = line.trim().strip_prefix("[node ")?.strip_suffix(']')?;
            let name = attribute(header, "name")?;
            let depth = match attribute(header, "parent") {
                None => 0,
                Some(".") => 1,
                Some(parent) => parent.split('/').count() as u32 + 1,
            };
            let kind = attribute(header, "type").unwrap_or("instance");
            Some(item(kind, name, depth, Some(i as u32 + 1)))
        })
        .collect()
}

fn attribute<'a>(header: &'a str, key: &str) -> Option<&'a str> {
    let pattern = format!("{key}=\"");
    let mut search = header;
    loop {
        let at = search.find(&pattern)?;
        let boundary = at == 0 || search.as_bytes()[at - 1] == b' ';
        let rest = &search[at + pattern.len()..];
        if boundary {
            return rest.find('"').map(|end| &rest[..end]);
        }
        search = rest;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_tree_depths() {
        let scene = "[gd_scene format=3]\n[node name=\"Main\" type=\"Node2D\"]\n[node name=\"Player\" parent=\".\" instance=ExtResource(\"1\")]\n[node name=\"Sprite\" type=\"Sprite2D\" parent=\"Player\"]\n";
        let items: Vec<(String, String, u32)> = outline(scene).into_iter().map(|i| (i.name, i.kind, i.depth)).collect();
        assert_eq!(
            items,
            [
                ("Main".into(), "Node2D".into(), 0),
                ("Player".into(), "instance".into(), 1),
                ("Sprite".into(), "Sprite2D".into(), 2)
            ]
        );
    }
}
