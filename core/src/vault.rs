//! The folder of files. Paths in and out are vault-relative with '/'.

use crate::{Error, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const CONFIG_DIR: &str = ".engram-notes";

#[derive(Debug, Clone)]
pub struct Vault {
    root: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct FileEntry {
    pub path: String,
    pub mtime_ms: i64,
    pub size: u64,
    pub is_markdown: bool,
}

pub fn normalize_rel(path: &str) -> String {
    let p = path.replace('\\', "/");
    p.trim_start_matches("./")
        .trim_start_matches('/')
        .to_owned()
}

fn is_hidden(name: &std::ffi::OsStr) -> bool {
    name.to_string_lossy().starts_with('.')
}

impl Vault {
    pub fn open(root: impl AsRef<Path>) -> Result<Vault> {
        let root = root.as_ref();
        if !root.is_dir() {
            return Err(Error::NotFound(root.display().to_string()));
        }
        let root = root.canonicalize().map_err(|e| Error::io(root, e))?;
        let cfg = root.join(CONFIG_DIR);
        fs::create_dir_all(&cfg).map_err(|e| Error::io(&cfg, e))?;
        Ok(Vault { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_dir(&self) -> PathBuf {
        self.root.join(CONFIG_DIR)
    }

    pub fn abs(&self, rel: &str) -> PathBuf {
        let mut p = self.root.clone();
        for part in normalize_rel(rel)
            .split('/')
            .filter(|s| !s.is_empty() && *s != "..")
        {
            p.push(part);
        }
        p
    }

    pub fn walk(&self) -> Result<Vec<FileEntry>> {
        let mut out = Vec::new();
        let walker = walkdir::WalkDir::new(&self.root)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !is_hidden(e.file_name()));
        for entry in walker {
            let entry = entry.map_err(|e| Error::io(&self.root, e.into()))?;
            if !entry.file_type().is_file() {
                continue;
            }
            let rel = entry.path().strip_prefix(&self.root).unwrap();
            let path = rel.to_string_lossy().replace('\\', "/");
            let md = entry
                .metadata()
                .map_err(|e| Error::io(entry.path(), e.into()))?;
            out.push(FileEntry {
                is_markdown: path.to_lowercase().ends_with(".md"),
                path,
                mtime_ms: mtime_ms(&md),
                size: md.len(),
            });
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    /// Folders below the root, hidden ones skipped, so empty folders can be shown.
    pub fn folders(&self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        let walker = walkdir::WalkDir::new(&self.root)
            .min_depth(1)
            .into_iter()
            .filter_entry(|e| !is_hidden(e.file_name()));
        for entry in walker {
            let entry = entry.map_err(|e| Error::io(&self.root, e.into()))?;
            if entry.file_type().is_dir() {
                let rel = entry.path().strip_prefix(&self.root).unwrap();
                out.push(rel.to_string_lossy().replace('\\', "/"));
            }
        }
        out.sort();
        Ok(out)
    }

    pub fn stat(&self, rel: &str) -> Result<Option<FileEntry>> {
        let abs = self.abs(rel);
        match fs::metadata(&abs) {
            Ok(md) if md.is_file() => Ok(Some(FileEntry {
                path: normalize_rel(rel),
                mtime_ms: mtime_ms(&md),
                size: md.len(),
                is_markdown: rel.to_lowercase().ends_with(".md"),
            })),
            Ok(_) => Ok(None),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::io(abs, e)),
        }
    }

    pub fn read(&self, rel: &str) -> Result<String> {
        let abs = self.abs(rel);
        fs::read_to_string(&abs).map_err(|e| Error::io(abs, e))
    }

    pub fn write(&self, rel: &str, text: &str) -> Result<()> {
        let abs = self.abs(rel);
        let dir = abs.parent().ok_or_else(|| Error::NotFound(rel.into()))?;
        fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        let tmp = dir.join(format!(
            ".{}.tmp",
            abs.file_name().unwrap().to_string_lossy()
        ));
        fs::write(&tmp, text).map_err(|e| Error::io(&tmp, e))?;
        fs::rename(&tmp, &abs).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            Error::io(&abs, e)
        })
    }

    pub fn create(&self, rel: &str, text: &str) -> Result<()> {
        if self.abs(rel).exists() {
            return Err(Error::Exists(normalize_rel(rel)));
        }
        self.write(rel, text)
    }

    pub fn delete(&self, rel: &str) -> Result<()> {
        let abs = self.abs(rel);
        trash::delete(&abs).map_err(|e| Error::io(&abs, std::io::Error::other(e)))
    }

    pub fn rename(&self, from: &str, to: &str) -> Result<()> {
        let src = self.abs(from);
        let dst = self.abs(to);
        if dst.exists() {
            return Err(Error::Exists(normalize_rel(to)));
        }
        if let Some(dir) = dst.parent() {
            fs::create_dir_all(dir).map_err(|e| Error::io(dir, e))?;
        }
        fs::rename(&src, &dst).map_err(|e| Error::io(src, e))
    }
}

fn mtime_ms(md: &fs::Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> (tempfile::TempDir, Vault) {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        (d, v)
    }

    #[test]
    fn open_creates_config_dir_and_rejects_missing() {
        let (d, v) = tmp();
        assert!(d.path().join(".engram-notes").is_dir());
        assert_eq!(v.root(), d.path().canonicalize().unwrap());
        assert!(Vault::open(d.path().join("nope")).is_err());
    }

    #[test]
    fn walk_skips_hidden_and_sorts() {
        let (d, v) = tmp();
        fs::create_dir_all(d.path().join("b/.git")).unwrap();
        fs::write(d.path().join("b/.git/x.md"), "").unwrap();
        fs::write(d.path().join("b/two.md"), "2").unwrap();
        fs::write(d.path().join("a.md"), "1").unwrap();
        fs::write(d.path().join("pic.png"), "p").unwrap();
        fs::write(d.path().join(".hidden.md"), "").unwrap();
        let e: Vec<_> = v
            .walk()
            .unwrap()
            .into_iter()
            .map(|e| (e.path, e.is_markdown))
            .collect();
        assert_eq!(
            e,
            vec![
                ("a.md".into(), true),
                ("b/two.md".into(), true),
                ("pic.png".into(), false)
            ]
        );
    }

    #[test]
    fn folders_include_empty_and_skip_hidden() {
        let (d, v) = tmp();
        fs::create_dir_all(d.path().join("a/b")).unwrap();
        fs::create_dir_all(d.path().join(".git/x")).unwrap();
        fs::create_dir_all(d.path().join("empty")).unwrap();
        assert_eq!(v.folders().unwrap(), vec!["a", "a/b", "empty"]);
    }

    #[test]
    fn write_is_atomic_and_creates_parents() {
        let (d, v) = tmp();
        v.write("deep/er/note.md", "hi").unwrap();
        assert_eq!(v.read("deep/er/note.md").unwrap(), "hi");
        let leftovers: Vec<_> = fs::read_dir(d.path().join("deep/er"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers.len(), 1);
    }

    #[test]
    fn create_refuses_existing() {
        let (_d, v) = tmp();
        v.create("n.md", "").unwrap();
        assert!(matches!(v.create("n.md", ""), Err(Error::Exists(_))));
    }

    #[test]
    fn rename_moves_and_refuses_overwrite() {
        let (_d, v) = tmp();
        v.create("a.md", "x").unwrap();
        v.create("c.md", "y").unwrap();
        v.rename("a.md", "sub/b.md").unwrap();
        assert_eq!(v.read("sub/b.md").unwrap(), "x");
        assert!(v.stat("a.md").unwrap().is_none());
        assert!(matches!(
            v.rename("sub/b.md", "c.md"),
            Err(Error::Exists(_))
        ));
    }

    #[test]
    fn normalize() {
        assert_eq!(normalize_rel("./a\\b/c.md"), "a/b/c.md");
        assert_eq!(normalize_rel("/x.md"), "x.md");
    }
}
