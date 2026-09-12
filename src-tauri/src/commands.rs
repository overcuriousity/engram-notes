use crate::error::{CmdResult, CommandError};
use crate::state::{AppState, Open};
use engram_core::config::{self, AppConfig};
use engram_core::index::fts::FtsHit;
use engram_core::index::query::{LinkRow, TagCount, Unresolved};
use engram_core::index::{Index, RebuildStats};
use engram_core::rename::RenamePlan;
use engram_core::vault::{FileEntry, Vault};
use engram_core::watch::{Change, ChangeKind};
use tauri::{AppHandle, Emitter, Manager, State};

#[derive(serde::Serialize)]
pub struct VaultInfo {
    pub root: String,
    pub config: AppConfig,
    pub stats: RebuildStats,
}

#[derive(serde::Serialize)]
pub struct NoteText {
    pub path: String,
    pub text: String,
    pub mtime_ms: i64,
}

fn with_open<T>(
    state: &State<AppState>,
    f: impl FnOnce(&mut Open) -> CmdResult<T>,
) -> CmdResult<T> {
    let mut guard = state.open.lock().unwrap();
    let open = guard.as_mut().ok_or_else(CommandError::closed)?;
    f(open)
}

fn recent_file() -> Option<std::path::PathBuf> {
    config::data_dir().ok().map(|d| d.join("recent.json"))
}

fn remember(root: &str) {
    let Some(p) = recent_file() else { return };
    let mut list = read_recent();
    list.retain(|r| r != root);
    list.insert(0, root.to_owned());
    list.truncate(10);
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(&p, serde_json::to_string(&list).unwrap_or_default());
}

