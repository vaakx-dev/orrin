use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use orrin_core::api::{FactsOutput, FactsSet, SyncFacts};
use orrin_core::model::{Tool, ToolSource};
use orrin_core::service::{Facts, Hub, Service, Setup, call_peer};
use orrin_core::{Error, PcId, Result, now_ms};

use crate::book::{Book, Entry, normalise};
use crate::probe::{Probe, probe};
use crate::scan::{scan, which};

pub struct FactsService {
    inner: Arc<Inner>,
}

struct Inner {
    hub: Hub,
    me: PcId,
    file: PathBuf,
    book: RwLock<Book>,
    refreshing: tokio::sync::Mutex<()>,
}

impl FactsService {
    pub fn open(setup: &Setup) -> Result<Self> {
        let file = setup.home.root().join("facts.json");
        let book = RwLock::new(Book::load(&file));
        let refreshing = tokio::sync::Mutex::new(());
        Ok(Self { inner: Arc::new(Inner { hub: setup.hub.clone(), me: setup.me.pc.clone(), file, book, refreshing }) })
    }
}

impl Inner {
    fn book(&self) -> RwLockReadGuard<'_, Book> {
        self.book.read().unwrap_or_else(|e| e.into_inner())
    }

    fn book_mut(&self) -> RwLockWriteGuard<'_, Book> {
        self.book.write().unwrap_or_else(|e| e.into_inner())
    }

    async fn refresh(&self) -> Result<()> {
        let _running = self.refreshing.lock().await;
        let current = self.book().clone();
        let scanned = scan(&current).await;
        let mut book = self.book_mut();
        book.replace_scanned(scanned, now_ms());
        book.save(&self.file)
    }

    fn record(&self, tool: Tool) -> Result<Option<Tool>> {
        let name = tool.name.clone();
        let mut book = self.book_mut();
        if book.put(Entry::new(tool)) {
            book.save(&self.file)?;
        }
        Ok(book.get(&name).cloned())
    }
}

impl Service for FactsService {
    fn start(&self) -> Result<()> {
        let Ok(runtime) = tokio::runtime::Handle::try_current() else { return Ok(()) };
        let inner = self.inner.clone();
        runtime.spawn(async move {
            if let Err(err) = inner.refresh().await {
                tracing::warn!("facts refresh failed: {err}");
            }
        });
        Ok(())
    }
}

#[async_trait]
impl Facts for FactsService {
    async fn list(&self, input: orrin_core::api::Facts) -> Result<FactsOutput> {
        let wanted = input.name.as_deref().map(normalise);
        let keep = |tool: &Tool| wanted.as_deref().is_none_or(|w| tool.name.contains(w));
        if let Some(pc) = input.pc.filter(|pc| *pc != self.inner.me) {
            let mut remote = call_peer(&*self.inner.hub.peers, &pc, SyncFacts {}).await?;
            remote.tools.retain(keep);
            return Ok(remote);
        }
        if input.refresh {
            self.inner.refresh().await?;
        }
        let tools = self.inner.book().tools().into_iter().filter(keep).collect();
        Ok(FactsOutput { pc: self.inner.me.clone(), tools })
    }

    async fn set(&self, input: FactsSet) -> Result<Tool> {
        let path = Path::new(&input.path);
        if !path.is_file() {
            return Err(Error::invalid(format!("no such file: {}", input.path)));
        }
        let version = match input.version {
            Some(version) => Some(version),
            None => match probe(path, &["--version"]).await {
                Probe::Version(version) => Some(version),
                _ => None,
            },
        };
        let tool = Tool { name: normalise(&input.name), path: input.path, version, source: ToolSource::Manual };
        self.inner.record(tool.clone())?.ok_or_else(|| Error::internal("tool was not stored"))
    }

    async fn tool(&self, name: &str) -> Result<Option<Tool>> {
        if let Some(tool) = self.inner.book().get(name) {
            return Ok(Some(tool.clone()));
        }
        Ok(which(name).map(|path| Tool {
            name: normalise(name),
            path: path.to_string_lossy().into_owned(),
            version: None,
            source: ToolSource::Path,
        }))
    }

    async fn learn(&self, name: &str, path: &str, version: Option<String>) -> Result<()> {
        let resolved = if Path::new(path).is_file() { Some(PathBuf::from(path)) } else { which(path) };
        let Some(resolved) = resolved else { return Ok(()) };
        let tool = Tool {
            name: normalise(name),
            path: resolved.to_string_lossy().into_owned(),
            version,
            source: ToolSource::Learned,
        };
        self.inner.record(tool)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use orrin_core::service::Hub;
    use orrin_core::{Config, Events, Home, Identity};

    use super::*;

    fn setup(home: &Path) -> Setup {
        Setup {
            home: Home::at(home).unwrap(),
            config: Config::default(),
            me: Identity { pc: PcId::from("pc_me"), name: "me".into(), key: "k".into() },
            events: Events::new(),
            hub: Hub::new(),
        }
    }

    #[tokio::test]
    async fn set_learn_and_list_persist() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("blender.exe");
        std::fs::write(&exe, "").unwrap();
        let exe = exe.to_string_lossy().to_string();
        let facts = FactsService::open(&setup(dir.path())).unwrap();

        let set = FactsSet { name: "Blender".into(), path: exe.clone(), version: Some("4.2".into()) };
        assert_eq!(facts.set(set).await.unwrap().source, ToolSource::Manual);
        facts.learn("blender", &exe, None).await.unwrap();
        facts.learn("mytool", &exe, Some("1.0".into())).await.unwrap();
        assert_eq!(facts.tool("blender.exe").await.unwrap().unwrap().version.as_deref(), Some("4.2"));

        let reopened = FactsService::open(&setup(dir.path())).unwrap();
        let listed =
            reopened.list(orrin_core::api::Facts { name: Some("my".into()), ..Default::default() }).await.unwrap();
        assert_eq!(listed.tools.len(), 1);
        assert_eq!(listed.tools[0].source, ToolSource::Learned);
        assert!(reopened.tool("cargo").await.unwrap().is_some());
    }
}
