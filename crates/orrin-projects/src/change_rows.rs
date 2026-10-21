use orrin_core::model::Change;
use orrin_core::{ChangeId, ProjectId};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::db::{json, to_json};
use crate::thin::Dated;

fn change(row: &Row) -> rusqlite::Result<Change> {
    let mut change: Change = json(row, 0)?;
    change.pinned = row.get(1)?;
    Ok(change)
}

pub fn insert(conn: &Connection, c: &Change) -> rusqlite::Result<bool> {
    let added = conn.execute(
        "INSERT OR IGNORE INTO changes (id, project, at, pinned, body) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![c.id.as_str(), c.project.as_str(), c.at, c.pinned, to_json(c)],
    )?;
    if added == 0 {
        return Ok(false);
    }
    let mut stmt = conn.prepare_cached("INSERT INTO change_paths (change, path) VALUES (?1, ?2)")?;
    for path in c.files.keys() {
        stmt.execute(params![c.id.as_str(), path])?;
    }
    Ok(true)
}

pub fn get(conn: &Connection, id: &ChangeId) -> rusqlite::Result<Option<Change>> {
    conn.query_row("SELECT body, pinned FROM changes WHERE id = ?1", [id.as_str()], change).optional()
}

pub struct LogQuery<'a> {
    pub project: &'a ProjectId,
    pub path: Option<&'a str>,
    pub before: Option<&'a ChangeId>,
    pub limit: u32,
}

pub fn log(conn: &Connection, q: &LogQuery) -> rusqlite::Result<Vec<Change>> {
    let mut stmt = conn.prepare(
        "SELECT body, pinned FROM changes c
         WHERE c.project = ?1
           AND (?2 IS NULL OR EXISTS (
               SELECT 1 FROM change_paths p
               WHERE p.change = c.id AND (p.path = ?2 OR substr(p.path, 1, length(?2) + 1) = ?2 || '/')))
           AND (?3 IS NULL OR c.seq < (SELECT seq FROM changes WHERE id = ?3))
         ORDER BY c.seq DESC
         LIMIT ?4",
    )?;
    let rows = stmt.query_map(params![q.project.as_str(), q.path, q.before.map(ChangeId::as_str), q.limit], change)?;
    rows.collect()
}

pub fn pin(conn: &Connection, id: &ChangeId, pinned: bool) -> rusqlite::Result<bool> {
    Ok(conn.execute("UPDATE changes SET pinned = ?2 WHERE id = ?1", params![id.as_str(), pinned])? > 0)
}

pub fn dated(conn: &Connection, project: &ProjectId) -> rusqlite::Result<Vec<Dated>> {
    let mut stmt = conn.prepare("SELECT id, at, pinned FROM changes WHERE project = ?1 ORDER BY seq DESC")?;
    let rows = stmt.query_map([project.as_str()], |row| {
        Ok(Dated { id: ChangeId(row.get(0)?), at: row.get(1)?, pinned: row.get(2)? })
    })?;
    rows.collect()
}

pub fn delete_project(conn: &Connection, project: &ProjectId) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM changes WHERE project = ?1", [project.as_str()])?;
    Ok(())
}

pub fn delete(conn: &Connection, id: &ChangeId) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM changes WHERE id = ?1", [id.as_str()])?;
    Ok(())
}
