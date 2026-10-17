use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use orrin_core::{Error, ErrorCode, Hash, Result};
use rusqlite::types::Type;
use rusqlite::{Connection, ErrorCode as SqlCode, Row};
use serde::de::DeserializeOwned;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    home_pc TEXT NOT NULL,
    main TEXT NOT NULL,
    settings TEXT NOT NULL,
    created_at INTEGER NOT NULL
);
CREATE TABLE IF NOT EXISTS changes (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    project TEXT NOT NULL,
    at INTEGER NOT NULL,
    pinned INTEGER NOT NULL,
    body TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS changes_by_project ON changes (project, seq);
CREATE TABLE IF NOT EXISTS change_paths (
    change TEXT NOT NULL REFERENCES changes (id) ON DELETE CASCADE,
    path TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS change_paths_by_change ON change_paths (change);
CREATE INDEX IF NOT EXISTS change_paths_by_path ON change_paths (path);
";

pub struct Db(Mutex<Connection>);

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open(path).map_err(sql)?;
        conn.busy_timeout(Duration::from_secs(5)).map_err(sql)?;
        conn.pragma_update(None, "journal_mode", "WAL").map_err(sql)?;
        conn.pragma_update(None, "synchronous", "NORMAL").map_err(sql)?;
        conn.pragma_update(None, "foreign_keys", true).map_err(sql)?;
        conn.execute_batch(SCHEMA).map_err(sql)?;
        Ok(Self(Mutex::new(conn)))
    }

    pub fn run<T>(&self, work: impl FnOnce(&mut Connection) -> rusqlite::Result<T>) -> Result<T> {
        let mut conn = self.0.lock().map_err(|_| Error::internal("projects database lock poisoned"))?;
        work(&mut conn).map_err(sql)
    }
}

pub fn sql(err: rusqlite::Error) -> Error {
    let code = match err.sqlite_error_code() {
        Some(SqlCode::ConstraintViolation) => ErrorCode::Conflict,
        _ => ErrorCode::Internal,
    };
    Error::new(code, err.to_string())
}

fn bad_column(idx: usize, err: impl std::error::Error + Send + Sync + 'static) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(idx, Type::Text, Box::new(err))
}

pub fn json<T: DeserializeOwned>(row: &Row, idx: usize) -> rusqlite::Result<T> {
    let text: String = row.get(idx)?;
    serde_json::from_str(&text).map_err(|err| bad_column(idx, err))
}

pub fn hash(row: &Row, idx: usize) -> rusqlite::Result<Hash> {
    let text: String = row.get(idx)?;
    Hash::parse(&text).map_err(|err| bad_column(idx, err))
}

pub fn to_json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).unwrap_or_default()
}
