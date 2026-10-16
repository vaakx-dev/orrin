use std::collections::HashMap;

use orrin_core::api::{Search, SearchOutput};
use orrin_core::model::SearchHit;
use orrin_core::{Error, Result};
use regex::{Regex, RegexBuilder};

use super::paths::PathFilter;
use super::plan;
use crate::analyse::Kind;
use crate::documents;
use crate::engine::{Engine, Visible, blocking};
use crate::records::key;

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
        let searchable = |f: &Visible| (f.record.text || f.record.document) && filter.allows(&f.path);
        let found: Vec<Visible> = state.candidates(view, &plan).into_iter().filter(searchable).collect();
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
    let records = engine.records();
    let mut hits = blocking(move || {
        let mut hits = Vec::new();
        for file in candidates {
            let mut load = || fetched.remove(&file.path).or_else(|| store.get(&file.hash).ok());
            let room = limit + 1 - hits.len();
            match Kind::of(&file.path) {
                kind @ Kind::Document(format) => {
                    let blocks = records
                        .blocks(&key(kind, &file.hash))
                        .or_else(|| load().map(|bytes| documents::extract(format, &bytes).blocks));
                    let Some(blocks) = blocks else { continue };
                    scan(&regex, &file.path, documents::lines(&blocks), room, &mut hits);
                }
                _ => {
                    let Some(bytes) = load() else { continue };
                    let text = String::from_utf8_lossy(&bytes);
                    scan(&regex, &file.path, text.lines().map(|line| (None, line)), room, &mut hits);
                }
            }
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

fn scan<'a>(
    regex: &Regex,
    path: &str,
    lines: impl Iterator<Item = (Option<&'a str>, &'a str)>,
    room: usize,
    hits: &mut Vec<SearchHit>,
) {
    let matches = lines.enumerate().filter_map(|(n, (anchor, line))| regex.find(line).map(|m| (n, anchor, line, m)));
    for (number, anchor, line, found) in matches.take(room) {
        hits.push(SearchHit {
            path: path.to_string(),
            line: number as u32 + 1,
            column: line[..found.start()].chars().count() as u32 + 1,
            text: line.chars().take(MAX_LINE).collect(),
            anchor: anchor.map(str::to_string),
        });
    }
}
