mod show;

use orrin_core::api::{JobList, PcsList, PcsPair, PcsRedeem, Ping, ProjectsList, ViewList};
use orrin_core::daemon::DaemonInfo;
use orrin_core::model::ViewState;
use serde_json::json;

use crate::api::Api;
use crate::exit::Outcome;

pub async fn list(api: &Api) -> anyhow::Result<Outcome> {
    api.show(&api.send(PcsList {}).await?, |o| show::pcs(&o.pcs))?;
    Ok(Outcome::Done)
}

pub async fn pair(api: &Api) -> anyhow::Result<Outcome> {
    api.show(&api.send(PcsPair {}).await?, show::pair)?;
    Ok(Outcome::Done)
}

pub async fn redeem(api: &Api, code: String, address: String) -> anyhow::Result<Outcome> {
    api.show(&api.send(PcsRedeem { code, address }).await?, show::redeemed)?;
    Ok(Outcome::Done)
}

pub async fn status(api: &Api) -> anyhow::Result<Outcome> {
    let pong = api.send(Ping {}).await?;
    let port = DaemonInfo::read(&api.home)?.port;
    let projects = api.send(ProjectsList {}).await?.projects.len();
    let views = api.send(ViewList { project: None }).await?.views;
    let jobs = api.send(JobList { project: None }).await.map(|o| o.jobs).unwrap_or_default();
    let pcs = api.send(PcsList {}).await.map(|o| o.pcs).unwrap_or_default();
    let peers: Vec<_> = pcs.iter().filter(|pc| !pc.me).collect();
    let here = api.target(None, None).await.ok().map(|t| match t.view {
        Some(view) => format!("{} ({}), view {view}", t.project.name, t.project.id),
        None => format!("{} ({})", t.project.name, t.project.id),
    });
    let status = show::Status {
        name: pong.name.clone(),
        version: pong.version.clone(),
        port,
        projects,
        open_views: views.iter().filter(|v| v.state == ViewState::Open).count(),
        active_jobs: jobs.iter().filter(|j| !j.state.is_done()).count(),
        peers: peers.len(),
        peers_online: peers.iter().filter(|pc| pc.online).count(),
        here: here.clone(),
    };
    let raw = json!({ "pong": pong, "port": port, "projects": projects, "views": views, "jobs": jobs, "pcs": pcs, "here": here });
    api.show(&raw, |_| show::status(&status))?;
    Ok(Outcome::Done)
}
