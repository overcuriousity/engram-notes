//! Vectors in SQLite blobs and a brute-force cosine scan over them.

use crate::Result;
use crate::index::Index;
use rusqlite::{OptionalExtension, params};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pending {
    pub hash: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct VecHit {
    pub path: String,
    pub heading: String,
    /// 1-based line in the file.
    pub line: u32,
    pub text: String,
    pub similarity: f32,
}

fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

fn from_blob(b: &[u8]) -> Vec<f32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .copied()
        .map(f32::from_le_bytes)
        .collect()
}

// Vectors are stored normalised, so the cosine is a dot product.
fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

// The line a passage starts on in the file, not in the body.
const HIT_COLUMNS: &str = "p.path, p.heading, p.line + n.body_line - 1, p.text, v.embedding";

impl Index {
    pub fn model_id(&self) -> Result<Option<String>> {
        Ok(self
            .conn()
            .query_row("SELECT value FROM meta WHERE key='model'", [], |r| r.get(0))
            .optional()?)
    }

    /// Records the model. A different one makes every vector meaningless.
    pub fn set_model_id(&self, id: &str) -> Result<()> {
        if self.model_id()?.as_deref() != Some(id) {
            self.conn().execute("DELETE FROM vectors", [])?;
        }
        self.conn().execute(
            "INSERT OR REPLACE INTO meta(key, value) VALUES ('model', ?1)",
            [id],
        )?;
        Ok(())
    }

    /// Distinct passage texts with no vector yet, in a predictable order.
    pub fn pending_vectors(&self, limit: usize) -> Result<Vec<Pending>> {
        Ok(self
            .conn()
            .prepare(
                "SELECT p.hash, CASE WHEN p.heading = '' THEN p.text
                                     ELSE p.heading || char(10) || char(10) || p.text END
                 FROM passages p LEFT JOIN vectors v ON v.hash = p.hash
                 WHERE v.hash IS NULL
                 GROUP BY p.hash ORDER BY min(p.path), min(p.ordinal) LIMIT ?1",
            )?
            .query_map([limit as i64], |r| {
                Ok(Pending {
                    hash: r.get(0)?,
                    text: r.get(1)?,
                })
            })?
            .collect::<std::result::Result<_, _>>()?)
    }

    pub fn pending_count(&self) -> Result<usize> {
        let n: i64 = self.conn().query_row(
            "SELECT count(DISTINCT p.hash) FROM passages p
             LEFT JOIN vectors v ON v.hash = p.hash WHERE v.hash IS NULL",
            [],
            |r| r.get(0),
        )?;
        Ok(n as usize)
    }

