//! The derived view of a vault. Rebuildable from the files at any time.

pub mod fts;
pub mod query;
pub mod rebuild;
pub mod resolve;

use crate::{Error, Result};
use rusqlite::{Connection, ErrorCode};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: &str = "2";
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
        create_parent(path)?;
        Ok(Index::connect(path)?)
    }

    /// Like `open`, but a corrupt file is deleted and created afresh; the flag
    /// says whether that happened.
    pub fn open_or_recreate(path: &Path) -> Result<(Index, bool)> {
        create_parent(path)?;
        match Index::connect(path) {
            Ok(ix) => Ok((ix, false)),
            Err(e) if is_corrupt(&e) => {
                for suffix in ["", "-wal", "-shm"] {
                    let mut name = path.as_os_str().to_owned();
                    name.push(suffix);
                    let p = PathBuf::from(name);
                    match std::fs::remove_file(&p) {
                        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                            return Err(Error::io(p, e));
                        }
                        _ => {}
                    }
                }
                Ok((Index::connect(path)?, true))
            }
            Err(e) => Err(e.into()),
        }
    }

    pub fn open_in_memory() -> Result<Index> {
        let mut ix = Index {
            conn: Connection::open_in_memory()?,
        };
        ix.prepare()?;
        Ok(ix)
    }

    fn connect(path: &Path) -> rusqlite::Result<Index> {
        let mut ix = Index {
            conn: Connection::open(path)?,
        };
        let check: String = ix.conn.query_row("PRAGMA quick_check", [], |r| r.get(0))?;
        if check != "ok" {
            let code = rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_CORRUPT);
            return Err(rusqlite::Error::SqliteFailure(code, Some(check)));
        }
        ix.prepare()?;
        Ok(ix)
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    fn prepare(&mut self) -> rusqlite::Result<()> {
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
    fn drop_all(&mut self) -> rusqlite::Result<()> {
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

fn create_parent(path: &Path) -> Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
    }
    Ok(())
}

fn is_corrupt(e: &rusqlite::Error) -> bool {
    matches!(
        e.sqlite_error_code(),
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase)
    )
}

impl From<rusqlite::Error> for Error {
    fn from(e: rusqlite::Error) -> Self {
        Error::Index(e.to_string())
    }
}
