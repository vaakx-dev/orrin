use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use orrin_core::model::{Artifact, Job, LogLine};
use orrin_core::{Error, JobId, Result};
use rusqlite::{Connection, ToSql, params};
use serde::Serialize;
use serde::de::DeserializeOwned;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS build_jobs (
    id TEXT PRIMARY KEY,
    created_at INTEGER NOT NULL,
    job TEXT NOT NULL,
    artifacts TEXT NOT NULL DEFAULT '[]',
    args TEXT NOT NULL DEFAULT '[]',
    remote TEXT
);
CREATE TABLE IF NOT EXISTS build_logs (
    job TEXT NOT NULL,
    n INTEGER NOT NULL,
    line TEXT NOT NULL,
    PRIMARY KEY (job, n)
);";

const PRUNE: &str = "
DELETE FROM build_logs WHERE job IN (SELECT id FROM build_jobs ORDER BY created_at DESC LIMIT -1 OFFSET ?1);
DELETE FROM build_jobs WHERE id IN (SELECT id FROM build_jobs ORDER BY created_at DESC LIMIT -1 OFFSET ?1);";

const FLUSH_AT: usize = 256;

pub struct Saved {
    pub job: Job,
    pub artifacts: Vec<Artifact>,
    pub args: Vec<String>,
    pub remote: Option<JobId>,
    pub logs: Vec<LogLine>,
}

pub struct Journal {
    db: Mutex<Connection>,
    pending: Mutex<Vec<(JobId, LogLine)>>,
}

impl Journal {
    pub fn open(path: &Path) -> Result<Self> {
        let db = Connection::open(path).map_err(sql)?;
        db.busy_timeout(Duration::from_secs(5)).map_err(sql)?;
        db.pragma_update(None, "journal_mode", "WAL").map_err(sql)?;
        db.pragma_update(None, "synchronous", "NORMAL").map_err(sql)?;
        db.execute_batch(SCHEMA).map_err(sql)?;
        Ok(Self { db: Mutex::new(db), pending: Mutex::default() })
    }

    pub fn job(&self, job: &Job) {
        self.flush();
        let upsert = "INSERT INTO build_jobs (id, created_at, job) VALUES (?1, ?2, ?3)
                      ON CONFLICT(id) DO UPDATE SET job = excluded.job";
        self.write(upsert, params![job.id.as_str(), job.created_at as i64, json(job)]);
    }

    pub fn artifacts(&self, id: &JobId, artifacts: &[Artifact]) {
        self.write("UPDATE build_jobs SET artifacts = ?2 WHERE id = ?1", params![id.as_str(), json(&artifacts)]);
    }

    pub fn args(&self, id: &JobId, args: &[String]) {
        self.write("UPDATE build_jobs SET args = ?2 WHERE id = ?1", params![id.as_str(), json(&args)]);
    }

    pub fn remote(&self, id: &JobId, remote: &JobId) {
        self.write("UPDATE build_jobs SET remote = ?2 WHERE id = ?1", params![id.as_str(), remote.as_str()]);
    }

    pub fn log(&self, id: &JobId, line: &LogLine) {
        let full = {
            let mut pending = self.pending.lock().unwrap();
            pending.push((id.clone(), line.clone()));
            pending.len() >= FLUSH_AT
        };
        if full {
            self.flush();
        }
    }

    pub fn flush(&self) {
        let lines = std::mem::take(&mut *self.pending.lock().unwrap());
        if lines.is_empty() {
            return;
        }
        let mut db = self.db.lock().unwrap();
        let written = (|| {
            let tx = db.transaction()?;
            {
                let mut insert =
                    tx.prepare_cached("INSERT OR REPLACE INTO build_logs (job, n, line) VALUES (?1, ?2, ?3)")?;
                for (id, line) in &lines {
                    insert.execute(params![id.as_str(), line.n as i64, json(line)])?;
                }
            }
            tx.commit()
        })();
        if let Err(err) = written {
            tracing::warn!("could not save job logs: {err}");
        }
    }

    pub fn load(&self, keep: usize) -> Result<Vec<Saved>> {
        let db = self.db.lock().unwrap();
        for statement in PRUNE.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            db.execute(statement, params![keep as i64]).map_err(sql)?;
        }
        let mut jobs =
            db.prepare("SELECT job, artifacts, args, remote FROM build_jobs ORDER BY created_at").map_err(sql)?;
        let rows = jobs
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get(3)?))
            })
            .map_err(sql)?;
        let mut logs = db.prepare("SELECT line FROM build_logs WHERE job = ?1 ORDER BY n").map_err(sql)?;
        let mut saved = Vec::new();
        for row in rows {
            let (job, artifacts, args, remote): (String, String, String, Option<String>) = row.map_err(sql)?;
            let job: Job = parse(&job)?;
            let lines = logs
                .query_map([job.id.as_str()], |row| row.get::<_, String>(0))
                .map_err(sql)?
                .filter_map(|line| parse(&line.ok()?).ok())
                .collect();
            saved.push(Saved {
                job,
                artifacts: parse(&artifacts)?,
                args: parse(&args)?,
                remote: remote.as_deref().map(JobId::from),
                logs: lines,
            });
        }
        Ok(saved)
    }

    fn write(&self, statement: &str, params: &[&dyn ToSql]) {
        if let Err(err) = self.db.lock().unwrap().execute(statement, params) {
            tracing::warn!("could not save a job: {err}");
        }
    }
}

fn json(value: &impl Serialize) -> String {
    serde_json::to_string(value).unwrap_or_default()
}

fn parse<T: DeserializeOwned>(text: &str) -> Result<T> {
    Ok(serde_json::from_str(text)?)
}

fn sql(err: rusqlite::Error) -> Error {
    Error::internal(err.to_string())
}

#[cfg(test)]
mod tests {
    use orrin_core::model::{JobKind, JobState, Stream};
    use orrin_core::{PcId, ProjectId, SnapshotId};

    use super::*;

    fn job() -> Job {
        Job {
            id: JobId::new(),
            project: ProjectId::new(),
            kind: JobKind::Build,
            target: "app".into(),
            snapshot: SnapshotId::of(b"snap"),
            pc: PcId::new(),
            state: JobState::Running,
            cached: false,
            exit_code: None,
            error: None,
            created_at: 1,
            started_at: None,
            finished_at: None,
            result: None,
        }
    }

    #[test]
    fn jobs_and_logs_survive_a_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("orrin.db");
        let mut running = job();
        {
            let journal = Journal::open(&path).unwrap();
            journal.job(&running);
            journal.args(&running.id, &["--release".into()]);
            journal.log(&running.id, &LogLine { n: 1, stream: Stream::Stdout, text: "hi".into(), at: 2 });
            running.state = JobState::Succeeded;
            journal.job(&running);
        }
        let saved = Journal::open(&path).unwrap().load(10).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].job, running);
        assert_eq!(saved[0].args, ["--release"]);
        assert_eq!(saved[0].logs[0].text, "hi");
    }
}
