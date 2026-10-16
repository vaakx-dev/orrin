mod lang;
mod nest;
mod queries;

use orrin_core::model::OutlineItem;
use tree_sitter::{Parser, QueryCursor, StreamingIterator};

pub use lang::Lang;

use crate::analyse::Sym;
use nest::Def;

pub fn extract(lang: Lang, source: &str) -> Vec<Sym> {
    let Some(query) = lang.query() else { return Vec::new() };
    let mut parser = Parser::new();
    if parser.set_language(&lang.language()).is_err() {
        return Vec::new();
    }
    let Some(tree) = parser.parse(source, None) else { return Vec::new() };
    let bytes = source.as_bytes();
    let name_index = query.capture_index_for_name("name");
    let mut defs = Vec::new();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), bytes);
    while let Some(found) = matches.next() {
        let name = found.captures().iter().find(|c| Some(c.index) == name_index);
        let def = found.captures().iter().find(|c| Some(c.index) != name_index);
        let (Some(name), Some(def)) = (name, def) else { continue };
        let Ok(text) = name.node.utf8_text(bytes) else { continue };
        defs.push(Def {
            name: text.to_string(),
            kind: query.capture_names()[def.index as usize],
            start: def.node.start_byte(),
            end: def.node.end_byte(),
            line: def.node.start_position().row as u32 + 1,
        });
    }
    nest::nest(defs)
}

pub fn outline(symbols: &[Sym]) -> Vec<OutlineItem> {
    symbols
        .iter()
        .map(|s| OutlineItem { kind: s.kind.clone(), name: s.name.clone(), depth: s.depth, line: Some(s.line) })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(lang: Lang, source: &str) -> Vec<(String, String, Option<String>)> {
        extract(lang, source).into_iter().map(|s| (s.name, s.kind, s.container)).collect()
    }

    #[test]
    fn every_language_query_compiles() {
        for lang in [
            Lang::Rust,
            Lang::TypeScript,
            Lang::Tsx,
            Lang::JavaScript,
            Lang::Python,
            Lang::C,
            Lang::Cpp,
            Lang::GdScript,
        ] {
            assert!(lang.query().is_some(), "{lang:?}");
        }
    }

    #[test]
    fn finds_definitions_with_containers() {
        let rust =
            names(Lang::Rust, "struct Point;\nimpl Point {\n    fn len(&self) -> u32 { const X: u32 = 1; X }\n}\n");
        assert_eq!(rust[0], ("Point".into(), "struct".into(), None));
        assert_eq!(rust[2], ("len".into(), "method".into(), Some("Point".into())));
        assert_eq!(rust.len(), 3);

        let ts =
            names(Lang::TypeScript, "interface Shape {}\nclass Box { area() { return 1 } }\nconst run = () => 1\n");
        assert!(ts.contains(&("area".into(), "method".into(), Some("Box".into()))));
        assert!(ts.contains(&("run".into(), "function".into(), None)));

        let gd = names(Lang::GdScript, "extends Node\nsignal hit\nvar speed = 1\nfunc _ready():\n\tvar local = 2\n");
        assert_eq!(gd.iter().map(|s| s.0.as_str()).collect::<Vec<_>>(), ["hit", "speed", "_ready"]);
    }
}
