mod client;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use orrin_core::Home;
use orrin_server::Server;
use serde_json::Value;

#[derive(Parser)]
#[command(name = "orrin", version, about = "Owns your projects: files, views, landing, builds and sync")]
struct Cli {
    #[arg(long, global = true)]
    home: Option<PathBuf>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Serve {
        #[arg(long)]
        port: Option<u16>,
    },
    Call {
        request: String,
        #[arg(long, default_value = "{}")]
        json: String,
    },
    Ping,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let home = Home::resolve(cli.home)?;
    match cli.command {
        Command::Serve { port } => serve(home, port).await,
        Command::Call { request, json } => print(client::call(&home, &request, serde_json::from_str(&json)?).await?),
        Command::Ping => print(client::call(&home, "ping", Value::Object(Default::default())).await?),
    }
}

async fn serve(home: Home, port: Option<u16>) -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::from_default_env()).init();
    let server = Server::bind(home, port).await?;
    println!("orrin listening on port {}", server.port());
    server.run(async { tokio::signal::ctrl_c().await.unwrap_or_default() }).await?;
    Ok(())
}

fn print(value: Value) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(&value)?);
    Ok(())
}
