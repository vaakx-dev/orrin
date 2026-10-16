mod follow;
mod show;

use orrin_core::JobId;
use orrin_core::api::{Build, Export, JobInput, JobList, ProjectsList, Run, Screenshot, Test};
use orrin_core::model::{Job, JobKind};

use crate::api::Api;
use crate::cli::JobArgs;
use crate::exit::Outcome;
use crate::projects::pc_names;

pub async fn submit(api: &Api, kind: JobKind, args: &JobArgs) -> anyhow::Result<Outcome> {
    let (project, recipe) = args.scoped.split();
    let target = api.target(project, args.scoped.view.as_deref()).await?;
    let pc = match &args.pc {
        Some(name) => Some(api.pc(name).await?),
        None => None,
    };
    let input =
        JobInput { project: target.project.id, view: target.view, recipe: recipe.into(), args: args.args.clone(), pc };
    let job = send(api, kind, input).await?;
    if args.follow {
        if !api.json {
            eprintln!("» {}", show::job(&job));
        }
        return follow::follow(api, &job.id).await;
    }
    api.show(&job, show::job)?;
    Ok(Outcome::Done)
}

async fn send(api: &Api, kind: JobKind, input: JobInput) -> anyhow::Result<Job> {
    match kind {
        JobKind::Build => api.send(Build(input)).await,
        JobKind::Run => api.send(Run(input)).await,
        JobKind::Screenshot => api.send(Screenshot(input)).await,
        JobKind::Test => api.send(Test(input)).await,
        JobKind::Export => api.send(Export(input)).await,
    }
}

pub async fn list(api: &Api, project: Option<&str>) -> anyhow::Result<Outcome> {
    let project = match project {
        Some(name) => Some(api.project(Some(name)).await?.id),
        None => None,
    };
    let out = api.send(JobList { project }).await?;
    let projects = api.send(ProjectsList {}).await?.projects;
    let names =
        show::Names { projects: projects.into_iter().map(|p| (p.id, p.name)).collect(), pcs: pc_names(api).await };
    api.show(&out, |o| show::jobs(&o.jobs, &names))?;
    Ok(Outcome::Done)
}

pub async fn logs(api: &Api, job: &str, follow: bool) -> anyhow::Result<Outcome> {
    let job = JobId::from(job);
    if follow {
        return follow::follow(api, &job).await;
    }
    follow::print_backlog(api, &job, &mut 0).await?;
    Ok(Outcome::Done)
}
