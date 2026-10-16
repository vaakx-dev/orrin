use std::fs::{self, File};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

use orrin_core::id::fresh;
use orrin_core::service::Materialise;
use orrin_core::{Hash, Result};

use crate::disk::{DiskStore, place_new};
use crate::verified::Verified;

impl DiskStore {
    pub(crate) fn materialise_at(&self, hash: &Hash, path: &Path, how: Materialise) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        match how {
            Materialise::Copy => self.copy_to(hash, path, false),
            Materialise::ReadonlyCopy => self.copy_to(hash, path, true),
            Materialise::Link => self.link_to(hash, path),
        }
    }

    fn copy_to(&self, hash: &Hash, path: &Path, readonly: bool) -> Result<()> {
        let temp = sibling_temp(path);
        self.write_file(hash, &temp, readonly)?;
        replace(&temp, path)
    }

    fn link_to(&self, hash: &Hash, path: &Path) -> Result<()> {
        let source = self.file_path(hash);
        if !source.is_file() {
            let temp = self.temp_path();
            self.write_file(hash, &temp, true)?;
            place_new(&temp, &source)?;
        }
        let temp = sibling_temp(path);
        match fs::hard_link(&source, &temp) {
            Ok(()) => replace(&temp, path),
            Err(_) => self.copy_to(hash, path, true),
        }
    }

    fn write_file(&self, hash: &Hash, temp: &Path, readonly: bool) -> Result<()> {
        let result = self.try_write_file(hash, temp, readonly);
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }

    fn try_write_file(&self, hash: &Hash, temp: &Path, readonly: bool) -> Result<()> {
        let mut out = Verified::new(BufWriter::new(File::create(temp)?));
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
