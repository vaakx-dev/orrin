mod show;

use anyhow::bail;
use orrin_core::api::{Facts, Find, Outline, Search};

use crate::api::Api;
use crate::cli::Scoped;
use crate::exit::Outcome;

pub async fn find(api: &Api, scoped: &Scoped) -> anyhow::Result<Outcome> {
    let (project, pattern) = scoped.split();
    let target = api.target(project, scoped.view.as_deref()).await?;
    let input = Find { project: target.project.id, view: target.view, pattern: pattern.into(), limit: scoped.limit };
    api.show(&api.send(input).await?, show::found)?;
    Ok(Outcome::Done)
}

pub struct SearchFlags {
    pub regex: bool,
    pub case_sensitive: bool,
    pub paths: Option<String>,
}

pub async fn search(api: &Api, scoped: &Scoped, flags: SearchFlags) -> anyhow::Result<Outcome> {
    let (project, query) = scoped.split();
    let target = api.target(project, scoped.view.as_deref()).await?;
    let input = Search {
        project: target.project.id,
        view: target.view,
        query: query.into(),
        regex: flags.regex,
        case_sensitive: flags.case_sensitive,
        paths: flags.paths,
        limit: scoped.limit,
    };
    api.show(&api.send(input).await?, show::hits)?;
    Ok(Outcome::Done)
}

pub async fn outline(api: &Api, scoped: &Scoped) -> anyhow::Result<Outcome> {
    let (project, path) = scoped.split();
    let target = api.target(project, scoped.view.as_deref()).await?;
    let input = Outline { project: target.project.id, view: target.view, path: path.replace('\\', "/") };
    api.show(&api.send(input).await?, show::outline)?;
    Ok(Outcome::Done)
}

pub async fn locate_tool(api: &Api, tool: &str) -> anyhow::Result<Outcome> {
    let out = api.send(Facts { pc: None, name: Some(tool.into()), refresh: false }).await?;
    if out.tools.is_empty() && !api.json {
        bail!("{tool} is not installed on this PC");
    }
    api.show(&out, |o| show::tools(&o.tools))?;
    Ok(Outcome::Done)
}
