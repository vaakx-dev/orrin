use orrin_core::Result;
use orrin_core::api::{Where, WhereOutput};
use orrin_core::model::Symbol;

use crate::engine::Engine;

const DEFAULT_LIMIT: usize = 50;

pub async fn locate(engine: &Engine, input: Where) -> Result<WhereOutput> {
    let wants = |kind: &str| input.kind.as_deref().is_none_or(|k| k == kind);
    let mut symbols = Vec::new();
    if wants("tool")
        && let Ok(Some(tool)) = engine.hub().facts.tool(&input.name).await
    {
        symbols.push(Symbol {
            name: tool.name,
            kind: "tool".into(),
            path: tool.path,
            line: 0,
            container: tool.version,
        });
    }
    let view = input.view.as_ref();
    let index = engine.open(&input.project, view).await?;
    let state = index.read();
    let name = input.name.as_str();
    let lower = name.to_lowercase();
    if wants("file") {
        state.each(view, |path, _| {
            let file = path.rsplit('/').next().unwrap_or(path);
            if path == name || file.to_lowercase() == lower || path.ends_with(&format!("/{name}")) {
                symbols.push(Symbol {
                    name: file.into(),
                    kind: "file".into(),
                    path: path.into(),
                    line: 0,
                    container: None,
                });
            }
        });
    }
    let mut code = Vec::new();
    let mut loose = Vec::new();
    state.each(view, |path, file| {
        for sym in file.record.symbols.iter().filter(|s| wants(&s.kind)) {
            let found = || Symbol {
                name: sym.name.clone(),
                kind: sym.kind.clone(),
                path: path.into(),
                line: sym.line,
                container: sym.container.clone(),
            };
            if sym.name == name {
                code.push(found());
            } else if sym.name.to_lowercase() == lower {
                loose.push(found());
            }
        }
    });
    symbols.extend(if code.is_empty() { loose } else { code });
    symbols.truncate(input.limit.map(|l| l as usize).unwrap_or(DEFAULT_LIMIT));
    Ok(WhereOutput { clock: state.clock(view), symbols })
}
