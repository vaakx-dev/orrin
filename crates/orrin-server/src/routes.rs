use orrin_core::api::{self, Pong};
use orrin_core::service::Hub;

use crate::registry::Registry;

pub fn registry() -> Registry {
    let mut r = Registry::default();
    meta(&mut r);
    projects(&mut r);
    files(&mut r);
    index(&mut r);
    views(&mut r);
    jobs(&mut r);
    pcs(&mut r);
    sync(&mut r);
    r
}

fn meta(r: &mut Registry) {
    r.add(|h: Hub, _: api::Ping| async move {
        Ok(Pong {
            pc: h.me.pc.clone(),
            name: h.me.name.clone(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            pid: std::process::id(),
        })
    });
}

fn projects(r: &mut Registry) {
    r.add(|h: Hub, _: api::ProjectsList| async move { h.projects.list().await });
    r.add(|h: Hub, i: api::ProjectsCreate| async move { h.projects.create(i).await });
    r.add(|h: Hub, i: api::ProjectsImport| async move { h.views.import(i).await });
    r.add(|h: Hub, i: api::ProjectsSettings| async move { h.projects.settings(i).await });
    r.add(|h: Hub, i: api::Log| async move { h.projects.log(i).await });
    r.add(|h: Hub, i: api::GitExport| async move { h.git.export(i).await });
    r.add(|h: Hub, i: api::GitImport| async move { h.git.import(i).await });
}

fn files(r: &mut Registry) {
    r.add(|h: Hub, i: api::Read| async move { h.views.read(i).await });
    r.add(|h: Hub, i: api::Patch| async move { h.views.patch(i).await });
    r.add(|h: Hub, i: api::Create| async move { h.views.create_file(i).await });
    r.add(|h: Hub, i: api::Move| async move { h.views.move_file(i).await });
    r.add(|h: Hub, i: api::Delete| async move { h.views.delete(i).await });
    r.add(|h: Hub, i: api::Changes| async move { h.views.changes(i).await });
    r.add(|h: Hub, i: api::Diff| async move { h.views.diff(i).await });
}

fn index(r: &mut Registry) {
    r.add(|h: Hub, i: api::Find| async move { h.index.find(i).await });
    r.add(|h: Hub, i: api::Search| async move { h.index.search(i).await });
    r.add(|h: Hub, i: api::Outline| async move { h.index.outline(i).await });
    r.add(|h: Hub, i: api::Where| async move { h.index.locate(i).await });
}

fn views(r: &mut Registry) {
    r.add(|h: Hub, i: api::ViewCreate| async move { h.views.create(i).await });
    r.add(|h: Hub, i: api::ViewList| async move { h.views.list(i).await });
    r.add(|h: Hub, i: api::ViewDrop| async move { h.views.remove(i).await });
    r.add(|h: Hub, i: api::ViewLand| async move { h.landing.land(i).await });
    r.add(|h: Hub, i: api::Undo| async move { h.landing.undo(i).await });
}

fn jobs(r: &mut Registry) {
    r.add(|h: Hub, i: api::Build| async move { h.builds.submit(i.kind(), i.0).await });
    r.add(|h: Hub, i: api::Run| async move { h.builds.submit(i.kind(), i.0).await });
    r.add(|h: Hub, i: api::Screenshot| async move { h.builds.submit(i.kind(), i.0).await });
    r.add(|h: Hub, i: api::Test| async move { h.builds.submit(i.kind(), i.0).await });
    r.add(|h: Hub, i: api::Export| async move { h.builds.submit(i.kind(), i.0).await });
    r.add(|h: Hub, i: api::JobStatus| async move { h.builds.status(i.0).await });
    r.add(|h: Hub, i: api::JobLogs| async move { h.builds.logs(i).await });
    r.add(|h: Hub, i: api::JobArtifacts| async move { h.builds.artifacts(i.0).await });
    r.add(|h: Hub, i: api::JobCancel| async move { h.builds.cancel(i.0).await });
    r.add(|h: Hub, i: api::JobList| async move { h.builds.list(i).await });
    r.add(|h: Hub, i: api::Facts| async move { h.facts.list(i).await });
    r.add(|h: Hub, i: api::FactsSet| async move { h.facts.set(i).await });
}

fn pcs(r: &mut Registry) {
    r.add(|h: Hub, i: api::PcsList| async move { h.peers.pcs(i).await });
    r.add(|h: Hub, i: api::PcsPair| async move { h.peers.pair(i).await });
    r.add(|h: Hub, i: api::PcsRedeem| async move { h.peers.redeem(i).await });
    r.add(|h: Hub, i: api::PcsSettings| async move { h.peers.settings(i).await });
}

fn sync(r: &mut Registry) {
    r.add(|h: Hub, i: api::SyncPair| async move { h.peers.accept_pair(i).await });
    r.add(|h: Hub, i: api::SyncAnnounce| async move { h.peers.announce(i).await });
    r.add(|h: Hub, i: api::SyncMissing| async move { h.peers.missing(i).await });
    r.add(|h: Hub, i: api::SyncGet| async move { h.peers.objects(i).await });
    r.add(|h: Hub, i: api::SyncPut| async move { h.peers.receive(i).await });
    r.add(|h: Hub, i: api::SyncChanges| async move { h.peers.changes(i).await });
    r.add(|h: Hub, i: api::SyncPoke| async move { h.peers.poke(i).await });
    r.add(|h: Hub, i: api::SyncLand| async move { h.landing.land_change(i).await });
    r.add(|h: Hub, i: api::SyncJob| async move { h.builds.accept(i).await });
    r.add(|h: Hub, i: api::SyncLogs| async move { h.builds.logs(i.0).await });
    r.add(|h: Hub, _: api::SyncFacts| async move { h.facts.list(api::Facts::default()).await });
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_request_is_registered() {
        let mut expected = orrin_core::api::REQUESTS.to_vec();
        expected.sort();
        assert_eq!(super::registry().names(), expected);
    }
}
