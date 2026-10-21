mod show;

use std::collections::HashMap;
use std::path::PathBuf;

use orrin_core::api::{
    LandOutput, PcsList, ProjectsImport, ProjectsList, ProjectsRemove, Undo, ViewCreate, ViewLand, ViewList,
};
use orrin_core::{ChangeId, PcId, ViewId};

use crate::api::Api;
use crate::exit::Outcome;

pub async fn list(api: &Api) -> anyhow::Result<Outcome> {
    let out = api.send(ProjectsList {}).await?;
    let names = pc_names(api).await;
    api.show(&out, |o| show::projects(&o.projects, &names))?;
    Ok(Outcome::Done)
}

pub async fn import(api: &Api, folder: PathBuf, name: Option<String>) -> anyhow::Result<Outcome> {
    let path = std::path::absolute(folder)?;
    api.show(&api.send(ProjectsImport { path, name }).await?, show::opened)?;
    Ok(Outcome::Done)
}

pub async fn remove(api: &Api, project: &str, delete_folder: bool) -> anyhow::Result<Outcome> {
    let project = api.project(Some(project)).await?.id;
    api.show(&api.send(ProjectsRemove { project, delete_folder }).await?, show::removed)?;
    Ok(Outcome::Done)
}

pub async fn views(api: &Api, project: Option<&str>) -> anyhow::Result<Outcome> {
    let project = match project {
        Some(name) => Some(api.project(Some(name)).await?.id),
        None => api.project(None).await.ok().map(|p| p.id),
    };
    api.show(&api.send(ViewList { project }).await?, |o| show::views(&o.views))?;
    Ok(Outcome::Done)
}

pub async fn create_view(
    api: &Api,
    project: Option<&str>,
    owner: String,
    claims: Vec<String>,
    from: Option<String>,
) -> anyhow::Result<Outcome> {
    let project = api.project(project).await?.id;
    let base_view = from.as_deref().map(ViewId::from);
    api.show(&api.send(ViewCreate { project, owner, base: None, base_view, claims }).await?, show::view)?;
    Ok(Outcome::Done)
}

pub async fn land(api: &Api, view: &str, intent: String, into: Option<String>) -> anyhow::Result<Outcome> {
    let out = api.send(ViewLand { view: ViewId::from(view), intent, into: into.as_deref().map(ViewId::from) }).await?;
    landed(api, out)
}

pub async fn undo(
    api: &Api,
    project: Option<&str>,
    change: Option<String>,
    author: Option<String>,
) -> anyhow::Result<Outcome> {
    let project = api.project(project).await?.id;
    let change = change.as_deref().map(ChangeId::from);
    landed(api, api.send(Undo { project, change, author }).await?)
}

fn landed(api: &Api, out: LandOutput) -> anyhow::Result<Outcome> {
    api.show(&out, show::landed)?;
    Ok(if out.landed { Outcome::Done } else { Outcome::Failed })
}

pub async fn pc_names(api: &Api) -> HashMap<PcId, String> {
    let pcs = api.send(PcsList {}).await.map(|o| o.pcs).unwrap_or_default();
    pcs.into_iter().map(|pc| (pc.id, pc.name)).collect()
}
