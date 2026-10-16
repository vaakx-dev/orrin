use orrin_core::api::SyncLand;
use orrin_core::model::View;
use orrin_core::service::Edits;
use orrin_core::{ChangeId, Hash, ProjectId, SnapshotId};

pub struct Proposal {
    pub project: ProjectId,
    pub parent: SnapshotId,
    pub files: Edits,
    pub author: String,
    pub intent: String,
    pub claims: Vec<String>,
    pub undoes: Option<ChangeId>,
}

impl Proposal {
    pub fn from_view(view: &View, files: Edits, intent: String) -> Self {
        Self {
            project: view.project.clone(),
            parent: view.base.clone(),
            files,
            author: view.owner.clone(),
            intent,
            claims: view.claims.clone(),
            undoes: None,
        }
    }

    pub fn from_sync(land: SyncLand) -> Self {
        Self {
            project: land.project,
            parent: land.parent,
            files: land.files,
            author: land.author,
            intent: land.intent,
            claims: land.claims,
            undoes: None,
        }
    }

    pub fn into_sync(self) -> SyncLand {
        SyncLand {
            project: self.project,
            parent: self.parent,
            files: self.files,
            author: self.author,
            intent: self.intent,
            claims: self.claims,
        }
    }

    pub fn hashes(&self) -> Vec<Hash> {
        let files = self.files.values().flatten().map(|state| state.hash.clone());
        std::iter::once(self.parent.clone()).chain(files).collect()
    }
}
