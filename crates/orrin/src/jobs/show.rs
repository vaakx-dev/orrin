use std::collections::HashMap;

use orrin_core::model::{Job, JobKind, JobState, LogLine, Stream};
use orrin_core::{PcId, ProjectId};

use crate::table::{Table, ago};

pub struct Names {
    pub projects: HashMap<ProjectId, String>,
    pub pcs: HashMap<PcId, String>,
}

pub fn jobs(jobs: &[Job], names: &Names) -> String {
    let mut table = Table::new(&["ID", "KIND", "PROJECT", "RECIPE", "STATE", "PC", "STARTED"]);
    for job in jobs.iter().rev() {
        table.row(vec![
            job.id.to_string(),
            kind(job.kind).into(),
            names.projects.get(&job.project).cloned().unwrap_or_else(|| job.project.to_string()),
            job.recipe.clone(),
            state(job),
            names.pcs.get(&job.pc).cloned().unwrap_or_else(|| job.pc.to_string()),
            ago(job.started_at.unwrap_or(job.created_at)),
        ]);
    }
    table.render("No jobs.")
}

pub fn job(job: &Job) -> String {
    format!("{}  {} {}  {}", job.id, kind(job.kind), job.recipe, state(job))
}

pub fn state(job: &Job) -> String {
    let word = match job.state {
        JobState::Queued => "queued",
        JobState::Running => "running",
        JobState::Succeeded => "ok",
        JobState::Failed => "failed",
        JobState::Cancelled => "cancelled",
    };
    let mut text = word.to_string();
    if let (JobState::Failed, Some(code)) = (job.state, job.exit_code) {
        text.push_str(&format!(" (exit {code})"));
    }
    if job.cached {
        text.push_str(", cached");
    }
    if let (Some(start), Some(end)) = (job.started_at, job.finished_at) {
        text.push_str(&format!(" in {:.1}s", end.saturating_sub(start) as f64 / 1000.0));
    }
    text
}

pub fn line(line: &LogLine) -> String {
    match line.stream {
        Stream::System => format!("» {}", line.text),
        Stream::Stdout | Stream::Stderr => line.text.clone(),
    }
}

pub fn kind(kind: JobKind) -> &'static str {
    match kind {
        JobKind::Build => "build",
        JobKind::Run => "run",
        JobKind::Screenshot => "shot",
        JobKind::Test => "test",
        JobKind::Export => "export",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orrin_core::Hash;

    #[test]
    fn job_line_shows_outcome_and_time() {
        let job = Job {
            id: "j_1".into(),
            project: "p_1".into(),
            kind: JobKind::Build,
            recipe: "game".into(),
            snapshot: Hash::of(b""),
            pc: "pc_1".into(),
            state: JobState::Failed,
            cached: false,
            exit_code: Some(2),
            error: None,
            created_at: 0,
            started_at: Some(1000),
            finished_at: Some(3500),
            result: None,
        };
        assert_eq!(super::job(&job), "j_1  build game  failed (exit 2) in 2.5s");
    }
}
