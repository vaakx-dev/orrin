use orrin_core::model::{Project, ProjectSettings};
use orrin_core::{PcId, ProjectId};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::db::{hash, json, to_json};

const COLUMNS: &str = "id, name, home_pc, main, settings, created_at";

fn project(row: &Row) -> rusqlite::Result<Project> {
    Ok(Project {
        id: ProjectId(row.get(0)?),
        name: row.get(1)?,
        home_pc: PcId(row.get(2)?),
        main: hash(row, 3)?,
        settings: json(row, 4)?,
        created_at: row.get(5)?,
    })
}

pub fn get(conn: &Connection, id: &ProjectId) -> rusqlite::Result<Option<Project>> {
    conn.query_row(&format!("SELECT {COLUMNS} FROM projects WHERE id = ?1"), [id.as_str()], project).optional()
}

pub fn list(conn: &Connection) -> rusqlite::Result<Vec<Project>> {
    let mut stmt = conn.prepare(&format!("SELECT {COLUMNS} FROM projects ORDER BY created_at, id"))?;
    stmt.query_map([], project)?.collect()
}

pub fn insert(conn: &Connection, p: &Project) -> rusqlite::Result<()> {
    conn.execute(
        &format!("INSERT INTO projects ({COLUMNS}) VALUES (?1, ?2, ?3, ?4, ?5, ?6)"),
        params![p.id.as_str(), p.name, p.home_pc.as_str(), p.main.as_str(), to_json(&p.settings), p.created_at],
    )?;
    Ok(())
}

pub fn update(conn: &Connection, p: &Project) -> rusqlite::Result<bool> {
    let changed = conn.execute(
        "UPDATE projects SET name = ?2, home_pc = ?3, main = ?4, settings = ?5, created_at = ?6 WHERE id = ?1",
        params![p.id.as_str(), p.name, p.home_pc.as_str(), p.main.as_str(), to_json(&p.settings), p.created_at],
    )?;
    Ok(changed > 0)
}

pub fn set_settings(conn: &Connection, id: &ProjectId, settings: &ProjectSettings) -> rusqlite::Result<bool> {
    let changed =
        conn.execute("UPDATE projects SET settings = ?2 WHERE id = ?1", params![id.as_str(), to_json(settings)])?;
    Ok(changed > 0)
}

pub fn delete(conn: &Connection, id: &ProjectId) -> rusqlite::Result<()> {
    conn.execute("DELETE FROM projects WHERE id = ?1", [id.as_str()])?;
    Ok(())
}
