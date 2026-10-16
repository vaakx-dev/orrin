use crate::analyse::Sym;

pub struct Def {
    pub name: String,
    pub kind: &'static str,
    pub start: usize,
    pub end: usize,
    pub line: u32,
}

const TYPES: &[&str] = &["class", "struct", "impl", "trait", "interface"];
const BODIES: &[&str] = &["function", "method"];
const LOCALS: &[&str] = &["variable", "constant"];

pub fn nest(mut defs: Vec<Def>) -> Vec<Sym> {
    defs.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));
    defs.dedup_by(|a, b| a.start == b.start && a.name == b.name);
    let mut stack: Vec<Def> = Vec::new();
    let mut symbols = Vec::new();
    for def in defs {
        while stack.last().is_some_and(|open| open.end <= def.start) {
            stack.pop();
        }
        let parent = stack.last();
        let parent_kind = parent.map(|p| p.kind);
        if LOCALS.contains(&def.kind) && parent_kind.is_some_and(|k| BODIES.contains(&k)) {
            continue;
        }
        let kind = match parent_kind {
            Some(k) if def.kind == "function" && TYPES.contains(&k) => "method",
            _ => def.kind,
        };
        symbols.push(Sym {
            name: def.name.clone(),
            kind: kind.to_string(),
            line: def.line,
            container: parent.map(|p| p.name.clone()),
            depth: stack.len() as u32,
        });
        stack.push(Def { kind, ..def });
    }
    symbols
}