fn read_recent() -> Vec<String> {
    recent_file()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

#[tauri::command]
pub fn recent_vaults() -> Vec<String> {
    read_recent()
}

#[tauri::command]
pub fn open_vault(app: AppHandle, state: State<AppState>, path: String) -> CmdResult<VaultInfo> {
    let vault = Vault::open(&path)?;
    let mut index = Index::open(&config::index_path(&vault)?)?;
    let stats = index.rebuild(&vault)?;
    let cfg = config::load_config(&vault)?;
    let root = vault.root().to_string_lossy().into_owned();
    remember(&root);

    let handle = app.clone();
    let watcher =
        engram_core::watch::watch(&vault, move |changes| apply_changes(&handle, changes)).ok();

    *state.open.lock().unwrap() = Some(Open {
        vault,
        index,
        config: cfg.clone(),
        watcher,
    });
    Ok(VaultInfo {
        root,
        config: cfg,
        stats,
    })
}

// Runs on the watcher thread: re-index what changed, then tell the window.
fn apply_changes(app: &AppHandle, changes: Vec<Change>) {
    let state = app.state::<AppState>();
    let mut guard = state.open.lock().unwrap();
    let Some(open) = guard.as_mut() else { return };
    for c in &changes {
        let _ = match c.kind {
            ChangeKind::Removed => open.index.remove_file(&c.path),
            ChangeKind::Changed => open.index.update_file(&open.vault, &c.path).map(|_| ()),
        };
        let _ = app.emit("file-changed", c);
    }
    let _ = app.emit("index-changed", &changes);
}

fn mtime(vault: &Vault, path: &str) -> CmdResult<i64> {
    Ok(vault.stat(path)?.map(|e| e.mtime_ms).unwrap_or(0))
}

#[tauri::command]
pub fn list_files(state: State<AppState>) -> CmdResult<Vec<FileEntry>> {
    with_open(&state, |o| Ok(o.vault.walk()?))
}

#[tauri::command]
pub fn read_note(state: State<AppState>, path: String) -> CmdResult<NoteText> {
    with_open(&state, |o| {
        let text = o.vault.read(&path)?;
        let mtime_ms = mtime(&o.vault, &path)?;
        Ok(NoteText {
            path,
            text,
            mtime_ms,
        })
    })
}

#[tauri::command]
pub fn write_note(state: State<AppState>, path: String, text: String) -> CmdResult<i64> {
    with_open(&state, |o| {
        o.vault.write(&path, &text)?;
        o.index.update_file(&o.vault, &path)?;
        mtime(&o.vault, &path)
    })
}

#[tauri::command]
pub fn create_note(state: State<AppState>, path: String, text: String) -> CmdResult<()> {
    with_open(&state, |o| {
        o.vault.create(&path, &text)?;
        o.index.update_file(&o.vault, &path)?;
        Ok(())
    })
}

#[tauri::command]
pub fn create_folder(state: State<AppState>, path: String) -> CmdResult<()> {
    with_open(&state, |o| {
        let abs = o.vault.abs(&path);
        std::fs::create_dir_all(&abs).map_err(|e| engram_core::Error::io(abs, e))?;
        Ok(())
    })
}

#[tauri::command]
pub fn delete_file(state: State<AppState>, path: String) -> CmdResult<()> {
    with_open(&state, |o| {
        o.vault.delete(&path)?;
        o.index.remove_file(&path)?;
        Ok(())
    })
}

#[tauri::command]
pub fn plan_rename(state: State<AppState>, from: String, to: String) -> CmdResult<RenamePlan> {
    with_open(&state, |o| {
        Ok(engram_core::rename::plan_rename(&o.index, &from, &to)?)
    })
}

#[tauri::command]
pub fn apply_rename(state: State<AppState>, plan: RenamePlan) -> CmdResult<()> {
    with_open(&state, |o| {
        Ok(engram_core::rename::apply_rename(
            &o.vault,
            &mut o.index,
            &plan,
        )?)
    })
}

#[tauri::command]
pub fn backlinks(state: State<AppState>, path: String) -> CmdResult<Vec<LinkRow>> {
    with_open(&state, |o| Ok(o.index.backlinks(&path)?))
}

#[tauri::command]
pub fn outgoing(state: State<AppState>, path: String) -> CmdResult<Vec<LinkRow>> {
    with_open(&state, |o| Ok(o.index.outgoing(&path)?))
}

#[tauri::command]
pub fn unresolved(state: State<AppState>) -> CmdResult<Vec<Unresolved>> {
    with_open(&state, |o| Ok(o.index.unresolved()?))
}

#[tauri::command]
pub fn resolve_link(state: State<AppState>, target: String) -> CmdResult<Option<String>> {
    with_open(&state, |o| Ok(o.index.resolve_target(&target)?))
}

#[tauri::command]
pub fn titles(state: State<AppState>) -> CmdResult<Vec<(String, String)>> {
    with_open(&state, |o| Ok(o.index.titles()?))
}

#[tauri::command]
pub fn tags(state: State<AppState>) -> CmdResult<Vec<TagCount>> {
    with_open(&state, |o| Ok(o.index.tags()?))
}

#[tauri::command]
pub fn properties(
    state: State<AppState>,
    path: String,
) -> CmdResult<serde_json::Map<String, serde_json::Value>> {
    with_open(&state, |o| Ok(o.index.properties(&path)?))
}

#[tauri::command]
pub fn set_property(
    state: State<AppState>,
    path: String,
    key: String,
    value: serde_json::Value,
) -> CmdResult<()> {
    with_open(&state, |o| {
        let text = o.vault.read(&path)?;
        let out = engram_core::frontmatter::set_property(&text, &key, value);
        o.vault.write(&path, &out)?;
        o.index.update_file(&o.vault, &path)?;
        Ok(())
    })
}

#[tauri::command]
pub fn search(
    state: State<AppState>,
    query: String,
    limit: Option<usize>,
) -> CmdResult<Vec<FtsHit>> {
    with_open(&state, |o| {
        Ok(o.index.search_fts(&query, limit.unwrap_or(50))?)
    })
}

#[tauri::command]
pub fn get_config(state: State<AppState>) -> CmdResult<AppConfig> {
    with_open(&state, |o| Ok(o.config.clone()))
}

#[tauri::command]
pub fn set_config(state: State<AppState>, config: AppConfig) -> CmdResult<()> {
    with_open(&state, |o| {
        config::save_config(&o.vault, &config)?;
        o.config = config;
        Ok(())
    })
}

#[tauri::command]
pub fn get_workspace(state: State<AppState>) -> CmdResult<serde_json::Value> {
    with_open(&state, |o| Ok(config::load_workspace(&o.vault)?))
}

#[tauri::command]
pub fn set_workspace(state: State<AppState>, workspace: serde_json::Value) -> CmdResult<()> {
    with_open(&state, |o| {
        Ok(config::save_workspace(&o.vault, &workspace)?)
    })
}

#[tauri::command]
pub fn daily_note(state: State<AppState>) -> CmdResult<String> {
    with_open(&state, |o| {
        let path = config::daily_note_path(&o.config, chrono::Local::now().date_naive());
        if o.vault.stat(&path)?.is_none() {
            let template = match &o.config.daily_notes.template {
                Some(t) => o.vault.read(t).unwrap_or_default(),
                None => String::new(),
            };
            o.vault.create(&path, &template)?;
            o.index.update_file(&o.vault, &path)?;
        }
        Ok(path)
    })
}
