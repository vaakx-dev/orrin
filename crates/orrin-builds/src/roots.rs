use orrin_core::Hash;
use orrin_core::model::JobResult;
use orrin_core::service::Store;

use crate::context::Context;

pub fn collect(ctx: &Context) -> Vec<Hash> {
    let store = &*ctx.hub.store;
    let jobs = ctx.table.jobs();
    let mut results = ctx.cache.results();
    results.extend(jobs.iter().filter_map(|job| job.result.clone()));
    let mut roots: Vec<Hash> = jobs.into_iter().map(|job| job.snapshot).collect();
    roots.extend(ctx.links.roots());
    for result in results {
        roots.extend(artifacts(store, &result));
        roots.push(result);
    }
    roots
}

fn artifacts(store: &dyn Store, result: &Hash) -> Vec<Hash> {
    let bytes = store.get(result).unwrap_or_default();
    let result: JobResult = serde_json::from_slice(&bytes).unwrap_or_default();
    result.artifacts.into_iter().map(|a| a.hash).collect()
}
