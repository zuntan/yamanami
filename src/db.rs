use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};

#[derive(Clone)]
pub struct Database(Arc<Mutex<Connection>>);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct App {
    pub name: String,
    pub path: String,
    pub release: i64,
    pub sha256: String,
}

impl Database {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let connection = Connection::open(path)?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA foreign_keys=ON;
             PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS modules (
                 sha256 TEXT PRIMARY KEY, wasm BLOB NOT NULL
             );
             CREATE TABLE IF NOT EXISTS applications (
                 name TEXT PRIMARY KEY,
                 active_release INTEGER REFERENCES releases(id)
             );
             CREATE TABLE IF NOT EXISTS releases (
                 id INTEGER PRIMARY KEY,
                 application TEXT NOT NULL REFERENCES applications(name),
                 created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );
             CREATE TABLE IF NOT EXISTS release_modules (
                 release_id INTEGER NOT NULL REFERENCES releases(id),
                 name TEXT NOT NULL,
                 sha256 TEXT NOT NULL REFERENCES modules(sha256),
                 PRIMARY KEY(release_id, name)
             );
             CREATE TABLE IF NOT EXISTS routes (
                 application TEXT PRIMARY KEY REFERENCES applications(name),
                 path TEXT NOT NULL UNIQUE
             );
             CREATE TABLE IF NOT EXISTS deployment_events (
                 id INTEGER PRIMARY KEY,
                 application TEXT NOT NULL REFERENCES applications(name),
                 release_id INTEGER NOT NULL REFERENCES releases(id),
                 created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
             );",
        )?;
        Ok(Self(Arc::new(Mutex::new(connection))))
    }

    // SQLite never runs on an async I/O worker. No database lock crosses an await.
    pub async fn active(&self) -> Result<Vec<(App, Vec<u8>)>> {
        let db = self.0.clone();
        tokio::task::spawn_blocking(move || -> Result<_> {
            let connection = db
                .lock()
                .map_err(|_| anyhow::anyhow!("database lock poisoned"))?;
            let mut query = connection.prepare(
                "SELECT a.name, rt.path, a.active_release, m.sha256, m.wasm
                 FROM applications a
                 JOIN routes rt ON rt.application=a.name
                 JOIN release_modules rm ON rm.release_id=a.active_release AND rm.name='main'
                 JOIN modules m ON m.sha256=rm.sha256 ORDER BY a.name",
            )?;
            let rows = query.query_map([], |row| {
                Ok((
                    App {
                        name: row.get(0)?,
                        path: row.get(1)?,
                        release: row.get(2)?,
                        sha256: row.get(3)?,
                    },
                    row.get(4)?,
                ))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
                .context("load active releases")
        })
        .await?
    }

    pub async fn deploy(
        &self,
        name: String,
        path: String,
        hash: String,
        wasm: Vec<u8>,
    ) -> Result<App> {
        let db = self.0.clone();
        tokio::task::spawn_blocking(move || -> Result<_> {
            let mut connection = db.lock().map_err(|_| anyhow::anyhow!("database lock poisoned"))?;
            let tx = connection.transaction()?;
            tx.execute("INSERT OR IGNORE INTO modules VALUES (?1, ?2)", params![hash, wasm])?;
            tx.execute("INSERT OR IGNORE INTO applications(name) VALUES (?1)", [&name])?;
            tx.execute("INSERT INTO releases(application) VALUES (?1)", [&name])?;
            let release = tx.last_insert_rowid();
            tx.execute("INSERT INTO release_modules VALUES (?1, 'main', ?2)", params![release, hash])?;
            tx.execute(
                "INSERT INTO routes VALUES (?1, ?2) ON CONFLICT(application) DO UPDATE SET path=excluded.path",
                params![name, path],
            )?;
            tx.execute("UPDATE applications SET active_release=?1 WHERE name=?2", params![release, name])?;
            tx.execute("INSERT INTO deployment_events(application, release_id) VALUES (?1, ?2)",
                params![name, release])?;
            tx.commit()?;
            Ok(App { name, path, release, sha256: hash })
        }).await?
    }

    pub async fn path_owner(&self, path: String) -> Result<Option<String>> {
        let db = self.0.clone();
        tokio::task::spawn_blocking(move || -> Result<_> {
            let conn = db
                .lock()
                .map_err(|_| anyhow::anyhow!("database lock poisoned"))?;
            Ok(conn
                .query_row(
                    "SELECT application FROM routes WHERE path=?1",
                    [path],
                    |row| row.get(0),
                )
                .optional()?)
        })
        .await?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn persistence_dedup_and_failed_transaction() -> Result<()> {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("test.db");
        let db = Database::open(&path)?;
        let first = db
            .deploy("a".into(), "/a".into(), "hash".into(), vec![1])
            .await?;
        let second = db
            .deploy("b".into(), "/b".into(), "hash".into(), vec![1])
            .await?;
        assert_ne!(first.release, second.release);
        assert!(
            db.deploy("b".into(), "/a".into(), "other".into(), vec![2])
                .await
                .is_err()
        );
        let active = Database::open(&path)?.active().await?;
        assert_eq!(active.len(), 2);
        assert_eq!(active[1].0.path, "/b");
        let conn = db.0.lock().unwrap();
        for (table, expected) in [("modules", 1), ("releases", 2), ("deployment_events", 2)] {
            let count: i64 =
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
            assert_eq!(count, expected);
        }
        Ok(())
    }
}
