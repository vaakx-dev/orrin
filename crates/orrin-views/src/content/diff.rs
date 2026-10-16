use orrin_core::api::FileDiff;
use orrin_core::model::FileStatus;
use similar::TextDiff;

use crate::content::text::is_binary;

pub fn file_diff(path: &str, status: FileStatus, old: &[u8], new: &[u8]) -> FileDiff {
    let binary = is_binary(old) || is_binary(new);
    let patch = (!binary).then(|| unified(path, old, new));
    FileDiff { path: path.to_string(), status, binary, patch }
}

fn unified(path: &str, old: &[u8], new: &[u8]) -> String {
    let old = String::from_utf8_lossy(old);
    let new = String::from_utf8_lossy(new);
    let diff = TextDiff::from_lines(old.as_ref(), new.as_ref());
    diff.unified_diff().context_radius(3).header(&format!("a/{path}"), &format!("b/{path}")).to_string()
}

pub fn under(path: &str, filter: Option<&str>) -> bool {
    filter.is_none_or(|f| path == f || path.starts_with(&format!("{f}/")))
}
