//! File changes from outside the app, debounced, as vault-relative paths.

use crate::vault::Vault;
use crate::{Error, Result};
use notify::RecursiveMode;
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChangeKind {
    Changed,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Change {
    pub path: String,
    pub kind: ChangeKind,
}

pub struct Watcher {
    _debouncer: Debouncer<notify::RecommendedWatcher, RecommendedCache>,
}

fn rel(root: &Path, p: &Path) -> Option<String> {
    let r = p.strip_prefix(root).ok()?;
    if r.components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
    {
        return None;
    }
    Some(r.to_string_lossy().replace('\\', "/"))
}

fn io_err(root: &Path, e: notify::Error) -> Error {
    Error::io(root, std::io::Error::other(e))
}

pub fn watch(
    vault: &Vault,
    on_event: impl Fn(std::result::Result<Vec<Change>, String>) + Send + 'static,
) -> Result<Watcher> {
    let root: PathBuf = vault.root().to_path_buf();
    let handler_root = root.clone();
    let mut debouncer = new_debouncer(
        Duration::from_millis(300),
        None,
        move |res: DebounceEventResult| {
            let events = match res {
                Ok(events) => events,
                Err(errors) => {
                    let msg: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
                    on_event(Err(msg.join("; ")));
                    return;
                }
            };
            let mut out: Vec<Change> = Vec::new();
            for ev in events {
                let kind = if ev.kind.is_remove() {
                    ChangeKind::Removed
                } else {
                    ChangeKind::Changed
                };
                for p in &ev.paths {
                    let Some(path) = rel(&handler_root, p) else {
                        continue;
                    };
                    // A rename arrives as one event with two paths: old, then new.
                    let k = if ev.kind.is_modify() && ev.paths.len() == 2 && p == &ev.paths[0] {
                        ChangeKind::Removed
                    } else {
                        kind.clone()
                    };
                    if !out.iter().any(|c| c.path == path && c.kind == k) {
                        out.push(Change { path, kind: k });
                    }
                }
            }
            if !out.is_empty() {
                on_event(Ok(out));
            }
        },
    )
    .map_err(|e| io_err(&root, e))?;
    debouncer
        .watch(&root, RecursiveMode::Recursive)
        .map_err(|e| io_err(&root, e))?;
    Ok(Watcher {
        _debouncer: debouncer,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    #[test]
    fn reports_markdown_changes_and_ignores_config_dir() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        let seen: Arc<Mutex<Vec<Change>>> = Arc::default();
        let sink = seen.clone();
        let _w = watch(&v, move |c| {
            sink.lock().unwrap().extend(c.unwrap_or_default())
        })
        .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        std::fs::write(d.path().join("n.md"), "x").unwrap();
        std::fs::write(d.path().join(".engram-notes/app.json"), "{}").unwrap();
        let start = Instant::now();
        loop {
            if seen.lock().unwrap().iter().any(|c| c.path == "n.md")
                || start.elapsed() > Duration::from_secs(5)
            {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let got = seen.lock().unwrap();
        assert!(
            got.iter()
                .any(|c| c.path == "n.md" && c.kind == ChangeKind::Changed),
            "{got:?}"
        );
        assert!(!got.iter().any(|c| c.path.starts_with(".engram-notes")));
    }
}
