//! The derived view of a vault. Rebuildable from the files at any time.

pub mod rebuild;

use crate::{Error, Result};
use rusqlite::Connection;
use std::path::Path;

pub const SCHEMA_VERSION: &str = "1";
const SCHEMA: &str = include_str!("schema.sql");

pub struct Index {
    conn: Connection,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct RebuildStats {
    pub added: usize,
    pub updated: usize,
    pub removed: usize,
    pub unchanged: usize,
}

impl Index {
    pub fn open(path: &Path) -> Result<Index> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        let mut ix = Index {
            conn: Connection::open(path)?,
        };
        ix.prepare()?;
        Ok(ix)
    }

    pub fn open_in_memory() -> Result<Index> {
        let mut ix = Index {
            conn: Connection::open_in_memory()?,
        };
        ix.prepare()?;
        Ok(ix)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    fn prepare(&mut self) -> Result<()> {
        self.conn.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA synchronous=NORMAL;",
        )?;
        let has_meta: i64 = self.conn.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='meta'",
            [],
            |r| r.get(0),
        )?;
        if has_meta > 0 {
            let version: Option<String> = self
                .conn
                .query_row("SELECT value FROM meta WHERE key='schema'", [], |r| {
                    r.get(0)
                })
                .ok();
            if version.as_deref() != Some(SCHEMA_VERSION) {
                self.drop_all()?;
            }
        }
        self.conn.execute_batch(SCHEMA)?;
        self.conn.execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES ('schema', ?1)",
            [SCHEMA_VERSION],
        )?;
        Ok(())
    }

    // Files are the truth: a schema change throws the derived state away.
    fn drop_all(&mut self) -> Result<()> {
        let names: Vec<(String, String)> = self
            .conn
            .prepare(
                "SELECT type, name FROM sqlite_master WHERE type IN ('table','view') AND name NOT LIKE 'sqlite_%'",
            )?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<std::result::Result<_, _>>()?;
        self.conn.execute_batch("PRAGMA foreign_keys=OFF;")?;
        for (kind, name) in names {
            self.conn
                .execute_batch(&format!("DROP {kind} IF EXISTS \"{name}\";"))?;
        }
        self.conn.execute_batch("PRAGMA foreign_keys=ON;")?;
        Ok(())
    }
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Index(e.to_string())
    }
}

impl Index {
    pub fn resolve_all(&mut self) -> Result<()> {
        Ok(())
    }
}
