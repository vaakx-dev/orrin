use std::collections::HashMap;
use std::sync::Arc;

use orrin_core::Result;
use orrin_core::model::{CompilerCache, Rule, Stream};

use super::{Build, setup};
use crate::runner::compiler::{self, Wiring};
use crate::tools;

#[derive(Clone)]
pub struct Prepared {
    pub delta: setup::Delta,
    pub cache: Arc<Wiring>,
}

#[derive(Default)]
pub struct Prep {
    setups: HashMap<String, setup::Delta>,
    caches: HashMap<CompilerCache, Arc<Wiring>>,
}

impl Prep {
    pub async fn rule(&mut self, build: &Build<'_>, rule: &Rule) -> Result<Prepared> {
        let delta = match &rule.setup {
            Some(line) => self.setup(build, line).await?,
            None => setup::Delta::new(),
        };
        Ok(Prepared { delta, cache: self.cache(build, rule.cache).await? })
    }

    async fn setup(&mut self, build: &Build<'_>, line: &str) -> Result<setup::Delta> {
        if let Some(delta) = self.setups.get(line) {
            return Ok(delta.clone());
        }
        let command = tools::command_line(line, build.tools);
        let (delta, fresh) =
            setup::capture(&build.ctx.home, &command, build.dir, build.env, build.cancel.clone()).await?;
        if fresh {
            build.ctx.table.log(build.id, Stream::System, format!("setup: ran {command}"));
        }
        self.setups.insert(line.to_string(), delta.clone());
        Ok(delta)
    }

    async fn cache(&mut self, build: &Build<'_>, cache: CompilerCache) -> Result<Arc<Wiring>> {
        if let Some(wiring) = self.caches.get(&cache) {
            return Ok(wiring.clone());
        }
        let wiring = Arc::new(compiler::backend(build.ctx, cache, build.dir).await?);
        if let Some(note) = &wiring.note {
            build.ctx.table.log(build.id, Stream::System, note.clone());
        }
        self.caches.insert(cache, wiring.clone());
        Ok(wiring)
    }
}
