use std::path::Path;

use anyhow::{anyhow, bail};
use orrin_core::api::{PcsList, ProjectsList, ViewList};
use orrin_core::model::{Project, View, ViewState};
use orrin_core::{PcId, ProjectId, ViewId};

use crate::api::Api;

pub struct Target {
    pub project: Project,
    pub view: Option<ViewId>,
}

impl Api {
    pub async fn target(&self, project: Option<&str>, view: Option<&str>) -> anyhow::Result<Target> {
        let projects = self.send(ProjectsList {}).await?.projects;
        let view = view.map(ViewId::from);
        if let Some(name) = project {
            return Ok(Target { project: pick(projects, name)?, view });
        }
        let views = self.send(ViewList { project: None }).await?.views;
        let here = std::env::current_dir()?;
        let Some(found) = view_at(&views, &here) else {
            bail!("not inside an Orrin project folder; name the project");
        };
        let project = projects
            .into_iter()
            .find(|p| p.id == found.project)
            .ok_or_else(|| anyhow!("project {} is gone", found.project))?;
        Ok(Target { project, view: view.or(Some(found.id.clone())) })
    }

    pub async fn project(&self, project: Option<&str>) -> anyhow::Result<Project> {
        Ok(self.target(project, None).await?.project)
    }

    pub async fn pc(&self, name: &str) -> anyhow::Result<PcId> {
        let pcs = self.send(PcsList {}).await?.pcs;
        pcs.into_iter()
            .find(|pc| pc.id.as_str() == name || pc.name.eq_ignore_ascii_case(name))
            .map(|pc| pc.id)
            .ok_or_else(|| anyhow!("no PC named {name}"))
    }
}

pub fn pick(projects: Vec<Project>, name: &str) -> anyhow::Result<Project> {
    let id = ProjectId::from(name);
    if let Some(found) = projects.iter().find(|p| p.id == id) {
        return Ok(found.clone());
    }
    let mut named = projects.into_iter().filter(|p| p.name.eq_ignore_ascii_case(name));
    match (named.next(), named.next()) {
        (Some(found), None) => Ok(found),
        (Some(_), Some(_)) => bail!("several projects are named {name}; use the id"),
        _ => bail!("no project named {name}"),
    }
}

pub fn view_at<'a>(views: &'a [View], here: &Path) -> Option<&'a View> {
    let here = normal(here);
    views
        .iter()
        .filter(|v| v.state == ViewState::Open && here.starts_with(normal(&v.path)))
        .max_by_key(|v| v.path.as_os_str().len())
}

fn normal(path: &Path) -> std::path::PathBuf {
    let path = dunce(path.canonicalize().unwrap_or_else(|_| path.to_path_buf()));
    std::path::PathBuf::from(path.to_string_lossy().to_lowercase().replace('\\', "/"))
}

fn dunce(path: std::path::PathBuf) -> std::path::PathBuf {
    let text = path.to_string_lossy();
    match text.strip_prefix(r"\\?\") {
        Some(rest) => rest.into(),
        None => path,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orrin_core::model::{ProjectSettings, ViewKind};
    use orrin_core::{Hash, PcId};

    fn project(id: &str, name: &str) -> Project {
        Project {
            id: id.into(),
            name: name.into(),
            home_pc: PcId::from("pc"),
            main: Hash::of(b""),
            settings: ProjectSettings::default(),
            created_at: 0,
        }
    }

    fn view(id: &str, path: &str) -> View {
        View {
            id: id.into(),
            project: "p1".into(),
            owner: "me".into(),
            kind: ViewKind::User,
            path: path.into(),
            base: Hash::of(b""),
            base_view: None,
            claims: vec![],
            state: ViewState::Open,
            created_at: 0,
            landed: None,
        }
    }

    #[test]
    fn projects_match_by_id_or_name() {
        let all = vec![project("p1", "Game"), project("p2", "Site")];
        assert_eq!(pick(all.clone(), "p2").unwrap().name, "Site");
        assert_eq!(pick(all.clone(), "game").unwrap().id.as_str(), "p1");
        assert!(pick(all, "nope").is_err());
    }

    #[test]
    fn the_deepest_view_holding_the_folder_wins() {
        let root = std::env::temp_dir();
        let outer = root.join("orrin-outer");
        let inner = outer.join("inner");
        let views = vec![view("outer", outer.to_str().unwrap()), view("inner", inner.to_str().unwrap())];
        assert_eq!(view_at(&views, &inner.join("src")).unwrap().id.as_str(), "inner");
        assert_eq!(view_at(&views, &outer).unwrap().id.as_str(), "outer");
        assert!(view_at(&views, &root).is_none());
    }
}
