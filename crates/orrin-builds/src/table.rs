use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::Duration;

use orrin_core::api::{Event, JobLogsOutput};
use orrin_core::model::{Artifact, Job, JobState, LogLine, Stream};
use orrin_core::{Error, Events, Hash, JobId, Result, now_ms};
use tokio::sync::{Notify, watch};

use crate::journal::{Journal, Saved};

const KEEP_LINES: usize = 20_000;

struct Entry {
    job: Job,
    logs: VecDeque<LogLine>,
    next_line: u64,
    artifacts: Vec<Artifact>,
    cancel: watch::Sender<bool>,
}

#[derive(Default)]
struct Inner {
    entries: HashMap<JobId, Entry>,
    running: HashMap<Hash, JobId>,
}

pub enum Claim {
    Fresh(Job, watch::Receiver<bool>),
    Shared(Job),
}

pub struct Table {
    inner: Mutex<Inner>,
    changed: Notify,
    events: Events,
    journal: Journal,
}

impl Table {
    pub fn new(events: Events, journal: Journal) -> Self {
        Self { inner: Mutex::default(), changed: Notify::new(), events, journal }
    }

    pub fn journal(&self) -> &Journal {
        &self.journal
    }

    pub fn claim(&self, job: Job, key: &Hash) -> Claim {
        let mut inner = self.inner.lock().unwrap();
        if let Some(existing) = inner.running.get(key).and_then(|id| inner.entries.get(id)) {
            return Claim::Shared(existing.job.clone());
        }
        inner.running.insert(key.clone(), job.id.clone());
        let cancelled = Self::add(&mut inner, job.clone(), Vec::new(), Vec::new());
        drop(inner);
        self.added(&job);
        Claim::Fresh(job, cancelled)
    }

    pub fn insert(&self, job: Job) -> watch::Receiver<bool> {
        let cancelled = Self::add(&mut self.inner.lock().unwrap(), job.clone(), Vec::new(), Vec::new());
        self.added(&job);
        cancelled
    }

    pub fn restore(&self, saved: Saved) -> watch::Receiver<bool> {
        Self::add(&mut self.inner.lock().unwrap(), saved.job, saved.logs, saved.artifacts)
    }

    fn added(&self, job: &Job) {
        self.journal.job(job);
        self.events.emit(Event::Job { job: job.clone() });
    }

    fn add(inner: &mut Inner, job: Job, logs: Vec<LogLine>, artifacts: Vec<Artifact>) -> watch::Receiver<bool> {
        let (cancel, cancelled) = watch::channel(false);
        let id = job.id.clone();
        let next_line = logs.last().map_or(0, |line| line.n);
        let logs = logs.into_iter().rev().take(KEEP_LINES).rev().collect();
        inner.entries.insert(id, Entry { job, logs, next_line, artifacts, cancel });
        cancelled
    }

    pub fn release(&self, key: &Hash, id: &JobId) {
        let mut inner = self.inner.lock().unwrap();
        if inner.running.get(key) == Some(id) {
            inner.running.remove(key);
        }
    }

    pub fn update(&self, id: &JobId, change: impl FnOnce(&mut Job)) -> Option<Job> {
        let (job, changed) = {
            let mut inner = self.inner.lock().unwrap();
            let entry = inner.entries.get_mut(id)?;
            let before = entry.job.clone();
            change(&mut entry.job);
            (entry.job.clone(), entry.job != before)
        };
        if changed {
            self.journal.job(&job);
            self.events.emit(Event::Job { job: job.clone() });
            self.changed.notify_waiters();
        }
        Some(job)
    }

    pub fn log(&self, id: &JobId, stream: Stream, text: String) {
        let line = {
            let mut inner = self.inner.lock().unwrap();
            let Some(entry) = inner.entries.get_mut(id) else { return };
            entry.next_line += 1;
            let line = LogLine { n: entry.next_line, stream, text, at: now_ms() };
            entry.logs.push_back(line.clone());
            if entry.logs.len() > KEEP_LINES {
                entry.logs.pop_front();
            }
            line
        };
        self.journal.log(id, &line);
        self.events.emit(Event::JobLog { job: id.clone(), line });
        self.changed.notify_waiters();
    }

    pub fn replay(&self, id: &JobId, lines: Vec<LogLine>) {
        for line in lines {
            self.log(id, line.stream, line.text);
        }
    }

    pub fn set_artifacts(&self, id: &JobId, artifacts: Vec<Artifact>) {
        self.journal.artifacts(id, &artifacts);
        if let Some(entry) = self.inner.lock().unwrap().entries.get_mut(id) {
            entry.artifacts = artifacts;
        }
    }

    pub fn job(&self, id: &JobId) -> Result<Job> {
        self.with(id, |entry| entry.job.clone())
    }

    pub fn jobs(&self) -> Vec<Job> {
        let mut jobs: Vec<Job> = self.inner.lock().unwrap().entries.values().map(|e| e.job.clone()).collect();
        jobs.sort_by_key(|job| job.created_at);
        jobs
    }

    pub fn logs(&self, id: &JobId, after: u64, limit: usize) -> Result<JobLogsOutput> {
        Ok(self.page(id, after, limit)?.0)
    }

    pub async fn wait_logs(&self, id: &JobId, after: u64, limit: usize, wait: Duration) -> Result<JobLogsOutput> {
        let deadline = tokio::time::Instant::now() + wait;
        let (_, state) = self.page(id, after, 0)?;
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let (page, now) = self.page(id, after, limit)?;
            if !page.lines.is_empty() || page.done || now != state {
                return Ok(page);
            }
            if tokio::time::timeout_at(deadline, changed).await.is_err() {
                return Ok(page);
            }
        }
    }

    fn page(&self, id: &JobId, after: u64, limit: usize) -> Result<(JobLogsOutput, JobState)> {
        self.with(id, |entry| {
            let start = entry.logs.partition_point(|l| l.n <= after);
            let lines = entry.logs.range(start..).take(limit).cloned().collect();
            (JobLogsOutput { lines, done: entry.job.state.is_done() }, entry.job.state)
        })
    }

    pub fn tail(&self, id: &JobId, count: usize) -> Vec<LogLine> {
        self.with(id, |entry| entry.logs.iter().rev().take(count).rev().cloned().collect()).unwrap_or_default()
    }

    pub fn artifacts(&self, id: &JobId) -> Result<Vec<Artifact>> {
        self.with(id, |entry| entry.artifacts.clone())
    }

    pub fn cancel(&self, id: &JobId) -> Result<()> {
        self.with(id, |entry| {
            let _ = entry.cancel.send(true);
        })
    }

    pub async fn done(&self, id: &JobId) -> Result<Job> {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let job = self.job(id)?;
            if job.state.is_done() {
                return Ok(job);
            }
            changed.await;
        }
    }

    pub async fn wait_done(&self, id: &JobId, limit: Duration) -> Result<Job> {
        match tokio::time::timeout(limit, self.done(id)).await {
            Ok(done) => done,
            Err(_) => self.job(id),
        }
    }

    fn with<T>(&self, id: &JobId, read: impl FnOnce(&Entry) -> T) -> Result<T> {
        let inner = self.inner.lock().unwrap();
        inner.entries.get(id).map(read).ok_or_else(|| Error::not_found(format!("no job {id}")))
    }
}
