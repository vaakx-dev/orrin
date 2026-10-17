mod archive;
mod command;
mod git;

use std::path::{Path, PathBuf};

use orrin_core::model::Package;
use orrin_core::{Error, Hash, Result};
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Git { url: String, checkout: Checkout },
    Archive { url: String },
    Folder { dir: PathBuf },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Checkout {
    Tag(String),
    Rev(String),
}

impl Source {
    pub fn of(name: &str, package: &Package, base: Option<&Path>) -> Result<Self> {
        let bad = |why: &str| Error::invalid(format!("package {name}: {why}"));
        let checkout = match (&package.tag, &package.rev) {
            (Some(tag), None) => Some(Checkout::Tag(tag.clone())),
            (None, Some(rev)) => Some(Checkout::Rev(rev.clone())),
            (None, None) => None,
            (Some(_), Some(_)) => return Err(bad("give tag or rev, not both")),
        };
        match (&package.git, &package.url, &package.path, checkout) {
            (Some(url), None, None, Some(checkout)) => Ok(Self::Git { url: url.clone(), checkout }),
            (Some(_), None, None, None) => Err(bad("a git package needs a tag or rev")),
            (None, Some(url), None, None) => Ok(Self::Archive { url: url.clone() }),
            (None, None, Some(path), None) => Ok(Self::Folder {
                dir: folder(path, base)
                    .ok_or_else(|| bad("a relative path needs the project's own folder on this PC"))?,
            }),
            _ => Err(bad("give exactly one of git, url or path; tag and rev only go with git")),
        }
    }

    pub fn key(&self) -> Hash {
        Hash::of(&serde_json::to_vec(self).unwrap_or_default())
    }

    pub async fn fetch(&self, into: &Path) -> Result<PathBuf> {
        match self {
            Self::Git { url, checkout } => git::fetch(url, checkout, into).await.map(|()| into.to_path_buf()),
            Self::Archive { url } => archive::fetch(url, into).await,
            Self::Folder { dir } => Ok(dir.clone()),
        }
    }
}

fn folder(path: &str, base: Option<&Path>) -> Option<PathBuf> {
    let path = Path::new(path);
    match path.is_absolute() {
        true => Some(path.to_path_buf()),
        false => base.map(|base| base.join(path)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package(text: &str) -> Package {
        toml::from_str(text).unwrap()
    }

    #[test]
    fn reads_one_source_per_package() {
        let git = Source::of("x", &package("git = \"https://g/x\"\ntag = \"v1\""), None).unwrap();
        assert_eq!(git, Source::Git { url: "https://g/x".into(), checkout: Checkout::Tag("v1".into()) });
        assert_ne!(git.key(), Source::of("x", &package("git = \"https://g/x\"\ntag = \"v2\""), None).unwrap().key());
        let base = std::env::temp_dir();
        let folder = Source::of("x", &package("path = \"lib\""), Some(&base)).unwrap();
        assert_eq!(folder, Source::Folder { dir: base.join("lib") });
        assert!(Source::of("x", &package("path = \"lib\""), None).is_err());
        assert!(Source::of("x", &package("git = \"https://g/x\""), None).is_err());
        assert!(Source::of("x", &package("url = \"https://a\"\npath = \"/b\""), None).is_err());
    }
}
