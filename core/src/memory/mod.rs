//! A small slice of engram: events in sittings, activation and associations,
//! every value stored as `(value, stamped_at)` and read through `decayed`.

pub mod decay;
pub mod prime;
pub mod related;
pub mod spread;

use crate::Result;
use crate::config::MemoryConfig;
use crate::index::Index;
use decay::decayed;
use rusqlite::{OptionalExtension, params};
use std::collections::HashMap;

/// At most this many cues per link, the busiest kept.
const MAX_CUES: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Open,
    OpenFromSearch,
    FollowLink,
    Search,
}

impl EventKind {
    fn as_str(self) -> &'static str {
        match self {
            EventKind::Open => "open",
            EventKind::OpenFromSearch => "open_from_search",
            EventKind::FollowLink => "follow_link",
            EventKind::Search => "search",
        }
    }

    /// The kinds that say the user arrived at a note.
    fn is_arrival(self) -> bool {
        matches!(
            self,
            EventKind::Open | EventKind::OpenFromSearch | EventKind::FollowLink
        )
    }
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct AssocRow {
    /// The note the link was read from.
    pub via: String,
    pub other: String,
    /// Already decayed to the caller's clock.
    pub value: f64,
    pub cue: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Cue {
    q: String,
    n: u32,
}

fn bump_cue(cues: &mut Vec<Cue>, q: &str) {
    match cues.iter_mut().find(|c| c.q == q) {
        Some(c) => c.n += 1,
        None => cues.push(Cue {
            q: q.to_owned(),
            n: 1,
        }),
    }
    cues.sort_by_key(|c| std::cmp::Reverse(c.n));
    cues.truncate(MAX_CUES);
}

/// Pairs are stored with `a < b`, so a link is one row whichever way it is seen.
fn pair<'a>(a: &'a str, b: &'a str) -> (&'a str, &'a str) {
    if a < b { (a, b) } else { (b, a) }
}

impl Index {
    /// Appends the event, then learns from it: activation for the note, an
    /// association with every note reached shortly before it in this sitting.
    pub fn record_event(
        &mut self,
        kind: EventKind,
        path: Option<&str>,
        query: Option<&str>,
        cfg: &MemoryConfig,
        at: i64,
    ) -> Result<()> {
        let last: Option<(i64, i64)> = self
            .conn()
            .query_row(
                "SELECT at, sitting FROM events ORDER BY rowid DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let sitting = match last {
            Some((last_at, s)) if at - last_at <= cfg.sitting_gap_secs => s,
            Some((_, s)) => s + 1,
            None => 1,
        };
        self.conn().execute(
            "INSERT INTO events(at, kind, path, query, sitting) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![at, kind.as_str(), path, query, sitting],
        )?;
        let (Some(path), true) = (path, kind.is_arrival()) else {
            return Ok(());
        };
        self.bump_activation(path, 1.0, cfg, at)?;
        // Both spec rules in one pass: opened from the same search, or reached
        // in the same sitting within the window. One row per note however many
        // times it was reached; the second column is our query when that note
        // was reached under it too, which is exactly the cue.
        let recent: Vec<(String, Option<String>)> = self
            .conn()
            .prepare(
                "SELECT path, max(CASE WHEN query = ?5 THEN query END) FROM events
                 WHERE sitting = ?1 AND at >= ?2 AND at <= ?3 AND path IS NOT NULL AND path <> ?4
                   AND kind IN ('open', 'open_from_search', 'follow_link')
                 GROUP BY path",
            )?
            .query_map(
                params![sitting, at - cfg.assoc_window_secs, at, path, query],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )?
            .collect::<std::result::Result<_, _>>()?;
        for (other, cue) in recent {
            self.bump_assoc(path, &other, 1.0, cue.as_deref(), cfg, at)?;
        }
        Ok(())
    }

