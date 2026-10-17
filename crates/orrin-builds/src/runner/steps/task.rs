use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};

use orrin_core::model::{Artifact, Deps, Stream};
use orrin_core::{Error, Hash, Result};

use super::records::Outcome;
use super::tasks::Task;
use super::{Group, deps, files, key, listing};
use crate::process::{self, Exit, Limits, Spec};
use crate::runner::blocking;

pub enum Done {
    Ran(Vec<Artifact>),
    Reused(Vec<Artifact>),
    Stopped(Exit),
    Skipped,
}

impl Group<'_> {
    pub(super) async fn task(&self, task: &Task, limits: Limits, stop: &AtomicBool) -> Result<Done> {
        if stop.load(Ordering::Relaxed) {
            return Ok(Done::Skipped);
        }
        let done = self.attempt(task, limits).await;
        if !matches!(done, Ok(Done::Ran(_) | Done::Reused(_))) {
            stop.store(true, Ordering::Relaxed);
        }
        done
    }

    async fn attempt(&self, task: &Task, limits: Limits) -> Result<Done> {
        let (command, key_command) = self.command(task);
        let base = self.base(task, &key_command)?;
        if let Some(outputs) = self.reuse(&base).await? {
            return Ok(Done::Reused(outputs));
        }
        let found = match self.execute(task, &command, limits).await? {
            Ok(found) => found,
            Err(exit) => return Ok(Done::Stopped(exit)),
        };
        let deps = self.deps(task, found)?;
        let (store, dir, out) = (self.build.ctx.hub.store.clone(), self.build.dir.to_path_buf(), task.out.clone());
        let also = task.also.clone();
        let outputs = blocking(move || files::keep(&*store, &dir, out.as_deref(), &also)).await?;
        self.records.put_deps(&base, &deps)?;
        let full = key::full(&base, &self.dep_hashes(deps).await?)?;
        self.records.put_outcome(&full, &Outcome { outputs: outputs.clone() })?;
        Ok(Done::Ran(outputs))
    }

    fn base(&self, task: &Task, command: &str) -> Result<Hash> {
        let inputs = task
            .inputs
            .iter()
            .map(|path| match self.known.get(path) {
                Some(hash) => Ok((path.as_str(), hash)),
                None => Err(Error::invalid(format!("step input {path} is missing"))),
            })
            .collect::<Result<Vec<_>>>()?;
        let tools: Vec<(&str, Option<&str>)> = self
            .build
            .tools
            .iter()
            .map(|(name, tool)| (name.as_str(), tool.as_ref().map(crate::key::tool_identity)))
            .collect();
        key::base(&key::Base {
            command,
            env: &self.delta,
            tools: &tools,
            inputs: &inputs,
            out: task.out.as_deref(),
            also: &task.also,
        })
    }

    async fn reuse(&self, base: &Hash) -> Result<Option<Vec<Artifact>>> {
        let Some(deps) = self.records.deps(base) else { return Ok(None) };
        let full = key::full(base, &self.dep_hashes(deps).await?)?;
        let Some(outcome) = self.records.outcome(&full) else { return Ok(None) };
        let store = self.build.ctx.hub.store.clone();
        if !outcome.outputs.iter().all(|artifact| store.has(&artifact.hash)) {
            return Ok(None);
        }
        let (dir, outputs) = (self.build.dir.to_path_buf(), outcome.outputs.clone());
        blocking(move || files::place(&*store, &dir, &outputs)).await?;
        self.records.used(base, &full);
        Ok(Some(outcome.outputs))
    }

    async fn execute(&self, task: &Task, command: &str, limits: Limits) -> Result<Result<Vec<String>, Exit>> {
        let (table, id, dir) = (&self.build.ctx.table, self.build.id, self.build.dir);
        let label = match (task.inputs.as_slice(), &task.out) {
            ([one], _) => format!(" {one}"),
            (_, Some(out)) => format!(" {out}"),
            _ => String::new(),
        };
        table.log(id, Stream::System, format!("[{}]{label}", self.name));
        files::clear(dir, task.out.as_deref(), &task.also)?;
        let listed = match self.rule.command.contains(&format!("{{{}}}", listing::NAME)) {
            true => Some(listing::write(dir, task)?),
            false => None,
        };
        let spec = Spec {
            command: command.to_string(),
            args: Vec::new(),
            dir: dir.to_path_buf(),
            env: self.env.clone(),
            limits,
        };
        let msvc = self.rule.deps == Deps::Msvc;
        let mut found = Vec::new();
        let exit = process::run(spec, self.build.cancel.clone(), |stream, text| {
            if msvc && let Some(path) = deps::msvc_line(&text) {
                found.push(path.to_string());
                return;
            }
            table.log(id, stream, text);
        })
        .await?;
        if let Some(file) = listed {
            let _ = std::fs::remove_file(file);
        }
        if exit == Exit::Code(Some(0)) {
            return Ok(Ok(found));
        }
        if exit != Exit::Cancelled {
            table.log(id, Stream::System, format!("$ {command}"));
            table.log(id, Stream::System, format!("[{}]{label} failed: {exit:?}", self.name));
        }
        Ok(Err(exit))
    }

    fn deps(&self, task: &Task, found: Vec<String>) -> Result<Vec<String>> {
        let raw = match (self.rule.deps, &task.out) {
            (Deps::None, _) => Vec::new(),
            (Deps::Msvc, _) => found,
            (Deps::Gcc, None) => {
                return Err(Error::invalid(format!("rule {} reads a depfile but has no out", self.name)));
            }
            (Deps::Gcc, Some(out)) => {
                let path = self.build.dir.join(files::depfile(out));
                let text = std::fs::read_to_string(&path)
                    .map_err(|_| Error::invalid(format!("rule {} did not write {}", self.name, files::depfile(out))))?;
                deps::depfile(&text)
            }
        };
        let relative: BTreeSet<String> = raw.iter().map(|path| deps::relative(self.build.dir, path)).collect();
        Ok(relative.into_iter().collect())
    }

    async fn dep_hashes(&self, deps: Vec<String>) -> Result<Vec<(String, Hash)>> {
        let pairs: Vec<(String, Option<Hash>)> = deps
            .into_iter()
            .map(|path| {
                let known = self.known.get(&path).cloned();
                (path, known)
            })
            .collect();
        let dir = self.build.dir.to_path_buf();
        blocking(move || Ok(files::dep_hashes(&dir, pairs))).await
    }
}
