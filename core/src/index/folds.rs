//! Which items of a note are folded. Cosmetic and per machine: it lives in
//! the derived index and never in the file.

use super::Index;
use crate::Result;
use rusqlite::params;

/// Drops the fold state of `rel`, or of every note below it when it names a
/// folder. Fold keys are positional, so rows left behind would be applied to
/// whatever takes the path next.
pub(super) fn drop_folds(conn: &rusqlite::Connection, rel: &str) -> Result<()> {
    let dir = format!("{}/", rel.trim_end_matches('/'));
    conn.execute(
        "DELETE FROM folds WHERE path=?1 OR substr(path, 1, length(?2))=?2",
        params![rel, dir],
    )?;
    Ok(())
}

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

    /// Carries fold state to `to`, replacing whatever was there: a merge would
    /// fold items of the new note that were never folded.
    pub fn move_folds(&mut self, from: &str, to: &str) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM folds WHERE path=?1", [to])?;
        tx.execute("UPDATE folds SET path=?2 WHERE path=?1", [from, to])?;
        tx.commit()?;
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

    #[test]
    fn a_move_replaces_the_destination_rather_than_merging_into_it() {
        let mut ix = Index::open_in_memory().unwrap();
        ix.set_folds("a.md", &["0:0".into()]).unwrap();
        ix.set_folds("b.md", &["h1".into()]).unwrap();
        ix.move_folds("a.md", "b.md").unwrap();
        assert_eq!(ix.folds("b.md").unwrap(), vec!["0:0".to_string()]);
    }

    #[test]
    fn removing_a_note_or_its_folder_takes_the_folds_with_it() {
        let mut ix = Index::open_in_memory().unwrap();
        ix.set_folds("sub/a.md", &["0:0".into()]).unwrap();
        ix.set_folds("sub/b.md", &["h1".into()]).unwrap();
        ix.set_folds("other.md", &["h2".into()]).unwrap();
        ix.remove_file("sub/a.md").unwrap();
        assert!(ix.folds("sub/a.md").unwrap().is_empty());
        ix.remove_file("sub").unwrap();
        assert!(ix.folds("sub/b.md").unwrap().is_empty());
        assert_eq!(ix.folds("other.md").unwrap(), vec!["h2".to_string()]);
    }
}