    pub fn put_vectors(&mut self, rows: &[(String, Vec<f32>)]) -> Result<()> {
        let tx = self.conn_mut().transaction()?;
        {
            let mut ins = tx.prepare_cached(
                "INSERT OR REPLACE INTO vectors(hash, dim, embedding) VALUES (?1, ?2, ?3)",
            )?;
            for (hash, v) in rows {
                ins.execute(params![hash, v.len() as i64, to_blob(v)])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn vector_of(&self, hash: &str) -> Result<Option<Vec<f32>>> {
        Ok(self
            .conn()
            .query_row("SELECT embedding FROM vectors WHERE hash=?1", [hash], |r| {
                r.get::<_, Vec<u8>>(0)
            })
            .optional()?
            .map(|b| from_blob(&b)))
    }

    /// Every passage scored against `query`, best `limit` first.
    pub fn search_vectors(&self, query: &[f32], limit: usize) -> Result<Vec<VecHit>> {
        let sql = format!(
            "SELECT {HIT_COLUMNS} FROM passages p
             JOIN vectors v ON v.hash = p.hash JOIN notes n ON n.path = p.path"
        );
        let mut hits: Vec<VecHit> = self
            .conn()
            .prepare(&sql)?
            .query_map([], |r| {
                let blob: Vec<u8> = r.get(4)?;
                Ok(VecHit {
                    path: r.get(0)?,
                    heading: r.get(1)?,
                    line: r.get(2)?,
                    text: r.get(3)?,
                    similarity: dot(query, &from_blob(&blob)),
                })
            })?
            .collect::<std::result::Result<_, _>>()?;
        hits.sort_by(|a, b| b.similarity.total_cmp(&a.similarity));
        hits.truncate(limit);
        Ok(hits)
    }

    /// Passages of other notes nearest to any passage of `paths`, one per note,
    /// and only those near enough to mean something.
    pub fn similar_to(&self, paths: &[String], limit: usize, floor: f32) -> Result<Vec<VecHit>> {
        let mut mine: Vec<Vec<f32>> = Vec::new();
        {
            let mut stmt = self.conn().prepare(
                "SELECT v.embedding FROM passages p JOIN vectors v ON v.hash = p.hash WHERE p.path = ?1",
            )?;
            for path in paths {
                for row in stmt.query_map([path], |r| r.get::<_, Vec<u8>>(0))? {
                    mine.push(from_blob(&row?));
                }
            }
        }
        if mine.is_empty() {
            return Ok(vec![]);
        }
        let sql = format!(
            "SELECT {HIT_COLUMNS} FROM passages p
             JOIN vectors v ON v.hash = p.hash JOIN notes n ON n.path = p.path"
        );
        let mut best: std::collections::HashMap<String, VecHit> = std::collections::HashMap::new();
        let mut stmt = self.conn().prepare(&sql)?;
        let rows = stmt.query_map([], |r| {
            let blob: Vec<u8> = r.get(4)?;
            let v = from_blob(&blob);
            Ok(VecHit {
                path: r.get(0)?,
                heading: r.get(1)?,
                line: r.get(2)?,
                text: r.get(3)?,
                similarity: mine.iter().map(|m| dot(m, &v)).fold(f32::MIN, f32::max),
            })
        })?;
        for row in rows {
            let hit = row?;
            if paths.contains(&hit.path) {
                continue;
            }
            match best.get(&hit.path) {
                Some(old) if old.similarity >= hit.similarity => {}
                _ => {
                    best.insert(hit.path.clone(), hit);
                }
            }
        }
        let mut out: Vec<VecHit> = best.into_values().collect();
        out.sort_by(|a, b| {
            b.similarity
                .total_cmp(&a.similarity)
                .then(a.path.cmp(&b.path))
        });
        out.retain(|h| h.similarity >= floor);
        out.truncate(limit);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use crate::embed::{Embedder, FakeEmbedder};
    use crate::index::Index;
    use crate::vault::Vault;
    use std::fs;

    fn embedded() -> (tempfile::TempDir, Index) {
        let d = tempfile::tempdir().unwrap();
        fs::write(
            d.path().join("Rust.md"),
            "# Rust\nownership rules keep memory safe",
        )
        .unwrap();
        fs::write(
            d.path().join("Coffee.md"),
            "# Coffee\nkettle water grind beans",
        )
        .unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let mut e = FakeEmbedder::new(64);
        ix.set_model_id(&e.id()).unwrap();
        let pending = ix.pending_vectors(100).unwrap();
        let texts: Vec<String> = pending.iter().map(|p| p.text.clone()).collect();
        let vecs = e.embed_documents(&texts).unwrap();
        let rows: Vec<(String, Vec<f32>)> = pending.into_iter().map(|p| p.hash).zip(vecs).collect();
        ix.put_vectors(&rows).unwrap();
        (d, ix)
    }

    #[test]
    fn pending_lists_each_passage_once_and_empties_as_vectors_arrive() {
        let (_d, ix) = embedded();
        assert_eq!(ix.pending_count().unwrap(), 0);
        assert!(ix.pending_vectors(10).unwrap().is_empty());
    }

    #[test]
    fn the_scan_ranks_by_cosine_and_carries_the_passage() {
        let (_d, ix) = embedded();
        let mut e = FakeEmbedder::new(64);
        let q = e.embed_query("ownership rules").unwrap();
        let hits = ix.search_vectors(&q, 10).unwrap();
        assert_eq!(hits[0].path, "Rust.md");
        assert_eq!(hits[0].heading, "");
        assert_eq!(hits[0].line, 1);
        assert!(hits[0].text.contains("ownership"));
        assert!(hits[0].similarity > hits[1].similarity);
    }

    #[test]
    fn a_new_model_id_clears_the_vectors() {
        let (_d, ix) = embedded();
        ix.set_model_id("other").unwrap();
        assert_eq!(ix.model_id().unwrap().as_deref(), Some("other"));
        assert!(ix.pending_count().unwrap() > 0);
    }

    #[test]
    fn similar_to_skips_the_note_itself() {
        let (_d, ix) = embedded();
        let hits = ix.similar_to(&["Rust.md".to_string()], 10, 0.0).unwrap();
        assert!(hits.iter().all(|h| h.path != "Rust.md"));
        assert_eq!(hits[0].path, "Coffee.md");
    }

    #[test]
    fn a_deleted_note_leaves_no_passage_behind() {
        let (d, mut ix) = embedded();
        fs::remove_file(d.path().join("Coffee.md")).unwrap();
        let v = Vault::open(d.path()).unwrap();
        ix.rebuild(&v).unwrap();
        let mut e = FakeEmbedder::new(64);
        let q = e.embed_query("kettle").unwrap();
        assert!(
            ix.search_vectors(&q, 10)
                .unwrap()
                .iter()
                .all(|h| h.path != "Coffee.md")
        );
    }
}
