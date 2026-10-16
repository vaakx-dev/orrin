pub enum Side {
    Ours,
    Theirs,
}

pub struct BothChanged;

pub fn both_changed(name: &str) -> String {
    format!("{name} changed on both sides")
}

pub fn winner<T: PartialEq + ?Sized>(base: Option<&T>, ours: Option<&T>, theirs: Option<&T>) -> Option<Side> {
    if ours == base {
        Some(Side::Theirs)
    } else if theirs == base || theirs == ours {
        Some(Side::Ours)
    } else {
        None
    }
}

pub fn pick<'a, T: PartialEq + ?Sized>(
    base: Option<&'a T>,
    ours: Option<&'a T>,
    theirs: Option<&'a T>,
) -> Result<Option<&'a T>, BothChanged> {
    match winner(base, ours, theirs) {
        Some(Side::Ours) => Ok(ours),
        Some(Side::Theirs) => Ok(theirs),
        None => Err(BothChanged),
    }
}

pub fn merge<T: PartialEq + Clone>(
    name: &str,
    base: Option<&T>,
    ours: Option<&T>,
    theirs: Option<&T>,
    merge_all: impl FnOnce(&T, &T, &T) -> Result<T, String>,
) -> Result<Option<T>, String> {
    if let Ok(picked) = pick(base, ours, theirs) {
        return Ok(picked.cloned());
    }
    match (base, ours, theirs) {
        (Some(base), Some(ours), Some(theirs)) => merge_all(base, ours, theirs).map(Some),
        _ => Err(both_changed(name)),
    }
}
