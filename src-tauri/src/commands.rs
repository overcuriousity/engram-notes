use crate::embed::EmbedStatus;
use crate::error::{CmdResult, CommandError};
use crate::state::{AppState, Open};
use engram_core::bases::{SortKey, Table};
use engram_core::config::{self, AppConfig, Snippet};
use engram_core::graph::{Graph, SemanticEdge};
use engram_core::import::logseq::{self as logseq_import, Summary as ImportSummary};
use engram_core::index::query::{LinkRow, PropertyCount, TagCount, Unresolved};
use engram_core::index::{Index, RebuildStats};
use engram_core::memory::{EventKind, related::Related};
use engram_core::rename::RenamePlan;
use engram_core::search::SearchResults;
use engram_core::templates;
use engram_core::vault::{FileEntry, Vault};
use engram_core::watch::{Change, ChangeKind};
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;

#[derive(serde::Serialize)]
pub struct VaultInfo {
    pub root: String,
    pub config: AppConfig,
    pub stats: RebuildStats,
    pub index_recreated: bool,
    /// Why the watcher could not start; `None` while it runs.
    pub watch_error: Option<String>,
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

/// A folder given on the command line, as in `engram-notes ~/notes`.
#[tauri::command]
pub fn startup_vault() -> Option<String> {
    std::env::args()
        .skip(1)
        .find(|a| !a.starts_with('-') && std::path::Path::new(a).is_dir())
}

#[tauri::command]
pub fn open_vault(app: AppHandle, state: State<AppState>, path: String) -> CmdResult<VaultInfo> {
    let vault = Vault::open(&path)?;
    app.asset_protocol_scope()
        .allow_directory(vault.root(), true)
        .map_err(|e| CommandError {
            code: "io",
            message: e.to_string(),
            report: None,
        })?;
    let (mut index, index_recreated) = Index::open_or_recreate(&config::index_path(&vault)?)?;
    let stats = index.rebuild(&vault)?;
    let cfg = config::load_config(&vault)?;
    let root = vault.root().to_string_lossy().into_owned();
    remember(&root);

    let handle = app.clone();
    let (watcher, watch_error) =
        match engram_core::watch::watch(&vault, move |ev| on_watch(&handle, ev)) {
            Ok(w) => (Some(w), None),
            Err(e) => (None, Some(e.to_string())),
        };

    *state.open.lock().unwrap() = Some(Open {
        vault,
        index,
        config: cfg.clone(),
        _watcher: watcher,
    });
    // A previous vault's thread stops; a new one loads the model and drains.
    let embed = {
        let mut slot = state.embed.lock().unwrap();
        slot.stop();
        *slot = std::sync::Arc::new(crate::embed::Embed::default());
        slot.clone()
    };
    crate::embed::spawn(
        app.clone(),
        embed,
        cfg.embed.clone(),
        config::data_dir()?.join("models"),
    );

    Ok(VaultInfo {
        root,
        config: cfg,
        stats,
        index_recreated,
        watch_error,
    })
}

// Runs on the watcher thread. After a failure the UI rescans on focus.
fn on_watch(app: &AppHandle, event: Result<Vec<Change>, String>) {
    match event {
        Ok(changes) => apply_changes(app, changes),
        Err(message) => {
            let _ = app.emit("watch-failed", message);
        }
    }
}

/// A vault file's absolute path, for the webview's asset protocol.
#[tauri::command]
pub fn attachment_path(state: State<AppState>, path: String) -> CmdResult<String> {
    with_open(&state, |o| {
        Ok(o.vault.abs(&path).to_string_lossy().into_owned())
    })
}

#[tauri::command]
pub fn open_external(app: AppHandle, state: State<AppState>, path: String) -> CmdResult<()> {
    let abs = with_open(&state, |o| Ok(o.vault.abs(&path)))?;
    app.opener()
        .open_path(abs.to_string_lossy().into_owned(), None::<&str>)
        .map_err(|e| CommandError {
            code: "io",
            message: e.to_string(),
            report: None,
        })
}

/// Catches up with edits a failed watcher missed.
#[tauri::command]
pub fn rescan(state: State<AppState>) -> CmdResult<RebuildStats> {
    with_open(&state, |o| Ok(o.index.rebuild(&o.vault)?))
}

#[tauri::command]
pub fn anchor_line(
    state: State<AppState>,
    path: String,
    fragment: String,
) -> CmdResult<Option<u32>> {
    with_open(&state, |o| Ok(o.index.anchor_line(&path, &fragment)?))
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
pub fn list_folders(state: State<AppState>) -> CmdResult<Vec<String>> {
    with_open(&state, |o| Ok(o.vault.folders()?))
}

#[tauri::command]
pub fn graph(state: State<AppState>) -> CmdResult<Graph> {
    with_open(&state, |o| {
        Ok(engram_core::graph::build(&o.index, &o.vault.walk()?)?)
    })
}

#[tauri::command]
pub fn get_graph_config(state: State<AppState>) -> CmdResult<serde_json::Value> {
    with_open(&state, |o| Ok(config::load_graph(&o.vault)?))
}

#[tauri::command]
pub fn set_graph_config(state: State<AppState>, config: serde_json::Value) -> CmdResult<()> {
    with_open(&state, |o| Ok(config::save_graph(&o.vault, &config)?))
}

#[tauri::command]
pub fn run_base(state: State<AppState>, path: String, view: usize) -> CmdResult<Table> {
    with_open(&state, |o| {
        let text = o.vault.read(&path)?;
        let notes = engram_core::bases::notes(&o.index)?;
        let now = chrono::Local::now().naive_local();
        Ok(engram_core::bases::run(&text, &notes, view, now)?)
    })
}

#[tauri::command]
pub fn set_base_sort(
    state: State<AppState>,
    path: String,
    view: usize,
    sort: Vec<SortKey>,
) -> CmdResult<()> {
    with_open(&state, |o| {
        let text = o.vault.read(&path)?;
        o.vault
            .write(&path, &engram_core::bases::set_sort(&text, view, &sort)?)?;
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
        Ok(engram_core::rename::plan_rename(
            &o.vault, &o.index, &from, &to,
        )?)
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
) -> CmdResult<SearchResults> {
    // The query vector is taken before the index lock, and only if a model is up.
    let vector = {
        let embed = state.embed.lock().unwrap().clone();
        let mut guard = embed.embedder.lock().unwrap();
        guard.as_mut().and_then(|m| m.embed_query(&query).ok())
    };
    with_open(&state, |o| {
        Ok(engram_core::search::hybrid(
            &o.index,
            &query,
            vector.as_deref(),
            &o.config.search,
            &o.config.memory,
            now(),
            limit.unwrap_or(50),
        )?)
    })
}

fn now() -> i64 {
    chrono::Local::now().timestamp()
}

#[tauri::command]
pub fn record_event(
    state: State<AppState>,
    kind: EventKind,
    path: Option<String>,
    query: Option<String>,
) -> CmdResult<()> {
    with_open(&state, |o| {
        if !o.config.memory.enabled {
            return Ok(());
        }
        o.index.record_event(
            kind,
            path.as_deref(),
            query.as_deref(),
            &o.config.memory,
            now(),
        )?;
        Ok(())
    })
}

#[tauri::command]
pub fn related(state: State<AppState>, path: String) -> CmdResult<Related> {
    with_open(&state, |o| {
        let text = o.vault.read(&path).unwrap_or_default();
        Ok(engram_core::memory::related::related(
            &o.index,
            &path,
            &text,
            &o.config.memory,
            &o.config.search,
            now(),
            10,
        )?)
    })
}

#[tauri::command]
pub fn all_properties(state: State<AppState>) -> CmdResult<Vec<PropertyCount>> {
    with_open(&state, |o| Ok(o.index.property_counts()?))
}

#[tauri::command]
pub fn forget_memory(state: State<AppState>) -> CmdResult<()> {
    with_open(&state, |o| Ok(o.index.forget_memory()?))
}

#[tauri::command]
pub fn semantic_edges(
    state: State<AppState>,
    paths: Option<Vec<String>>,
    top_k: Option<usize>,
) -> CmdResult<Vec<SemanticEdge>> {
    with_open(&state, |o| {
        Ok(engram_core::graph::semantic_edges(
            &o.index,
            paths.as_deref(),
            top_k.unwrap_or(3),
            &o.config.memory,
            &o.config.search,
            now(),
        )?)
    })
}

#[tauri::command]
pub fn embed_status(state: State<AppState>) -> EmbedStatus {
    let embed = state.embed.lock().unwrap().clone();
    let status = embed.status.lock().unwrap();
    status.clone()
}

/// The editor pings while the user types; the queue waits.
#[tauri::command]
pub fn typing(state: State<AppState>) {
    let embed = state.embed.lock().unwrap().clone();
    *embed.typing.lock().unwrap() = Some(std::time::Instant::now());
}

/// A folder with the ONNX file and tokenizer, or `None` to download again.
#[tauri::command]
pub fn set_model_dir(app: AppHandle, state: State<AppState>, dir: Option<String>) -> CmdResult<()> {
    let cfg = with_open(&state, |o| {
        o.config.embed.model_dir = dir;
        config::save_config(&o.vault, &o.config)?;
        Ok(o.config.embed.clone())
    })?;
    let embed = {
        let mut slot = state.embed.lock().unwrap();
        slot.stop();
        *slot = std::sync::Arc::new(crate::embed::Embed::default());
        slot.clone()
    };
    crate::embed::spawn(app, embed, cfg, config::data_dir()?.join("models"));
    Ok(())
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
pub fn get_folds(state: State<AppState>, path: String) -> CmdResult<Vec<String>> {
    with_open(&state, |o| Ok(o.index.folds(&path)?))
}

#[tauri::command]
pub fn set_folds(state: State<AppState>, path: String, keys: Vec<String>) -> CmdResult<()> {
    with_open(&state, |o| Ok(o.index.set_folds(&path, &keys)?))
}

#[tauri::command]
pub fn daily_note(state: State<AppState>) -> CmdResult<String> {
    with_open(&state, |o| {
        let now = chrono::Local::now().naive_local();
        let path = config::daily_note_path(&o.config, now.date());
        if o.vault.stat(&path)?.is_none() {
            let text = match &o.config.daily_notes.template {
                Some(t) => templates::render(&o.vault, &o.config, t, now, title_of(&path))
                    .unwrap_or_default(),
                None => String::new(),
            };
            o.vault.create(&path, &text)?;
            o.index.update_file(&o.vault, &path)?;
        }
        Ok(path)
    })
}

fn title_of(path: &str) -> &str {
    let name = path.rsplit('/').next().unwrap_or(path);
    name.strip_suffix(".md").unwrap_or(name)
}

#[tauri::command]
pub fn templates(state: State<AppState>) -> CmdResult<Vec<String>> {
    with_open(&state, |o| Ok(templates::list(&o.vault, &o.config)?))
}

/// The template at `path` filled in for the note at `into`, as of now.
#[tauri::command]
pub fn render_template(state: State<AppState>, path: String, into: String) -> CmdResult<String> {
    with_open(&state, |o| {
        let now = chrono::Local::now().naive_local();
        Ok(templates::render(
            &o.vault,
            &o.config,
            &path,
            now,
            title_of(&into),
        )?)
    })
}

#[tauri::command]
pub fn snippets(state: State<AppState>) -> CmdResult<Vec<Snippet>> {
    with_open(&state, |o| Ok(config::snippets(&o.vault)?))
}

/// `dir` is an absolute folder from the dialog; it has to be inside the vault.
fn inside_vault(vault: &Vault, dir: &str) -> CmdResult<String> {
    let abs = std::path::Path::new(dir)
        .canonicalize()
        .map_err(|_| CommandError {
            code: "not_found",
            message: format!("{dir}: no such folder"),
            report: None,
        })?;
    let rel = abs.strip_prefix(vault.root()).map_err(|_| CommandError {
        code: "config",
        message: "the destination must be a folder inside the vault".into(),
        report: None,
    })?;
    Ok(rel.to_string_lossy().replace('\\', "/"))
}

/// `dir` is the graph from the dialog; the importer reads it as it is on
/// disk, so it can be neither the vault nor a part of it nor its parent.
fn outside_vault(vault: &Vault, dir: &str) -> CmdResult<std::path::PathBuf> {
    let abs = std::path::Path::new(dir)
        .canonicalize()
        .map_err(|_| CommandError {
            code: "not_found",
            message: format!("{dir}: no such folder"),
            report: None,
        })?;
    if abs.starts_with(vault.root()) || vault.root().starts_with(&abs) {
        return Err(CommandError {
            code: "config",
            message: "the graph must be a folder outside the vault".into(),
            report: None,
        });
    }
    Ok(abs)
}

#[tauri::command]
pub fn import_logseq(
    state: State<AppState>,
    source: String,
    dest: String,
) -> CmdResult<ImportSummary> {
    with_open(&state, |o| {
        let rel = inside_vault(&o.vault, &dest)?;
        let graph = outside_vault(&o.vault, &source)?;
        // An import that failed still wrote pages, and they are only findable
        // once the index has seen them.
        let out = logseq_import::run(&o.vault, &o.config, &graph, &rel);
        let rebuilt = o.index.rebuild(&o.vault);
        let summary = out?;
        rebuilt?;
        Ok(summary)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Each shape is what `ui/src/lib/api.ts` declares.
    #[test]
    fn vault_info() {
        let info = VaultInfo {
            root: "/v".into(),
            config: AppConfig::default(),
            stats: RebuildStats {
                added: 1,
                ..Default::default()
            },
            index_recreated: true,
            watch_error: None,
        };
        let v = serde_json::to_value(&info).unwrap();
        assert_eq!(v["root"], "/v");
        assert_eq!(v["config"]["editor"]["default_mode"], "live");
        assert_eq!(v["config"]["daily_notes"]["template"], json!(null));
        assert_eq!(v["config"]["templates"]["folder"], "Templates");
        assert_eq!(v["config"]["css_snippets"], json!([]));
        assert_eq!(
            v["stats"],
            json!({"added": 1, "updated": 0, "removed": 0, "unchanged": 0})
        );
        assert_eq!(v["index_recreated"], true);
        assert_eq!(v["watch_error"], json!(null));
    }

    #[test]
    fn snippets_and_the_title_a_template_sees() {
        let s = Snippet {
            name: "wide".into(),
            css: "b {}".into(),
            error: None,
        };
        assert_eq!(
            serde_json::to_value(&s).unwrap(),
            json!({"name": "wide", "css": "b {}", "error": null})
        );
        assert_eq!(title_of("Daily/2026-09-14.md"), "2026-09-14");
        assert_eq!(title_of("Note.md"), "Note");
        assert_eq!(title_of(""), "");
    }

    #[test]
    fn errors_changes_notes_and_links() {
        let e = CommandError::from(engram_core::Error::NotFound("x.md".into()));
        assert_eq!(
            serde_json::to_value(&e).unwrap(),
            json!({"code": "not_found", "message": "not found: x.md"})
        );
        let c = Change {
            path: "a.md".into(),
            kind: ChangeKind::Removed,
        };
        assert_eq!(
            serde_json::to_value(&c).unwrap(),
            json!({"path": "a.md", "kind": "removed"})
        );
        let n = NoteText {
            path: "a.md".into(),
            text: "t".into(),
            mtime_ms: 5,
        };
        assert_eq!(
            serde_json::to_value(&n).unwrap(),
            json!({"path": "a.md", "text": "t", "mtime_ms": 5})
        );
        let row = LinkRow {
            src_path: "a.md".into(),
            target_raw: "B".into(),
            target_path: None,
            kind: "wiki".into(),
            heading: None,
            block: Some("x".into()),
            alias: None,
            line: 3,
            context: "c".into(),
        };
        assert_eq!(
            serde_json::to_value(&row).unwrap(),
            json!({"src_path": "a.md", "target_raw": "B", "target_path": null, "kind": "wiki",
                   "heading": null, "block": "x", "alias": null, "line": 3, "context": "c"})
        );
    }

    #[test]
    fn a_graph_inside_the_vault_is_refused() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("In/pages")).unwrap();
        let v = Vault::open(d.path()).unwrap();
        let inside = d.path().join("In").to_string_lossy().into_owned();
        let err = outside_vault(&v, &inside).unwrap_err();
        assert_eq!(err.code, "config");
        let root = d.path().to_string_lossy().into_owned();
        assert_eq!(outside_vault(&v, &root).unwrap_err().code, "config");
        let outer = tempfile::tempdir().unwrap();
        assert!(outside_vault(&v, &outer.path().to_string_lossy()).is_ok());
        assert_eq!(
            outside_vault(&v, "/no/such/folder").unwrap_err().code,
            "not_found"
        );
    }

    #[test]
    fn graph_and_base_table() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("A.md"), "---\ns: 1\n---\n[[Ghost]]").unwrap();
        let v = Vault::open(d.path()).unwrap();
        let mut ix = Index::open_in_memory().unwrap();
        ix.rebuild(&v).unwrap();
        let g = engram_core::graph::build(&ix, &v.walk().unwrap()).unwrap();
        assert_eq!(
            serde_json::to_value(&g).unwrap(),
            json!({
                "nodes": [
                    {"id": "A.md", "title": "A", "kind": "note", "tags": []},
                    {"id": "Ghost", "title": "Ghost", "kind": "unresolved", "tags": []}
                ],
                "edges": [{"source": "A.md", "target": "Ghost"}]
            })
        );
        let notes = engram_core::bases::notes(&ix).unwrap();
        let now = chrono::Local::now().naive_local();
        let t = engram_core::bases::run(
            "views:\n  - type: table\n    order: [file.basename, s]\n",
            &notes,
            0,
            now,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(&t).unwrap(),
            json!({
                "views": ["View 1"], "view": 0,
                "columns": [
                    {"id": "file.basename", "label": "file basename", "editable": false},
                    {"id": "note.s", "label": "s", "editable": true}
                ],
                "rows": [{"path": "A.md", "cells": ["A", 1]}],
                "sort": [], "errors": []
            })
        );
        let k: engram_core::bases::SortKey =
            serde_json::from_value(json!({"property": "s", "direction": "DESC"})).unwrap();
        assert_eq!(k.direction, "DESC");
    }

    #[test]
    fn rename_plan_from_the_ui() {
        let p: RenamePlan =
            serde_json::from_value(json!({"from": "a.md", "to": "b.md", "affected": ["c.md"]}))
                .unwrap();
        assert_eq!(p.affected, vec!["c.md"]);
    }

    #[test]
    fn search_results_and_status_serialise_for_the_frontend() {
        let hit = engram_core::search::Hit {
            path: "A.md".into(),
            title: "A".into(),
            snippet: "<mark>a</mark>".into(),
            heading: Some("H".into()),
            line: 3,
            similarity: Some(0.8),
            score: 0.5,
            past_divider: true,
            primed: false,
        };
        let json = serde_json::to_value(engram_core::search::SearchResults {
            hits: vec![hit],
            associated: vec![],
        })
        .unwrap();
        assert_eq!(json["hits"][0]["past_divider"], true);
        // An f32 widens on the way out; the frontend reads a number, not a literal.
        let similarity = json["hits"][0]["similarity"].as_f64().unwrap();
        assert!((similarity - 0.8).abs() < 1e-6, "{similarity}");
        let status = serde_json::to_value(crate::embed::EmbedStatus::default()).unwrap();
        assert_eq!(status["state"], "off");
        assert_eq!(status["pending"], 0);
    }

    #[test]
    fn an_event_kind_arrives_as_snake_case() {
        let kind: engram_core::memory::EventKind =
            serde_json::from_str("\"open_from_search\"").unwrap();
        assert_eq!(kind, engram_core::memory::EventKind::OpenFromSearch);
    }

    #[test]
    fn import_summary() {
        let s = ImportSummary {
            pages: 2,
            journals: 1,
            assets: 0,
            skipped: 0,
            unmapped: 3,
            report: "import-report.md".into(),
        };
        let v = serde_json::to_value(&s).unwrap();
        assert_eq!(v["report"], "import-report.md");
        assert_eq!(v["unmapped"], 3);
    }

    #[test]
    fn destination_outside_the_vault_is_refused() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("v");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        let v = Vault::open(&root).unwrap();
        assert!(inside_vault(&v, d.path().to_str().unwrap()).is_err());
        assert_eq!(inside_vault(&v, root.to_str().unwrap()).unwrap(), "");
        assert_eq!(
            inside_vault(&v, root.join("sub").to_str().unwrap()).unwrap(),
            "sub"
        );
    }
}
