use std::fs::{self, File};
use std::io::{self, BufWriter};
use std::path::{Path, PathBuf};

use orrin_core::id::fresh;
use orrin_core::service::Materialise;
use orrin_core::{Hash, Result};

use crate::disk::{DiskStore, in_dir};
use crate::verified::Verified;

impl DiskStore {
    pub(crate) fn materialise_at(&self, hash: &Hash, path: &Path, how: Materialise) -> Result<()> {
        match how {
            Materialise::Copy => self.copy_to(hash, path, false),
            Materialise::ReadonlyCopy => self.copy_to(hash, path, true),
            Materialise::Link => self.link_to(hash, path),
        }
    }

    pub(crate) fn copy_to(&self, hash: &Hash, path: &Path, readonly: bool) -> Result<()> {
        match in_dir(path, || File::create_new(path)) {
            Ok(file) => self.fill(hash, file, readonly).inspect_err(|_| {
                let _ = fs::remove_file(path);
            }),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
                let temp = sibling_temp(path);
                self.write_file(hash, &temp, readonly)?;
                replace(&temp, path)
            }
            Err(err) => Err(err.into()),
        }
    }

    pub(crate) fn write_file(&self, hash: &Hash, temp: &Path, readonly: bool) -> Result<()> {
        let result = File::create(temp).map_err(Into::into).and_then(|file| self.fill(hash, file, readonly));
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }

    fn fill(&self, hash: &Hash, file: File, readonly: bool) -> Result<()> {
        let mut out = Verified::new(BufWriter::new(file));
        self.write_to(hash, &mut out)?;
        let file = out.finish(hash)?.into_inner().map_err(|err| err.into_error())?;
        if readonly {
            let mut permissions = file.metadata()?.permissions();
            permissions.set_readonly(true);
            file.set_permissions(permissions)?;
        }
        Ok(())
    }
}

pub(crate) fn place(path: &Path, make: impl Fn(&Path) -> io::Result<()>) -> Result<()> {
    match in_dir(path, || make(path)) {
        Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {
            let temp = sibling_temp(path);
            make(&temp)?;
            replace(&temp, path)
        }
        other => Ok(other?),
    }
}

fn sibling_temp(path: &Path) -> PathBuf {
    path.with_file_name(format!(".{}.orrin-tmp", fresh("m")))
}

fn replace(temp: &Path, path: &Path) -> Result<()> {
    if fs::rename(temp, path).is_ok() {
        return Ok(());
    }
    let result = fs::remove_file(path).and_then(|()| fs::rename(temp, path));
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    Ok(result?)
}
