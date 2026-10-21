mod api;
mod cli;
mod client;
mod deliver;
mod exit;
mod index;
mod jobs;
mod pcs;
mod projects;
mod table;
mod target;

use std::process::ExitCode;

use clap::Parser;
use orrin_core::Home;
use orrin_core::model::JobKind;
use orrin_server::Server;

use crate::api::Api;
use crate::cli::{Cli, Command, ViewCommand};
use crate::exit::Outcome;

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(outcome) => outcome.into(),
        Err(error) => {
            eprintln!("orrin: {error}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> anyhow::Result<Outcome> {
    let api = Api { home: Home::resolve(cli.home)?, json: cli.json };
    match cli.command {
        Command::Build(args) => jobs::submit(&api, JobKind::Build, &args).await,
        Command::Run(args) => jobs::submit(&api, JobKind::Run, &args).await,
        Command::Test(args) => jobs::submit(&api, JobKind::Test, &args).await,
        Command::Shot(args) => jobs::submit(&api, JobKind::Screenshot, &args).await,
        Command::Export(args) => jobs::submit(&api, JobKind::Export, &args).await,
        Command::Serve { port } => serve(api.home, port).await,
        Command::Call { request, body } => call(&api, &request, &body).await,
        Command::Ping => call(&api, "ping", "{}").await,
        Command::Status => pcs::status(&api).await,
        Command::Projects => projects::list(&api).await,
        Command::Import { folder, name } => projects::import(&api, folder, name).await,
        Command::Remove { project, delete_folder } => projects::remove(&api, &project, delete_folder).await,
        Command::Views { project } => projects::views(&api, project.as_deref()).await,
        Command::View(ViewCommand::New { project, owner, claim, from }) => {
            projects::create_view(&api, project.as_deref(), owner, claim, from).await
        }
        Command::Land { view, intent, into } => projects::land(&api, &view, intent, into).await,
        Command::Undo { project, change, author } => projects::undo(&api, project.as_deref(), change, author).await,
        Command::Find(scoped) => index::find(&api, &scoped).await,
        Command::Search { scoped, regex, case_sensitive, paths } => {
            index::search(&api, &scoped, index::SearchFlags { regex, case_sensitive, paths }).await
        }
        Command::Outline(scoped) => index::outline(&api, &scoped).await,
        Command::Where { tool } => index::locate_tool(&api, &tool).await,
        Command::Jobs { project } => jobs::list(&api, project.as_deref()).await,
        Command::Deliver { project, status } => deliver::deliver(&api, project.as_deref(), status).await,
        Command::Logs { job, follow } => jobs::logs(&api, &job, follow).await,
        Command::Pcs => pcs::list(&api).await,
        Command::Pair => pcs::pair(&api).await,
        Command::Redeem { code, address } => pcs::redeem(&api, code, address).await,
    }
}

async fn call(api: &Api, request: &str, body: &str) -> anyhow::Result<Outcome> {
    let reply = client::call(&api.home, request, serde_json::from_str(body)?).await?;
    println!("{}", serde_json::to_string_pretty(&reply)?);
    Ok(Outcome::Done)
}

async fn serve(home: Home, port: Option<u16>) -> anyhow::Result<Outcome> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let server = Server::bind(home, port).await?;
    println!("orrin listening on port {}", server.port());
    server.run(async { tokio::signal::ctrl_c().await.unwrap_or_default() }).await?;
    Ok(Outcome::Done)
}
