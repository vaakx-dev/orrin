use std::path::PathBuf;

use orrin_core::api::{
    Event, Log, LogOutput, ProjectOpened, ProjectRemoved, ProjectsCreate, ProjectsListOutput, ProjectsRemove,
    ProjectsSettings, ViewList,
};
use orrin_core::config::HistoryConfig;
use orrin_core::model::{Change, Project, ProjectSettings, Snapshot, ViewKind, clean_path};
use orrin_core::service::{Hub, Projects, Service, Setup};
use orrin_core::{ChangeId, Error, Events, Home, PcId, ProjectId, Result, now_ms};

use crate::change_rows::{self, LogQuery};
use crate::db::Db;
use crate::swap::swap_main;
use crate::{folder, project_rows, thin};

const LOG_LIMIT: u32 = 50;

pub struct Catalog {
    db: Db,
    hub: Hub,
    home: Home,
    events: Events,
    me: PcId,
    history: HistoryConfig,
}

impl Catalog {
    pub fn open(setup: &Setup) -> Result<Self> {
        Ok(Self {
            db: Db::open(&setup.home.db())?,
            hub: setup.hub.clone(),
            home: setup.home.clone(),
            events: setup.events.clone(),
            me: setup.me.pc.clone(),
            history: setup.config.history.clone(),
        })
    }

    fn new_project(&self, input: &ProjectsCreate) -> Result<Project> {
        let name = input.name.trim();
        if name.is_empty() {
            return Err(Error::invalid("project name is empty"));
        }
        Ok(Project {
            id: ProjectId::new(),
            name: name.to_string(),
            home_pc: input.home_pc.clone().unwrap_or_else(|| self.me.clone()),
            main: self.hub.store.put_snapshot(&Snapshot::default())?,
            settings: ProjectSettings::default(),
            created_at: now_ms(),
        })
    }

    async fn opened(&self, project: Project, input: &ProjectsCreate) -> Result<ProjectOpened> {
        let Some(path) = &input.path else {
            return Ok(ProjectOpened { project, view: None, files: 0 });
        };
        let view = match self.hub.views.adopt(&project.id, path).await {
            Ok(view) => view,
            Err(err) => {
                self.db.run(|conn| project_rows::delete(conn, &project.id))?;
                return Err(err);
            }
        };
        let project = self.get(&project.id).await?;
        let files = self.hub.store.snapshot(&project.main)?.entries.len() as u64;
        Ok(ProjectOpened { project, view: Some(view), files })
    }

    async fn deletable_folders(&self, project: &ProjectId) -> Result<Vec<PathBuf>> {
        let views = self.hub.views.list(ViewList { project: Some(project.clone()) }).await?.views;
        let folders: Vec<PathBuf> = views.into_iter().filter(|v| v.kind == ViewKind::User).map(|v| v.path).collect();
        let home = folder::user_home();
        for path in &folders {
            folder::check_deletable(path, self.home.root(), home.as_deref())?;
        }
        Ok(folders)
    }
}

impl Service for Catalog {}

#[async_trait::async_trait]
impl Projects for Catalog {
    async fn list(&self) -> Result<ProjectsListOutput> {
        Ok(ProjectsListOutput { projects: self.db.run(|conn| project_rows::list(conn))? })
    }

    async fn create(&self, input: ProjectsCreate) -> Result<ProjectOpened> {
        let project = self.new_project(&input)?;
        self.db.run(|conn| project_rows::insert(conn, &project))?;
        let opened = self.opened(project, &input).await?;
        self.events.emit(Event::ProjectCreated { project: opened.project.clone() });
        Ok(opened)
    }

    async fn settings(&self, input: ProjectsSettings) -> Result<ProjectSettings> {
        let Some(settings) = input.set else {
            return Ok(self.get(&input.project).await?.settings);
        };
        if !self.db.run(|conn| project_rows::set_settings(conn, &input.project, &settings))? {
            return Err(Error::not_found(format!("no project {}", input.project)));
        }
        Ok(settings)
    }

    async fn log(&self, input: Log) -> Result<LogOutput> {
        self.get(&input.project).await?;
        let path = input.path.as_deref().map(clean_path).transpose()?;
        let query = LogQuery {
            project: &input.project,
            path: path.as_deref(),
            before: input.before.as_ref(),
            limit: input.limit.unwrap_or(LOG_LIMIT),
        };
        Ok(LogOutput { changes: self.db.run(|conn| change_rows::log(conn, &query))? })
    }

    async fn get(&self, id: &ProjectId) -> Result<Project> {
        self.db.run(|conn| project_rows::get(conn, id))?.ok_or_else(|| Error::not_found(format!("no project {id}")))
    }

    async fn save(&self, project: Project) -> Result<()> {
        let inserted = self.db.run(|conn| {
            if project_rows::update(conn, &project)? {
                return Ok(false);
            }
            project_rows::insert(conn, &project)?;
            Ok(true)
        })?;
        if inserted {
            self.events.emit(Event::ProjectCreated { project });
        }
        Ok(())
    }

    async fn swap_main(&self, change: Change) -> Result<Project> {
        let project = self.db.run(|conn| swap_main(conn, &change))?.into_result(&change)?;
        self.events.emit(Event::MainMoved {
            project: project.id.clone(),
            main: project.main.clone(),
            change: change.id,
        });
        self.thin(&project.id).await?;
        Ok(project)
    }

    async fn append(&self, changes: Vec<Change>) -> Result<()> {
        self.db.run(|conn| {
            let tx = conn.transaction()?;
            for change in &changes {
                change_rows::insert(&tx, change)?;
            }
            tx.commit()
        })
    }

    async fn change(&self, id: &ChangeId) -> Result<Change> {
        self.db.run(|conn| change_rows::get(conn, id))?.ok_or_else(|| Error::not_found(format!("no change {id}")))
    }

    async fn pin(&self, id: &ChangeId, pinned: bool) -> Result<()> {
        if !self.db.run(|conn| change_rows::pin(conn, id, pinned))? {
            return Err(Error::not_found(format!("no change {id}")));
        }
        Ok(())
    }

    async fn remove(&self, input: ProjectsRemove) -> Result<ProjectRemoved> {
        let project = self.get(&input.project).await?;
        let folders = match input.delete_folder {
            true => self.deletable_folders(&project.id).await?,
            false => Vec::new(),
        };
        self.hub.builds.remove_project(&project.id).await?;
        let views = self.hub.views.remove_project(&project.id).await?;
        self.hub.index.remove_project(&project.id).await;
        self.db.run(|conn| {
            let tx = conn.transaction()?;
            change_rows::delete_project(&tx, &project.id)?;
            project_rows::delete(&tx, &project.id)?;
            tx.commit()
        })?;
        for path in &folders {
            folder::delete(path.clone()).await?;
        }
        self.events.emit(Event::ProjectRemoved { project: project.id.clone() });
        Ok(ProjectRemoved { project, views, deleted_folders: folders })
    }

    async fn thin(&self, project: &ProjectId) -> Result<u32> {
        self.db.run(|conn| {
            let tx = conn.transaction()?;
            let doomed = thin::doomed(&change_rows::dated(&tx, project)?, &self.history);
            for id in &doomed {
                change_rows::delete(&tx, id)?;
            }
            tx.commit()?;
            Ok(doomed.len() as u32)
        })
    }
}
