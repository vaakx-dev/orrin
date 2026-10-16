mod events;
mod facts;
mod files;
mod git;
mod index;
mod jobs;
mod meta;
mod pcs;
mod projects;
mod request;
mod sync;
mod views;

pub use events::*;
pub use facts::*;
pub use files::*;
pub use git::*;
pub use index::*;
pub use jobs::*;
pub use meta::*;
pub use pcs::*;
pub use projects::*;
pub use request::Request;
pub use sync::*;
pub use views::*;

use crate::model::{Job, Pc, PcSettings, ProjectSettings, Tool, View};
use request::requests;

requests! {
    Ping => "ping": Pong,
    ProjectsList => "projects.list": ProjectsListOutput,
    ProjectsCreate => "projects.create": ProjectOpened,
    ProjectsImport => "projects.import": ProjectOpened,
    ProjectsSettings => "projects.settings": ProjectSettings,
    Log => "log": LogOutput,
    Find => "find": FindOutput,
    Search => "search": SearchOutput,
    Read => "read": ReadOutput,
    Outline => "outline": OutlineOutput,
    Where => "where": WhereOutput,
    Changes => "changes": ChangesOutput,
    Diff => "diff": DiffOutput,
    Patch => "patch": Written,
    Create => "create": Written,
    Move => "move": Touched,
    Delete => "delete": Touched,
    ViewCreate => "view.create": View,
    ViewList => "view.list": ViewListOutput,
    ViewLand => "view.land": LandOutput,
    ViewDrop => "view.drop": Done,
    Undo => "undo": LandOutput,
    Facts => "facts": FactsOutput,
    FactsSet => "facts.set": Tool,
    Build => "build": Job,
    Run => "run": Job,
    Screenshot => "screenshot": Job,
    Test => "test": Job,
    Export => "export": Job,
    JobStatus => "job.status": Job,
    JobLogs => "job.logs": JobLogsOutput,
    JobArtifacts => "job.artifacts": JobArtifactsOutput,
    JobCancel => "job.cancel": Job,
    PcsList => "pcs.list": PcsListOutput,
    PcsPair => "pcs.pair": PcsPairOutput,
    PcsRedeem => "pcs.redeem": Pc,
    PcsSettings => "pcs.settings": PcSettings,
    GitExport => "git.export": GitExportOutput,
    GitImport => "git.import": GitImportOutput,
    SyncPair => "sync.pair": SyncPairOutput,
    SyncAnnounce => "sync.announce": SyncAnnounce,
    SyncMissing => "sync.missing": Hashes,
    SyncGet => "sync.get": Objects,
    SyncPut => "sync.put": Stored,
    SyncChanges => "sync.changes": SyncChangesOutput,
    SyncPoke => "sync.poke": Done,
    SyncLand => "sync.land": LandOutput,
    SyncJob => "sync.job": Job,
    SyncFacts => "sync.facts": FactsOutput,
}

pub fn is_peer_request(name: &str) -> bool {
    name.starts_with("sync.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_fields() {
        let ok: Read = serde_json::from_str(r#"{"project":"p_1","path":"a.txt"}"#).unwrap();
        assert_eq!(ok.path, "a.txt");
        assert!(serde_json::from_str::<Read>(r#"{"project":"p_1","path":"a","x":1}"#).is_err());
        assert!(serde_json::from_str::<Build>(r#"{"project":"p_1","recipe":"r","x":1}"#).is_err());
        assert!(serde_json::from_str::<ProjectsList>("{}").is_ok());
    }

    #[test]
    fn request_names_are_unique() {
        let mut names = REQUESTS.to_vec();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), REQUESTS.len());
        assert_eq!(<ViewLand as Request>::NAME, "view.land");
    }
}
