mod deps;
mod files;
mod fill;
mod key;
mod listing;
mod prep;
mod records;
mod setup;
mod task;
mod tasks;

use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use futures_util::{StreamExt, stream};
use orrin_core::model::{Artifact, Deps, Rule, Snapshot, Stream};
use orrin_core::{Error, Hash, JobId, Result};
use tokio::sync::watch;

pub use records::Records;

use super::compiler::{self, Env, Wiring};
use super::packages::{self, Placed};
use crate::context::Context;
use crate::plan::Plan;
use crate::process::{self, Exit, Limits};
use crate::tools::{self, Tools};
use fill::{Values, fill, native};
use prep::{Prep, Prepared};
use task::Done;
use tasks::Task;

type Pending<'a> = Pin<Box<dyn Future<Output = Result<Done>> + Send + 'a>>;

pub struct Build<'a> {
    pub ctx: &'a Context,
    pub plan: &'a Plan,
    pub tools: &'a Tools,
    pub id: &'a JobId,
    pub dir: &'a Path,
    pub files: &'a Snapshot,
    pub env: &'a Env,
    pub packages: &'a [Placed],
    pub parallel: u32,
    pub cancel: watch::Receiver<bool>,
}

pub struct Built {
    pub exit: Exit,
    pub outputs: Vec<String>,
}

pub async fn build(build: Build<'_>) -> Result<Built> {
    let records = Records::open(&build.ctx.home)?;
    let mut known: HashMap<String, Hash> =
        build.files.entries.iter().map(|e| (e.path.clone(), e.hash.clone())).collect();
    let mut prep = Prep::default();
    let mut previous = Vec::new();
    for step in &build.plan.target.steps {
        let rule = build.plan.manifest.rules.get(&step.rule);
        let rule = rule.ok_or_else(|| Error::not_found(format!("no rule {}", step.rule)))?;
        let prepared = prep.rule(&build, rule).await?;
        let group = Group::new(&build, &step.rule, rule, &step.vars, prepared, &records, &known);
        let mut files: Vec<String> = known.keys().cloned().collect();
        files.sort();
        let tasks = tasks::expand(step, rule, &files, &previous, &group.values)?;
        let outputs = match group.run(&tasks).await? {
            Ok(outputs) => outputs,
            Err(exit) => return Ok(Built { exit, outputs: Vec::new() }),
        };
        known.extend(outputs.into_iter().map(|a| (a.path, a.hash)));
        previous = tasks.into_iter().filter_map(|t| t.out).collect();
    }
    Ok(Built { exit: Exit::Code(Some(0)), outputs: previous })
}

struct Group<'a> {
    build: &'a Build<'a>,
    name: &'a str,
    rule: &'a Rule,
    values: Values,
    key_values: Values,
    env: Env,
    delta: setup::Delta,
    cache: Arc<Wiring>,
    records: &'a Records,
    known: &'a HashMap<String, Hash>,
}

impl<'a> Group<'a> {
    fn new(
        build: &'a Build<'a>,
        name: &'a str,
        rule: &'a Rule,
        vars: &BTreeMap<String, String>,
        prepared: Prepared,
        records: &'a Records,
        known: &'a HashMap<String, Hash>,
    ) -> Self {
        let plan = build.plan;
        let args = process::join_args(&plan.args);
        let dir = build.dir.to_string_lossy().into_owned();
        let shots = build.dir.join(super::folder::SCREENSHOTS).to_string_lossy().into_owned();
        let mut values = base(&dir, &shots, build.id.as_str(), &plan.name, &args);
        values.extend(packages::dirs(build.packages));
        let mut key_values = base("<dir>", "<screenshots>", "<job>", &plan.name, &args);
        key_values.extend(packages::stand_ins(build.packages));
        let (values, key_values) = (with_vars(values, vars), with_vars(key_values, vars));
        let Prepared { delta, cache } = prepared;
        let mut env = build.env.clone();
        env.extend(delta.iter().map(|(name, value)| (name.clone(), value.into())));
        env.extend(cache.env.iter().cloned());
        if rule.deps == Deps::Msvc {
            env.push(("VSLANG".into(), "1033".into()));
        }
        Self { build, name, rule, values, key_values, env, delta, cache, records, known }
    }

    async fn run(&self, tasks: &[Task]) -> Result<Result<Vec<Artifact>, Exit>> {
        let parallel = (self.build.parallel as usize).clamp(1, tasks.len().max(1));
        let limits = Limits {
            cpu_percent: None,
            memory_mb: self.build.plan.target.ram_mb.map(|ram| (ram / parallel as u64).max(1)),
        };
        let stop = AtomicBool::new(false);
        let work: Vec<Pending<'_>> = tasks.iter().map(|task| Box::pin(self.task(task, limits, &stop)) as _).collect();
        let ends: Vec<Result<Done>> = stream::iter(work).buffer_unordered(parallel).collect().await;
        let (mut ran, mut reused, mut outputs, mut failed) = (0, 0, Vec::new(), None);
        for end in ends {
            match end? {
                Done::Ran(made) => {
                    ran += 1;
                    outputs.extend(made);
                }
                Done::Reused(made) => {
                    reused += 1;
                    outputs.extend(made);
                }
                Done::Stopped(exit) => failed = failed.or(Some(exit)),
                Done::Skipped => {}
            }
        }
        let note = format!("{}: {ran} ran, {reused} reused", self.name);
        self.build.ctx.table.log(self.build.id, Stream::System, note);
        Ok(failed.map_or(Ok(outputs), Err))
    }

    fn command(&self, task: &Task) -> (String, String) {
        let mut values = self.values.clone();
        let mut key_values = self.key_values.clone();
        let extra = task_values(task);
        values.extend(extra.clone());
        key_values.extend(extra);
        let command = tools::command_line(&self.rule.command, self.build.tools);
        let command = compiler::wrap(&fill(&command, &values), self.cache.tool.as_deref());
        (command, fill(&self.rule.command, &key_values))
    }
}

fn base(dir: &str, shots: &str, job: &str, target: &str, args: &str) -> Values {
    [("dir", dir), ("screenshots", shots), ("job", job), ("target", target), ("args", args)]
        .map(|(name, value)| (name.to_string(), value.to_string()))
        .into()
}

fn with_vars(mut values: Values, vars: &BTreeMap<String, String>) -> Values {
    let filled: Values = vars.iter().map(|(name, value)| (name.clone(), fill(value, &values))).collect();
    values.extend(filled);
    values
}

fn task_values(task: &Task) -> Values {
    let mut values = Values::new();
    if let Some(first) = task.inputs.first() {
        values.extend(tasks::names(first).into_iter().map(|(name, value)| match name.as_str() {
            "path" => (name, native(&value)),
            _ => (name, value),
        }));
    }
    values.insert("in".into(), task.inputs.iter().map(|path| native(path)).collect::<Vec<_>>().join(" "));
    values.insert(listing::NAME.into(), native(&listing::path(task)));
    if let Some(out) = &task.out {
        values.extend(tasks::names(out).into_iter().map(|(name, value)| (format!("out.{name}"), native(&value))));
        values.insert("out".into(), native(out));
    }
    values
}
