use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use orrin_core::{Clock, Error, Hash, Result};
use rusqlite::{Connection, OptionalExtension, params};

use crate::analyse::{Kind, Record, trigrams};

const VERSION: u32 = 1;

pub struct Records {
    db: Mutex<Connection>,
    live: Mutex<HashMap<String, Weak<Record>>>,
}

pub fn key(kind: Kind, hash: &Hash) -> String {
    format!("{VERSION}:{}:{hash}", kind.key())
}

impl Records {
    pub fn open(path: &Path) -> Result<Self> {
        let db = Connection::open(path).map_err(sql)?;
        db.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS records (key TEXT PRIMARY KEY, meta BLOB NOT NULL, trigrams BLOB NOT NULL);
             CREATE TABLE IF NOT EXISTS meta (name TEXT PRIMARY KEY, value INTEGER NOT NULL);",
        )
        .map_err(sql)?;
        Ok(Self { db: Mutex::new(db), live: Mutex::default() })
    }

    pub fn get(&self, key: &str) -> Option<Arc<Record>> {
        if let Some(record) = self.live().get(key).and_then(Weak::upgrade) {
            return Some(record);
        }
        let row: Option<(Vec<u8>, Vec<u8>)> = self
            .db()
            .query_row("SELECT meta, trigrams FROM records WHERE key = ?1", [key], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional()
            .ok()
            .flatten();
        let (meta, grams) = row?;
        let mut record: Record = serde_json::from_slice(&meta).ok()?;
        record.trigrams = trigrams::decode(&grams);
        Some(self.remember(key, record))
    }

    pub fn put_all(&self, fresh: Vec<(String, Record)>) -> Result<Vec<Arc<Record>>> {
        let mut db = self.db();
        let tx = db.transaction().map_err(sql)?;
        for (key, record) in &fresh {
            let meta = serde_json::to_vec(record)?;
            tx.execute(
                "INSERT OR REPLACE INTO records (key, meta, trigrams) VALUES (?1, ?2, ?3)",
                params![key, meta, trigrams::encode(&record.trigrams)],
            )
            .map_err(sql)?;
        }
        tx.commit().map_err(sql)?;
        drop(db);
        Ok(fresh.into_iter().map(|(key, record)| self.remember(&key, record)).collect())
    }

    pub fn clock(&self) -> Clock {
        let value: Option<i64> = self
            .db()
            .query_row("SELECT value FROM meta WHERE name = 'clock'", [], |r| r.get(0))
            .optional()
            .ok()
            .flatten();
        Clock(value.unwrap_or(0) as u64)
    }

    pub fn save_clock(&self, clock: Clock) -> Result<()> {
        self.db()
            .execute("INSERT OR REPLACE INTO meta (name, value) VALUES ('clock', ?1)", [clock.0 as i64])
            .map_err(sql)?;
        Ok(())
    }

    fn remember(&self, key: &str, record: Record) -> Arc<Record> {
        let record = Arc::new(record);
        let mut live = self.live();
        if live.len() > 4096 && live.len().is_power_of_two() {
            live.retain(|_, weak| weak.strong_count() > 0);
        }
        live.insert(key.to_string(), Arc::downgrade(&record));
        record
    }

    fn db(&self) -> MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn live(&self) -> MutexGuard<'_, HashMap<String, Weak<Record>>> {
        self.live.lock().unwrap_or_else(|e| e.into_inner())
    }
}

fn sql(err: rusqlite::Error) -> Error {
    Error::internal(format!("index db: {err}"))
}
