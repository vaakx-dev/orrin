mod builds;
mod cache;
mod facts;
mod git;
mod hub;
mod index;
mod landing;
mod packages;
mod peers;
mod projects;
mod store;
#[rustfmt::skip]
mod unavailable;
mod views;

pub use builds::Builds;
pub use cache::Cache;
pub use facts::Facts;
pub use git::Git;
pub use hub::{Hub, Services, Setup};
pub use index::Index;
pub use landing::{Landing, Merged, Merger};
pub use packages::{PackageNeed, PackageReady, Packages};
pub use peers::{Peers, call_peer};
pub use projects::Projects;
pub use store::{GcReport, Materialise, Store};
pub use unavailable::Unavailable;
pub use views::{Edits, Views};

use crate::error::Result;

pub trait Service: Send + Sync {
    fn start(&self) -> Result<()> {
        Ok(())
    }
}
