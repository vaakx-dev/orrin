use super::value::Json;
use crate::three::{self, both_changed};
use crate::{diff3, keyed};

fn merge_items(path: &str, base: &[Json], ours: &[Json], theirs: &[Json]) -> Result<Vec<Json>, String> {
    let pairwise = |chunk: &diff3::Chunk<Json>| {
        if chunk.ours.len() != chunk.base.len() || chunk.theirs.len() != chunk.base.len() {
            return None;
        }
        let triples = chunk.base.iter().zip(chunk.ours).zip(chunk.theirs);
        triples.map(|((b, o), t)| merge(path, b, o, t).ok()).collect()
    };
    diff3::merge(base, ours, theirs, pairwise).map_err(|at| both_changed(&format!("{path}[{at}]")))
}

pub fn merge(path: &str, base: &Json, ours: &Json, theirs: &Json) -> Result<Json, String> {
    match (base, ours, theirs) {
        (Json::Object(b), Json::Object(o), Json::Object(t)) => keyed::merge(b, o, t, |key, b, o, t| {
            let path = format!("{path}.{key}");
            three::merge(&path, b, o, t, |b, o, t| merge(&path, b, o, t))
        })
        .map(Json::Object),
        (Json::Array(b), Json::Array(o), Json::Array(t)) => merge_items(path, b, o, t).map(Json::Array),
        _ => {
            three::pick(Some(base), Some(ours), Some(theirs)).ok().flatten().cloned().ok_or_else(|| both_changed(path))
        }
    }
}
