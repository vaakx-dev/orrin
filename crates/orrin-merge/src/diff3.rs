use std::hash::Hash;

use similar::{Algorithm, DiffOp, capture_diff_slices};

pub struct Chunk<'a, T> {
    pub base: &'a [T],
    pub ours: &'a [T],
    pub theirs: &'a [T],
}

fn matches<T: Eq + Hash>(base: &[T], side: &[T]) -> Vec<Option<usize>> {
    let mut map = vec![None; base.len()];
    for op in capture_diff_slices(Algorithm::Myers, base, side) {
        if let DiffOp::Equal { old_index, new_index, len } = op {
            for k in 0..len {
                map[old_index + k] = Some(new_index + k);
            }
        }
    }
    map
}

fn next_stable(to_ours: &[Option<usize>], to_theirs: &[Option<usize>], from: usize) -> Option<(usize, usize, usize)> {
    (from..to_ours.len()).find_map(|b| Some((b, to_ours[b]?, to_theirs[b]?)))
}

fn trivial<T: Eq + Clone>(chunk: &Chunk<T>) -> Option<Vec<T>> {
    if chunk.ours == chunk.base {
        Some(chunk.theirs.to_vec())
    } else if chunk.theirs == chunk.base || chunk.theirs == chunk.ours {
        Some(chunk.ours.to_vec())
    } else {
        None
    }
}

pub fn merge<T: Eq + Hash + Clone>(
    base: &[T],
    ours: &[T],
    theirs: &[T],
    mut resolve: impl FnMut(&Chunk<T>) -> Option<Vec<T>>,
) -> Result<Vec<T>, usize> {
    let to_ours = matches(base, ours);
    let to_theirs = matches(base, theirs);
    let ends = (base.len(), ours.len(), theirs.len());
    let mut merged = Vec::new();
    let (mut b, mut o, mut t) = (0, 0, 0);
    while (b, o, t) != ends {
        let (nb, no, nt) = next_stable(&to_ours, &to_theirs, b).unwrap_or(ends);
        if (nb, no, nt) == (b, o, t) {
            merged.push(base[b].clone());
            (b, o, t) = (b + 1, o + 1, t + 1);
            continue;
        }
        let chunk = Chunk { base: &base[b..nb], ours: &ours[o..no], theirs: &theirs[t..nt] };
        merged.extend(trivial(&chunk).or_else(|| resolve(&chunk)).ok_or(b)?);
        (b, o, t) = (nb, no, nt);
    }
    Ok(merged)
}
