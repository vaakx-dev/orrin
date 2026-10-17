use std::collections::HashSet;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};

use orrin_core::{Error, Hash, Result};
use rusqlite::{Connection, OptionalExtension, params};

use crate::object::Manifest;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS slices (
    chunk TEXT PRIMARY KEY,
    file TEXT NOT NULL,
    offset INTEGER NOT NULL,
    len INTEGER NOT NULL
);
";

pub struct Slices(Mutex<Connection>);

pub struct Slice<'a> {
    pub chunk: &'a Hash,
    pub offset: u64,
    pub len: u64,
}

pub struct Located {
    pub file: Hash,
    pub offset: u64,
    pub len: u64,
}

impl Located {
    pub fn of(manifest: Manifest) -> Option<Self> {
        match manifest {
            Manifest::Slice { file, offset, len } => Some(Self { file, offset, len }),
            _ => None,
        }
    }
}

impl From<Located> for Manifest {
    fn from(at: Located) -> Self {
        Manifest::Slice { file: at.file, offset: at.offset, len: at.len }
    }
}

impl Slices {
    pub fn open(path: &Path) -> Result<Self> {
        let db = Connection::open(path).map_err(sql)?;
        db.pragma_update(None, "journal_mode", "WAL").map_err(sql)?;
        db.pragma_update(None, "synchronous", "NORMAL").map_err(sql)?;
        db.execute_batch(SCHEMA).map_err(sql)?;
        Ok(Self(Mutex::new(db)))
    }

    pub fn get(&self, chunk: &Hash) -> Option<Located> {
        let row: Option<(String, i64, i64)> = self
            .db()
            .query_row("SELECT file, offset, len FROM slices WHERE chunk = ?1", [chunk.as_str()], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .optional()
            .ok()
            .flatten();
        let (file, offset, len) = row?;
        Some(Located { file: Hash::parse(&file).ok()?, offset: offset as u64, len: len as u64 })
    }

    pub fn has(&self, chunk: &Hash) -> bool {
        self.db()
            .query_row("SELECT 1 FROM slices WHERE chunk = ?1", [chunk.as_str()], |_| Ok(()))
            .optional()
            .is_ok_and(|row| row.is_some())
    }

    pub fn add(&self, file: &Hash, slices: &[Slice]) -> Result<()> {
        let mut db = self.db();
        let tx = db.transaction().map_err(sql)?;
        {
            let mut insert = tx
                .prepare("INSERT OR IGNORE INTO slices (chunk, file, offset, len) VALUES (?1, ?2, ?3, ?4)")
                .map_err(sql)?;
            for slice in slices {
                insert
                    .execute(params![slice.chunk.as_str(), file.as_str(), slice.offset as i64, slice.len as i64])
                    .map_err(sql)?;
            }
        }
        tx.commit().map_err(sql)
    }

    pub fn point(&self, file: &Hash, slice: &Slice) -> Result<()> {
        self.db()
            .execute(
                "INSERT OR REPLACE INTO slices (chunk, file, offset, len) VALUES (?1, ?2, ?3, ?4)",
                params![slice.chunk.as_str(), file.as_str(), slice.offset as i64, slice.len as i64],
            )
            .map_err(sql)?;
        Ok(())
    }

    pub fn remove(&self, chunk: &Hash) -> Result<()> {
        self.db().execute("DELETE FROM slices WHERE chunk = ?1", [chunk.as_str()]).map_err(sql)?;
        Ok(())
    }

    pub fn keep_only(&self, live: &HashSet<Hash>) -> Result<()> {
        let mut db = self.db();
        let chunks: Vec<String> = {
            let mut all = db.prepare("SELECT chunk FROM slices").map_err(sql)?;
            all.query_map([], |r| r.get(0)).map_err(sql)?.flatten().collect()
        };
        let tx = db.transaction().map_err(sql)?;
        for chunk in chunks.iter().filter(|c| Hash::parse(c).is_ok_and(|h| !live.contains(&h))) {
            tx.execute("DELETE FROM slices WHERE chunk = ?1", [chunk]).map_err(sql)?;
        }
        tx.commit().map_err(sql)
    }

    fn db(&self) -> MutexGuard<'_, Connection> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn sql(err: rusqlite::Error) -> Error {
    Error::internal(format!("store slices: {err}"))
}
