use std::collections::HashMap;

use orrin_core::api::{Search, SearchOutput};
use orrin_core::model::SearchHit;
use orrin_core::{Error, Result};
use regex::{Regex, RegexBuilder};

use super::paths::PathFilter;
use super::plan;
use crate::engine::{Engine, Visible, blocking};

const DEFAULT_LIMIT: usize = 100;
const MAX_LINE: usize = 400;

pub async fn search(engine: &Engine, input: Search) -> Result<SearchOutput> {
    let pattern = if input.regex { input.query.clone() } else { regex::escape(&input.query) };
    let regex = RegexBuilder::new(&pattern)
        .case_insensitive(!input.case_sensitive)
        .build()
        .map_err(|e| Error::invalid(e.to_string()))?;
    let plan = match input.regex {
        true => plan::regex(&input.query, input.case_sensitive),
        false => plan::literal(&input.query, input.case_sensitive),
    };
    let filter = PathFilter::new(input.paths.as_deref())?;
    let view = input.view.as_ref();
    let index = engine.open(&input.project, view).await?;
    let (clock, candidates) = {
        let state = index.read();
        let found: Vec<Visible> =
            state.candidates(view, &plan).into_iter().filter(|f| f.record.text && filter.allows(&f.path)).collect();
        (state.clock(view), found)
    };
    let mut fetched = HashMap::new();
    if let Some(view) = view {
        for file in candidates.iter().filter(|f| f.edited && !engine.hub().store.has(&f.hash)) {
            if let Some(bytes) = engine.read_view(&input.project, view, &file.path).await?.and_then(|f| f.bytes) {
                fetched.insert(file.path.clone(), bytes);
            }
        }
    }
    let limit = input.limit.map(|l| l as usize).unwrap_or(DEFAULT_LIMIT);
    let store = engine.hub().store.clone();
    let mut hits = blocking(move || {
        let mut hits = Vec::new();
        for file in candidates {
            let bytes = match fetched.remove(&file.path) {
                Some(bytes) => bytes,
                None => match store.get(&file.hash) {
                    Ok(bytes) => bytes,
                    Err(_) => continue,
                },
            };
            scan(&regex, &file.path, &bytes, limit + 1 - hits.len(), &mut hits);
            if hits.len() > limit {
                break;
            }
        }
        hits
    })
    .await?;
    let truncated = hits.len() > limit;
    hits.truncate(limit);
    Ok(SearchOutput { clock, hits, truncated })
}

fn scan(regex: &Regex, path: &str, bytes: &[u8], room: usize, hits: &mut Vec<SearchHit>) {
    let text = String::from_utf8_lossy(bytes);
    let matches = text.lines().enumerate().filter_map(|(n, line)| regex.find(line).map(|m| (n, line, m.start())));
    for (number, line, start) in matches.take(room) {
        hits.push(SearchHit {
            path: path.to_string(),
            line: number as u32 + 1,
            column: line[..start].chars().count() as u32 + 1,
            text: line.chars().take(MAX_LINE).collect(),
        });
    }
}
