//! Which items of a note are folded. Cosmetic and per machine: it lives in
//! the derived index and never in the file.

use super::Index;
use crate::Result;

impl Index {
    pub fn folds(&self, path: &str) -> Result<Vec<String>> {
        let mut st = self
            .conn
            .prepare("SELECT key FROM folds WHERE path=?1 ORDER BY key")?;
        let rows = st.query_map([path], |r| r.get(0))?;
        Ok(rows.collect::<std::result::Result<_, _>>()?)
    }

    pub fn set_folds(&mut self, path: &str, keys: &[String]) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM folds WHERE path=?1", [path])?;
        for k in keys {
            tx.execute(
                "INSERT OR IGNORE INTO folds(path, key) VALUES (?1, ?2)",
                [path, k],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn move_folds(&mut self, from: &str, to: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE OR REPLACE folds SET path=?2 WHERE path=?1",
            [from, to],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::index::Index;

    #[test]
    fn folds_round_trip_replace_and_move() {
        let mut ix = Index::open_in_memory().unwrap();
        assert!(ix.folds("a.md").unwrap().is_empty());
        ix.set_folds("a.md", &["0:1".into(), "h2".into()]).unwrap();
        assert_eq!(
            ix.folds("a.md").unwrap(),
            vec!["0:1".to_string(), "h2".to_string()]
        );
        ix.set_folds("a.md", &["0:2".into()]).unwrap();
        assert_eq!(ix.folds("a.md").unwrap(), vec!["0:2".to_string()]);
        ix.move_folds("a.md", "b.md").unwrap();
        assert!(ix.folds("a.md").unwrap().is_empty());
        assert_eq!(ix.folds("b.md").unwrap(), vec!["0:2".to_string()]);
    }
}
