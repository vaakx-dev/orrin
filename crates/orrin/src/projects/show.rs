use std::collections::HashMap;

use orrin_core::PcId;
use orrin_core::api::{LandOutput, ProjectOpened, ProjectRemoved};
use orrin_core::model::{Project, View, ViewKind, ViewState};

use crate::table::{Table, ago, short};

pub fn projects(projects: &[Project], pcs: &HashMap<PcId, String>) -> String {
    let mut table = Table::new(&["NAME", "ID", "HOME", "MAIN"]);
    for p in projects {
        let home = pcs.get(&p.home_pc).cloned().unwrap_or_else(|| p.home_pc.to_string());
        table.row(vec![p.name.clone(), p.id.to_string(), home, short(p.main.as_str())]);
    }
    table.render("No projects yet. Add one with: orrin import <folder>")
}

pub fn opened(out: &ProjectOpened) -> String {
    let place = out.view.as_ref().map(|v| format!(" at {}", v.path.display())).unwrap_or_default();
    format!("Imported {} ({}): {} files{place}", out.project.name, out.project.id, out.files)
}

pub fn removed(out: &ProjectRemoved) -> String {
    let mut lines = vec![format!("Removed {} ({}) and {} views", out.project.name, out.project.id, out.views)];
    match out.deleted_folders.as_slice() {
        [] => lines.push("Your folder on disk was left alone.".to_string()),
        folders => lines.extend(folders.iter().map(|f| format!("  deleted folder {}", f.display()))),
    }
    lines.join("\n")
}

pub fn views(views: &[View]) -> String {
    let mut table = Table::new(&["ID", "OWNER", "KIND", "STATE", "CREATED", "PATH"]);
    for v in views {
        table.row(vec![
            v.id.to_string(),
            v.owner.clone(),
            kind(v.kind).into(),
            state(v.state).into(),
            ago(v.created_at),
            v.path.display().to_string(),
        ]);
    }
    table.render("No views.")
}

pub fn view(view: &View) -> String {
    format!("{}  {}", view.id, view.path.display())
}

pub fn landed(out: &LandOutput) -> String {
    let mut lines = Vec::new();
    match (&out.change, out.landed) {
        (Some(change), true) => lines.push(format!(
            "Landed {}: {} files, main is now {}",
            change.id,
            change.files.len(),
            short(out.main.as_str())
        )),
        (None, true) => lines.push("Landed into the view.".to_string()),
        (_, false) => lines.push(format!("Not landed: {} conflicts", out.conflicts.len())),
    }
    lines.extend(out.conflicts.iter().map(|c| format!("  conflict  {}  {}", c.path, c.reason)));
    lines.extend(out.skipped.iter().map(|path| format!("  skipped   {path} (open or locked)")));
    lines.join("\n")
}

fn kind(kind: ViewKind) -> &'static str {
    match kind {
        ViewKind::User => "user",
        ViewKind::Worker => "worker",
    }
}

fn state(state: ViewState) -> &'static str {
    match state {
        ViewState::Open => "open",
        ViewState::Landed => "landed",
        ViewState::Dropped => "dropped",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orrin_core::Hash;
    use orrin_core::model::Conflict;

    #[test]
    fn conflicts_are_listed_one_per_line() {
        let out = LandOutput {
            landed: false,
            change: None,
            main: Hash::of(b""),
            conflicts: vec![Conflict {
                path: "art/logo.png".into(),
                base: None,
                ours: None,
                theirs: None,
                reason: "binary file changed on both sides".into(),
            }],
            skipped: vec!["notes.docx".into()],
        };
        assert_eq!(
            landed(&out),
            "Not landed: 1 conflicts\n  conflict  art/logo.png  binary file changed on both sides\n  skipped   notes.docx (open or locked)"
        );
    }
}
