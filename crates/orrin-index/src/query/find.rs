use globset::{GlobBuilder, GlobMatcher};
use orrin_core::api::{Find, FindOutput};
use orrin_core::model::FileHit;
use orrin_core::{Error, Result};

use crate::engine::Engine;

const DEFAULT_LIMIT: usize = 50;

pub async fn find(engine: &Engine, input: Find) -> Result<FindOutput> {
    let matcher = Matcher::new(&input.pattern)?;
    let index = engine.open(&input.project, input.view.as_ref()).await?;
    let state = index.read();
    let mut hits: Vec<(i64, FileHit)> = Vec::new();
    state.each(input.view.as_ref(), |path, file| {
        if let Some(score) = matcher.score(path) {
            hits.push((score, FileHit { path: path.to_string(), size: file.size, heading: None, anchor: None }));
        }
        if !file.record.document {
            return;
        }
        for item in &file.record.outline {
            if let Some(score) = matcher.heading(&item.name) {
                let hit = FileHit {
                    path: path.to_string(),
                    size: file.size,
                    heading: Some(item.name.clone()),
                    anchor: item.anchor.clone(),
                };
                hits.push((score, hit));
            }
        }
    });
    hits.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.path.cmp(&b.1.path)));
    let limit = input.limit.map(|l| l as usize).unwrap_or(DEFAULT_LIMIT);
    let truncated = hits.len() > limit;
    let files = hits.into_iter().take(limit).map(|(_, hit)| hit).collect();
    Ok(FindOutput { clock: state.clock(input.view.as_ref()), files, truncated })
}

enum Matcher {
    All,
    Glob(GlobMatcher),
    Fuzzy(Vec<Vec<char>>),
}

impl Matcher {
    fn new(pattern: &str) -> Result<Self> {
        let pattern = pattern.trim();
        if pattern.is_empty() {
            return Ok(Matcher::All);
        }
        if pattern.contains(['*', '?', '[', '{']) {
            let glob =
                GlobBuilder::new(pattern).case_insensitive(true).build().map_err(|e| Error::invalid(e.to_string()))?;
            return Ok(Matcher::Glob(glob.compile_matcher()));
        }
        Ok(Matcher::Fuzzy(pattern.split_whitespace().map(|t| t.to_lowercase().chars().collect()).collect()))
    }

    fn score(&self, path: &str) -> Option<i64> {
        match self {
            Matcher::All => Some(0),
            Matcher::Glob(glob) => glob.is_match(path).then(|| -(path.len() as i64)),
            Matcher::Fuzzy(tokens) => {
                let lower: Vec<char> = path.to_lowercase().chars().collect();
                tokens.iter().map(|t| fuzzy(t, &lower)).sum()
            }
        }
    }

    fn heading(&self, name: &str) -> Option<i64> {
        let Matcher::Fuzzy(tokens) = self else { return None };
        let lower: Vec<char> = name.to_lowercase().chars().collect();
        let found = |t: &Vec<char>| lower.windows(t.len()).any(|w| w == t.as_slice());
        if !tokens.iter().all(found) {
            return None;
        }
        let score: i64 = tokens.iter().filter_map(|t| subsequence(t, &lower)).sum();
        Some(score + 50 - lower.len() as i64 / 4)
    }
}

fn fuzzy(token: &[char], path: &[char]) -> Option<i64> {
    let name_start = path.iter().rposition(|c| *c == '/').map(|i| i + 1).unwrap_or(0);
    let name = &path[name_start..];
    let mut score = match subsequence(token, name) {
        Some(score) => score + 50,
        None => subsequence(token, path)?,
    };
    if name.starts_with(token) {
        score += 100;
    }
    let stem_end = name.iter().position(|c| *c == '.').unwrap_or(name.len());
    if name == token || &name[..stem_end] == token {
        score += 200;
    }
    Some(score - path.len() as i64 / 4)
}

fn subsequence(token: &[char], text: &[char]) -> Option<i64> {
    let mut score = 0;
    let mut at = 0;
    let mut previous: Option<usize> = None;
    for wanted in token {
        let found = text[at..].iter().position(|c| c == wanted)? + at;
        let boundary = found == 0 || matches!(text[found - 1], '/' | '_' | '-' | '.' | ' ');
        score += if boundary { 16 } else { 0 };
        score += match previous {
            Some(p) if p + 1 == found => 8,
            Some(p) => -((found - p - 1).min(10) as i64),
            None => 0,
        };
        previous = Some(found);
        at = found + 1;
    }
    Some(score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_names_beat_scattered_matches() {
        let matcher = Matcher::new("main").unwrap();
        let exact = matcher.score("src/main.rs").unwrap();
        let scattered = matcher.score("src/my_animation.rs").unwrap();
        assert!(exact > scattered);
        assert!(matcher.score("src/lib.rs").is_none());
        assert!(Matcher::new("*.RS").unwrap().score("src/lib.rs").is_some());
    }
}
