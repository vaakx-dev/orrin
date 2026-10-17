use anyhow::bail;
use futures_util::StreamExt;
use orrin_core::api::{Event, EventEnvelope, JobLogs, JobRef, JobStatus};
use orrin_core::JobId;
use orrin_core::model::{Job, JobState, LogLine, Stream};
use tokio_tungstenite::tungstenite::Message;

use super::show;
use crate::api::Api;
use crate::client::events_url;
use crate::exit::Outcome;

pub async fn follow(api: &Api, job: &JobId) -> anyhow::Result<Outcome> {
    let (mut socket, _) = tokio_tungstenite::connect_async(events_url(&api.home)?).await?;
    let mut last = 0;
    if print_backlog(api, job, &mut last).await? {
        return finish(api, job, &mut last).await;
    }
    while let Some(message) = socket.next().await {
        let Message::Text(text) = message? else { continue };
        let Ok(envelope) = serde_json::from_str::<EventEnvelope>(&text) else { continue };
        match envelope.event {
            Event::JobLog { job: id, line } if id == *job && line.n > last => {
                last = line.n;
                print_line(api, &line)?;
            }
            Event::Job { job: done } if done.id == *job && done.state.is_done() => {
                return finish(api, job, &mut last).await;
            }
            _ => {}
        }
    }
    bail!("lost the connection to orrin before {job} finished")
}

pub async fn print_backlog(api: &Api, job: &JobId, last: &mut u64) -> anyhow::Result<bool> {
    loop {
        let page = api.send(JobLogs { job: job.clone(), after: Some(*last), limit: None, wait_ms: None }).await?;
        let Some(end) = page.lines.last().map(|l| l.n) else { return Ok(page.done) };
        for line in &page.lines {
            print_line(api, line)?;
        }
        *last = end;
    }
}

async fn finish(api: &Api, job: &JobId, last: &mut u64) -> anyhow::Result<Outcome> {
    print_backlog(api, job, last).await?;
    let job: Job = api.send(JobStatus(JobRef { job: job.clone() })).await?;
    match api.json {
        true => println!("{}", serde_json::to_string(&job)?),
        false => eprintln!("» {}", show::job(&job)),
    }
    if let Some(error) = &job.error {
        eprintln!("» {error}");
    }
    Ok(if job.state == JobState::Succeeded { Outcome::Done } else { Outcome::Failed })
}

fn print_line(api: &Api, line: &LogLine) -> anyhow::Result<()> {
    match (api.json, line.stream) {
        (true, _) => println!("{}", serde_json::to_string(line)?),
        (false, Stream::Stderr) => eprintln!("{}", show::line(line)),
        (false, _) => println!("{}", show::line(line)),
    }
    Ok(())
}
