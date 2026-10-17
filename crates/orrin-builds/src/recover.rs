use std::sync::Arc;
use std::time::Duration;

use orrin_core::model::{Job, JobState, Stream};
use orrin_core::{JobId, PcId, Result, now_ms};

use crate::context::Context;
use crate::{remote, runner};

const KEEP_JOBS: usize = 1000;
const FLUSH_EVERY: Duration = Duration::from_secs(1);

#[derive(Debug, PartialEq, Eq)]
pub enum Recovery {
    Keep,
    Interrupt,
    Requeue,
    Follow(JobId),
}

pub fn recovery(job: &Job, me: &PcId, remote: Option<JobId>) -> Recovery {
    match (job.state, job.pc == *me, remote) {
        (state, _, _) if state.is_done() => Recovery::Keep,
        (_, false, Some(remote)) => Recovery::Follow(remote),
        (JobState::Queued, true, _) => Recovery::Requeue,
        _ => Recovery::Interrupt,
    }
}

pub fn restore(ctx: &Arc<Context>) -> Result<()> {
    for saved in ctx.table.journal().load(KEEP_JOBS)? {
        let (job, args) = (saved.job.clone(), saved.args.clone());
        let action = recovery(&job, &ctx.me.pc, saved.remote.clone());
        let cancel = ctx.table.restore(saved);
        match action {
            Recovery::Keep => {}
            Recovery::Interrupt => interrupt(ctx, &job.id),
            Recovery::Follow(id) => remote::resume(ctx, &job, args, id, cancel),
            Recovery::Requeue => {
                let ctx = ctx.clone();
                tokio::spawn(async move {
                    if let Err(err) = runner::resume(&ctx, &job, args, cancel).await {
                        fail(&ctx, &job.id, err.message);
                    }
                });
            }
        }
    }
    let ctx = ctx.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(FLUSH_EVERY).await;
            ctx.table.journal().flush();
        }
    });
    Ok(())
}

fn interrupt(ctx: &Context, id: &JobId) {
    fail(ctx, id, "interrupted: Orrin restarted while it ran".into());
}

fn fail(ctx: &Context, id: &JobId, error: String) {
    ctx.table.log(id, Stream::System, error.clone());
    ctx.table.update(id, |job| {
        job.state = JobState::Failed;
        job.error = Some(error);
        job.finished_at = Some(now_ms());
    });
}

#[cfg(test)]
mod tests {
    use orrin_core::model::JobKind;
    use orrin_core::{ProjectId, SnapshotId};

    use super::*;

    #[test]
    fn restart_requeues_queued_jobs_and_interrupts_running_ones() {
        let me = PcId::new();
        let job = |state, pc: &PcId| Job {
            id: JobId::new(),
            project: ProjectId::new(),
            kind: JobKind::Build,
            target: "app".into(),
            snapshot: SnapshotId::of(b"snap"),
            pc: pc.clone(),
            state,
            cached: false,
            exit_code: None,
            error: None,
            created_at: 0,
            started_at: None,
            finished_at: None,
            result: None,
        };
        let peer = PcId::new();
        let remote = JobId::new();
        assert_eq!(recovery(&job(JobState::Queued, &me), &me, None), Recovery::Requeue);
        assert_eq!(recovery(&job(JobState::Running, &me), &me, None), Recovery::Interrupt);
        assert_eq!(recovery(&job(JobState::Succeeded, &me), &me, None), Recovery::Keep);
        assert_eq!(recovery(&job(JobState::Running, &peer), &me, Some(remote.clone())), Recovery::Follow(remote));
        assert_eq!(recovery(&job(JobState::Queued, &peer), &me, None), Recovery::Interrupt);
    }
}