    /// Read the row, decay it to now, add the delta, write it back with a fresh
    /// stamp: no stale number is ever added to.
    pub fn bump_activation(
        &mut self,
        path: &str,
        delta: f64,
        cfg: &MemoryConfig,
        at: i64,
    ) -> Result<()> {
        let old: Option<(f64, i64)> = self
            .conn()
            .query_row(
                "SELECT value, stamped_at FROM activation WHERE path=?1",
                [path],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;
        let value = match old {
            Some((v, s)) => decayed(v, s, at, cfg.activation_half_life_days) + delta,
            None => delta,
        };
        self.conn().execute(
            "INSERT OR REPLACE INTO activation(path, value, stamped_at) VALUES (?1, ?2, ?3)",
            params![path, value, at],
        )?;
        Ok(())
    }

    pub fn bump_assoc(
        &mut self,
        a: &str,
        b: &str,
        delta: f64,
        cue: Option<&str>,
        cfg: &MemoryConfig,
        at: i64,
    ) -> Result<()> {
        let (a, b) = pair(a, b);
        let old: Option<(f64, i64, String)> = self
            .conn()
            .query_row(
                "SELECT value, stamped_at, queries FROM assoc WHERE a_path=?1 AND b_path=?2",
                [a, b],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .optional()?;
        let (value, mut cues) = match old {
            Some((v, s, q)) => (
                decayed(v, s, at, cfg.assoc_half_life_days) + delta,
                serde_json::from_str::<Vec<Cue>>(&q).unwrap_or_default(),
            ),
            None => (delta, Vec::new()),
        };
        if let Some(q) = cue.map(str::trim).filter(|q| !q.is_empty()) {
            bump_cue(&mut cues, q);
        }
        let queries = serde_json::to_string(&cues).unwrap_or_else(|_| "[]".into());
        self.conn().execute(
            "INSERT OR REPLACE INTO assoc(a_path, b_path, value, stamped_at, queries)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![a, b, value, at, queries],
        )?;
        Ok(())
    }

    /// Every note's activation, decayed to `at`. Notes that are gone are left out.
    pub fn activation_map(&self, at: i64, half_life_days: f64) -> Result<HashMap<String, f64>> {
        Ok(self
            .conn()
            .prepare(
                "SELECT a.path, a.value, a.stamped_at FROM activation a
                 JOIN notes n ON n.path = a.path",
            )?
            .query_map([], |r| {
                let path: String = r.get(0)?;
                let value: f64 = r.get(1)?;
                let stamped: i64 = r.get(2)?;
                Ok((path, decayed(value, stamped, at, half_life_days)))
            })?
            .collect::<std::result::Result<_, _>>()?)
    }

    /// Links from any of `paths`, strongest first, only those above the show
    /// threshold and only to notes that still exist.
    pub fn assoc_from(
        &self,
        paths: &[String],
        at: i64,
        cfg: &MemoryConfig,
    ) -> Result<Vec<AssocRow>> {
        let mut out = Vec::new();
        let mut stmt = self.conn().prepare(
            "SELECT CASE WHEN a_path = ?1 THEN b_path ELSE a_path END, value, stamped_at, queries
             FROM assoc JOIN notes n ON n.path = CASE WHEN a_path = ?1 THEN b_path ELSE a_path END
             WHERE a_path = ?1 OR b_path = ?1",
        )?;
        for via in paths {
            for row in stmt.query_map([via], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, f64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })? {
                let (other, value, stamped, queries) = row?;
                let value = decayed(value, stamped, at, cfg.assoc_half_life_days);
                if value < cfg.assoc_show {
                    continue;
                }
                let cue = serde_json::from_str::<Vec<Cue>>(&queries)
                    .unwrap_or_default()
                    .into_iter()
                    .next()
                    .map(|c| c.q);
                out.push(AssocRow {
                    via: via.clone(),
                    other,
                    value,
                    cue,
                });
            }
        }
        out.sort_by(|a, b| b.value.total_cmp(&a.value).then(a.other.cmp(&b.other)));
        Ok(out)
    }

    /// A renamed note keeps what it learned.
    pub fn move_memory(&mut self, from: &str, to: &str) -> Result<()> {
        let tx = self.conn_mut().transaction()?;
        tx.execute(
            "UPDATE OR REPLACE activation SET path=?2 WHERE path=?1",
            [from, to],
        )?;
        tx.execute(
            "UPDATE OR REPLACE assoc SET a_path=?2 WHERE a_path=?1",
            [from, to],
        )?;
        tx.execute(
            "UPDATE OR REPLACE assoc SET b_path=?2 WHERE b_path=?1",
            [from, to],
        )?;
        // The pair order is part of the key; a move can break it.
        tx.execute(
            "UPDATE assoc SET a_path = b_path, b_path = a_path WHERE a_path > b_path",
            [],
        )?;
        tx.execute("UPDATE events SET path=?2 WHERE path=?1", [from, to])?;
        tx.commit()?;
        Ok(())
    }

    pub fn forget_memory(&mut self) -> Result<()> {
        self.conn()
            .execute_batch("DELETE FROM activation; DELETE FROM assoc; DELETE FROM events;")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MemoryConfig;
    use crate::index::Index;
    use crate::vault::Vault;
    use std::fs;

    const DAY: i64 = 86_400;

    fn ix() -> (tempfile::TempDir, Index) {
        let d = tempfile::tempdir().unwrap();
        for name in ["A", "B", "C"] {
            fs::write(
                d.path().join(format!("{name}.md")),
                format!("# {name}\nbody"),
            )
            .unwrap();
        }
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        (d, ix)
    }

    // A link shows from 0.5 here, so one co-appearance is enough to read back.
    fn cfg() -> MemoryConfig {
        MemoryConfig {
            assoc_show: 0.5,
            ..MemoryConfig::default()
        }
    }

    fn sittings(ix: &Index) -> Vec<i64> {
        ix.conn()
            .prepare("SELECT sitting FROM events ORDER BY rowid")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn silence_longer_than_the_gap_starts_a_new_sitting() {
        let (_d, mut ix) = ix();
        let cfg = MemoryConfig::default();
        ix.record_event(EventKind::Open, Some("A.md"), None, &cfg, 0)
            .unwrap();
        ix.record_event(EventKind::Open, Some("B.md"), None, &cfg, 60)
            .unwrap();
        ix.record_event(EventKind::Open, Some("C.md"), None, &cfg, 60 + 1801)
            .unwrap();
        assert_eq!(sittings(&ix), vec![1, 1, 2]);
    }

    #[test]
    fn opening_bumps_activation_and_it_decays() {
        let (_d, mut ix) = ix();
        let cfg = MemoryConfig::default();
        ix.record_event(EventKind::Open, Some("A.md"), None, &cfg, 0)
            .unwrap();
        ix.record_event(EventKind::Open, Some("A.md"), None, &cfg, 10)
            .unwrap();
        let now = ix
            .activation_map(10, cfg.activation_half_life_days)
            .unwrap();
        assert!((now["A.md"] - 2.0).abs() < 1e-4, "{}", now["A.md"]);
        let later = ix
            .activation_map(30 * DAY, cfg.activation_half_life_days)
            .unwrap();
        assert!((later["A.md"] - 1.0).abs() < 1e-3);
    }

    #[test]
    fn two_notes_opened_in_one_sitting_are_associated_with_their_cue() {
        let (_d, mut ix) = ix();
        let cfg = cfg();
        ix.record_event(
            EventKind::OpenFromSearch,
            Some("A.md"),
            Some("rust"),
            &cfg,
            0,
        )
        .unwrap();
        ix.record_event(
            EventKind::OpenFromSearch,
            Some("B.md"),
            Some("rust"),
            &cfg,
            30,
        )
        .unwrap();
        let rows = ix.assoc_from(&["A.md".into()], 30, &cfg).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].other, "B.md");
        assert_eq!(rows[0].cue.as_deref(), Some("rust"));
        assert_eq!(rows[0].value, 1.0);
    }

    #[test]
    fn a_gap_wider_than_the_window_associates_nothing() {
        let (_d, mut ix) = ix();
        let cfg = cfg();
        ix.record_event(EventKind::Open, Some("A.md"), None, &cfg, 0)
            .unwrap();
        ix.record_event(EventKind::Open, Some("B.md"), None, &cfg, 601)
            .unwrap();
        assert!(
            ix.assoc_from(&["A.md".into()], 601, &cfg)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_pair_is_stored_once_whichever_way_round_it_is_seen() {
        let (_d, mut ix) = ix();
        let cfg = cfg();
        for (at, path) in [(0, "B.md"), (10, "A.md"), (20, "B.md")] {
            ix.record_event(EventKind::Open, Some(path), None, &cfg, at)
                .unwrap();
        }
        let n: i64 = ix
            .conn()
            .query_row("SELECT count(*) FROM assoc", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 1);
        let a: String = ix
            .conn()
            .query_row("SELECT a_path FROM assoc", [], |r| r.get(0))
            .unwrap();
        assert_eq!(a, "A.md");
        assert_eq!(
            ix.assoc_from(&["B.md".into()], 20, &cfg).unwrap()[0].other,
            "A.md"
        );
    }

    #[test]
    fn only_three_cues_survive_the_busiest_first() {
        let (_d, mut ix) = ix();
        let cfg = cfg();
        for (i, q) in ["one", "two", "three", "four", "two"].iter().enumerate() {
            let at = i as i64 * 20;
            ix.record_event(EventKind::OpenFromSearch, Some("A.md"), Some(q), &cfg, at)
                .unwrap();
            ix.record_event(
                EventKind::OpenFromSearch,
                Some("B.md"),
                Some(q),
                &cfg,
                at + 1,
            )
            .unwrap();
        }
        let raw: String = ix
            .conn()
            .query_row("SELECT queries FROM assoc", [], |r| r.get(0))
            .unwrap();
        let cues: Vec<serde_json::Value> = serde_json::from_str(&raw).unwrap();
        assert_eq!(cues.len(), 3);
        assert_eq!(cues[0]["q"], "two");
    }

    #[test]
    fn forgetting_empties_memory_and_leaves_the_notes() {
        let (_d, mut ix) = ix();
        let cfg = MemoryConfig::default();
        ix.record_event(EventKind::Open, Some("A.md"), None, &cfg, 0)
            .unwrap();
        ix.forget_memory().unwrap();
        assert!(ix.activation_map(0, 30.0).unwrap().is_empty());
        let n: i64 = ix
            .conn()
            .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
        let notes: i64 = ix
            .conn()
            .query_row("SELECT count(*) FROM notes", [], |r| r.get(0))
            .unwrap();
        assert_eq!(notes, 3);
    }

    // Both reads join `notes`, so the file has to move with the memory.
    #[test]
    fn memory_follows_a_renamed_note() {
        let (d, mut ix) = ix();
        let cfg = cfg();
        ix.record_event(EventKind::Open, Some("A.md"), None, &cfg, 0)
            .unwrap();
        ix.record_event(EventKind::Open, Some("B.md"), None, &cfg, 10)
            .unwrap();
        fs::create_dir_all(d.path().join("sub")).unwrap();
        fs::rename(d.path().join("A.md"), d.path().join("sub/A.md")).unwrap();
        let v = Vault::open(d.path()).unwrap();
        ix.rebuild(&v).unwrap();
        ix.move_memory("A.md", "sub/A.md").unwrap();
        assert!(
            ix.activation_map(10, 30.0)
                .unwrap()
                .contains_key("sub/A.md")
        );
        assert_eq!(
            ix.assoc_from(&["B.md".into()], 10, &cfg).unwrap()[0].other,
            "sub/A.md"
        );
    }
}
