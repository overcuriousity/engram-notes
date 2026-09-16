//! Vault-level settings under `.engram-notes/`, and where derived state goes.

use crate::vault::Vault;
use crate::{Error, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct EditorConfig {
    pub default_mode: String,
    /// Spaces per outline level. Two is CommonMark-correct under `- `.
    pub indent: usize,
}

impl Default for EditorConfig {
    fn default() -> Self {
        EditorConfig {
            default_mode: "live".into(),
            indent: 2,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct DailyNotes {
    pub folder: String,
    pub template: Option<String>,
    pub format: String,
}

impl Default for DailyNotes {
    fn default() -> Self {
        DailyNotes {
            folder: "Daily".into(),
            template: None,
            format: "%Y-%m-%d".into(),
        }
    }
}

/// Obsidian's `templates.json`, with its date and time formats in its tokens.
#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct TemplatesConfig {
    pub folder: String,
    pub date_format: String,
    pub time_format: String,
}

impl Default for TemplatesConfig {
    fn default() -> Self {
        TemplatesConfig {
            folder: "Templates".into(),
            date_format: "YYYY-MM-DD".into(),
            time_format: "HH:mm".into(),
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct SearchConfig {
    /// Each branch fetches `limit * multiplier` candidates before fusing.
    pub candidate_multiplier: usize,
    pub rrf_k: f64,
    pub cliff_factor: f32,
    pub cliff_min_share: f32,
    /// A cosine below this says nothing, so the hit is a stranger. e5's band is
    /// narrow — two notes with nothing in common still score about 0.76 — and
    /// this is what tells a neighbour from one. Measured for
    /// `multilingual-e5-small`; a different model needs a different number.
    pub similarity_floor: f32,
    /// How many fused hits the cross-encoder rescores. Its cost is the
    /// dominant one in a search, so this is what the budget trades against.
    pub rerank_n: usize,
    /// Below this a rerank score is a stranger. A MiniLM cross-encoder's
    /// sigmoid sits near 0 or 1, so this only has to sort the two bands.
    pub rerank_floor: f32,
    /// A rerank run longer than this switches reranking off for the session.
    pub rerank_budget_ms: u64,
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            candidate_multiplier: 3,
            rrf_k: crate::search::fuse::RRF_K,
            cliff_factor: crate::search::fuse::CLIFF_FACTOR,
            cliff_min_share: crate::search::fuse::CLIFF_MIN_SHARE,
            similarity_floor: 0.83,
            rerank_n: 20,
            rerank_floor: 0.1,
            rerank_budget_ms: 500,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct MemoryConfig {
    pub enabled: bool,
    pub activation_half_life_days: f64,
    pub assoc_half_life_days: f64,
    /// Silence longer than this starts a new sitting.
    pub sitting_gap_secs: i64,
    /// Two notes reached this far apart in one sitting are associated.
    pub assoc_window_secs: i64,
    /// A link shows once its decayed strength reaches this.
    pub assoc_show: f64,
    pub prime_margin: f64,
    pub prime_lift: usize,
    pub spread_max: usize,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        MemoryConfig {
            enabled: true,
            activation_half_life_days: 30.0,
            assoc_half_life_days: 90.0,
            sitting_gap_secs: 1800,
            assoc_window_secs: 600,
            assoc_show: 2.0,
            prime_margin: 0.5,
            prime_lift: 2,
            spread_max: 3,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct EmbedConfig {
    /// A folder with the ONNX file and tokenizer, replacing the bundled embedder.
    pub model_dir: Option<String>,
    /// The same for the reranker.
    pub reranker_dir: Option<String>,
    pub batch: usize,
}

impl Default for EmbedConfig {
    fn default() -> Self {
        EmbedConfig {
            model_dir: None,
            reranker_dir: None,
            batch: 32,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    pub editor: EditorConfig,
    pub daily_notes: DailyNotes,
    pub templates: TemplatesConfig,
    pub hotkeys: BTreeMap<String, String>,
    pub theme: String,
    /// Names of the snippets in `.engram-notes/snippets/` that are switched on.
    pub css_snippets: Vec<String>,
    pub search: SearchConfig,
    pub memory: MemoryConfig,
    pub embed: EmbedConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            editor: EditorConfig::default(),
            daily_notes: DailyNotes::default(),
            templates: TemplatesConfig::default(),
            hotkeys: BTreeMap::new(),
            theme: "system".into(),
            css_snippets: Vec::new(),
            search: SearchConfig::default(),
            memory: MemoryConfig::default(),
            embed: EmbedConfig::default(),
        }
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    match std::fs::read_to_string(path) {
        Ok(s) => serde_json::from_str(&s)
            .map(Some)
            .map_err(|e| Error::Config(format!("{}: {e}", path.display()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(Error::io(path, e)),
    }
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<()> {
    let text = serde_json::to_string_pretty(value).map_err(|e| Error::Config(e.to_string()))?;
    std::fs::write(path, text).map_err(|e| Error::io(path, e))
}

pub fn load_config(vault: &Vault) -> Result<AppConfig> {
    let path = vault.config_dir().join("app.json");
    match read_json::<AppConfig>(&path)? {
        Some(mut cfg) => {
            // An indent of zero is one no editor can build a unit from.
            cfg.editor.indent = cfg.editor.indent.clamp(1, 8);
            Ok(cfg)
        }
        None => {
            let cfg = AppConfig::default();
            write_json(&path, &cfg)?;
            Ok(cfg)
        }
    }
}

pub fn save_config(vault: &Vault, cfg: &AppConfig) -> Result<()> {
    write_json(&vault.config_dir().join("app.json"), cfg)
}

pub fn load_workspace(vault: &Vault) -> Result<serde_json::Value> {
    Ok(read_json(&vault.config_dir().join("workspace.json"))?
        .unwrap_or_else(|| serde_json::json!({})))
}

pub fn save_workspace(vault: &Vault, ws: &serde_json::Value) -> Result<()> {
    write_json(&vault.config_dir().join("workspace.json"), ws)
}

pub fn load_graph(vault: &Vault) -> Result<serde_json::Value> {
    Ok(read_json(&vault.config_dir().join("graph.json"))?.unwrap_or_else(|| serde_json::json!({})))
}

pub fn save_graph(vault: &Vault, graph: &serde_json::Value) -> Result<()> {
    write_json(&vault.config_dir().join("graph.json"), graph)
}

pub fn daily_note_path(cfg: &AppConfig, today: chrono::NaiveDate) -> String {
    let name = crate::templates::format_date(&cfg.daily_notes.format, today.into());
    let folder = cfg.daily_notes.folder.trim_matches('/');
    if folder.is_empty() {
        format!("{name}.md")
    } else {
        format!("{folder}/{name}.md")
    }
}

/// One user stylesheet, as Obsidian keeps them under `.obsidian/snippets/`.
#[derive(serde::Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Snippet {
    pub name: String,
    pub css: String,
    /// Why the file could not be read, if it could not; `css` is then empty.
    pub error: Option<String>,
}

pub fn snippets_dir(vault: &Vault) -> PathBuf {
    vault.config_dir().join("snippets")
}

/// Every `.css` file in the snippets folder, by name. The folder is made on
/// the first look so there is somewhere to drop a file. A file that will not
/// read is listed with its error, so one bad file does not take the rest of
/// the user's styling down with it.
pub fn snippets(vault: &Vault) -> Result<Vec<Snippet>> {
    let dir = snippets_dir(vault);
    std::fs::create_dir_all(&dir).map_err(|e| Error::io(&dir, e))?;
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|e| Error::io(&dir, e))? {
        let entry = entry.map_err(|e| Error::io(&dir, e))?;
        let path = entry.path();
        let Some(name) = path.file_stem().map(|s| s.to_string_lossy().into_owned()) else {
            continue;
        };
        let is_css = path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("css"));
        if !is_css || name.starts_with('.') || !path.is_file() {
            continue;
        }
        let (css, error) = match std::fs::read_to_string(&path) {
            Ok(css) => (css, None),
            Err(e) => (String::new(), Some(e.to_string())),
        };
        out.push(Snippet { name, css, error });
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

pub fn data_dir() -> Result<PathBuf> {
    dirs::data_dir()
        .map(|d| d.join("engram-notes"))
        .ok_or_else(|| Error::Config("no data directory".into()))
}

pub fn index_path(vault: &Vault) -> Result<PathBuf> {
    use sha2::Digest;
    let key = hex::encode(sha2::Sha256::digest(
        vault.root().to_string_lossy().as_bytes(),
    ));
    Ok(data_dir()?.join("vaults").join(&key[..16]).join("index.db"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_written_on_first_load_and_round_trip() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        let cfg = load_config(&v).unwrap();
        assert_eq!(cfg, AppConfig::default());
        assert!(d.path().join(".engram-notes/app.json").is_file());
        let mut changed = cfg.clone();
        changed.theme = "dark".into();
        changed.daily_notes.folder = "Journal".into();
        save_config(&v, &changed).unwrap();
        assert_eq!(load_config(&v).unwrap(), changed);
    }

    #[test]
    fn partial_file_fills_defaults() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        std::fs::write(
            d.path().join(".engram-notes/app.json"),
            r#"{"theme":"light"}"#,
        )
        .unwrap();
        let cfg = load_config(&v).unwrap();
        assert_eq!(cfg.theme, "light");
        assert_eq!(cfg.daily_notes.folder, "Daily");
    }

    #[test]
    fn workspace_is_opaque_json() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        assert_eq!(load_workspace(&v).unwrap(), serde_json::json!({}));
        save_workspace(&v, &serde_json::json!({"tabs": ["a.md"]})).unwrap();
        assert_eq!(load_workspace(&v).unwrap()["tabs"][0], "a.md");
    }

    #[test]
    fn graph_settings_are_opaque_json() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        assert_eq!(load_graph(&v).unwrap(), serde_json::json!({}));
        save_graph(&v, &serde_json::json!({"showOrphans": false})).unwrap();
        assert_eq!(load_graph(&v).unwrap()["showOrphans"], false);
        assert!(d.path().join(".engram-notes/graph.json").is_file());
    }

    #[test]
    fn daily_path_and_index_path() {
        let cfg = AppConfig::default();
        assert_eq!(
            daily_note_path(&cfg, chrono::NaiveDate::from_ymd_opt(2026, 9, 12).unwrap()),
            "Daily/2026-09-12.md"
        );
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        let p = index_path(&v).unwrap();
        assert!(p.ends_with("index.db"));
        assert!(p.to_string_lossy().contains("engram-notes"));
    }

    #[test]
    fn search_and_memory_defaults_are_the_shipped_numbers() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.search.candidate_multiplier, 3);
        assert_eq!(cfg.search.rrf_k, 60.0);
        assert_eq!(cfg.search.cliff_factor, 3.0);
        assert_eq!(cfg.search.similarity_floor, 0.83);
        assert_eq!(cfg.search.rerank_n, 20);
        assert_eq!(cfg.search.rerank_floor, 0.1);
        assert_eq!(cfg.search.rerank_budget_ms, 500);
        assert!(cfg.memory.enabled);
        assert_eq!(cfg.memory.activation_half_life_days, 30.0);
        assert_eq!(cfg.memory.assoc_half_life_days, 90.0);
        assert_eq!(cfg.memory.sitting_gap_secs, 1800);
        assert_eq!(cfg.memory.prime_lift, 2);
        assert_eq!(cfg.memory.spread_max, 3);
        assert_eq!(cfg.embed.batch, 32);
        assert_eq!(cfg.embed.model_dir, None);
        assert_eq!(cfg.embed.reranker_dir, None);
    }

    #[test]
    fn an_old_app_json_gains_the_new_sections() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        std::fs::write(
            d.path().join(".engram-notes/app.json"),
            r#"{"theme":"dark","memory":{"enabled":false}}"#,
        )
        .unwrap();
        let cfg = load_config(&v).unwrap();
        assert!(!cfg.memory.enabled);
        assert_eq!(cfg.memory.spread_max, 3);
        assert_eq!(cfg.search.rrf_k, 60.0);
    }

    #[test]
    fn templates_and_snippets_default_to_obsidians_shape() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.templates.folder, "Templates");
        assert_eq!(cfg.templates.date_format, "YYYY-MM-DD");
        assert_eq!(cfg.templates.time_format, "HH:mm");
        assert!(cfg.css_snippets.is_empty());
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        std::fs::write(
            d.path().join(".engram-notes/app.json"),
            r#"{"theme":"dark","css_snippets":["wide"]}"#,
        )
        .unwrap();
        let cfg = load_config(&v).unwrap();
        assert_eq!(cfg.css_snippets, vec!["wide"]);
        assert_eq!(cfg.templates.folder, "Templates");
    }

    #[test]
    fn daily_path_takes_obsidians_tokens_too() {
        let mut cfg = AppConfig::default();
        cfg.daily_notes.format = "YYYY/MM/YYYY-MM-DD".into();
        assert_eq!(
            daily_note_path(&cfg, chrono::NaiveDate::from_ymd_opt(2026, 9, 12).unwrap()),
            "Daily/2026/09/2026-09-12.md"
        );
    }

    #[test]
    fn snippets_are_css_files_by_name_and_the_folder_is_made() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        assert_eq!(snippets(&v).unwrap(), Vec::<Snippet>::new());
        let dir = d.path().join(".engram-notes/snippets");
        assert!(dir.is_dir());
        std::fs::write(dir.join("wide.css"), ":root { --file-line-width: 900px; }").unwrap();
        std::fs::write(dir.join("Aa.CSS"), "b {}").unwrap();
        std::fs::write(dir.join("notes.txt"), "x").unwrap();
        std::fs::write(dir.join(".draft.css"), "x").unwrap();
        std::fs::create_dir(dir.join("folder.css")).unwrap();
        let got = snippets(&v).unwrap();
        assert_eq!(
            got.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["Aa", "wide"]
        );
        assert_eq!(got[1].css, ":root { --file-line-width: 900px; }");
        assert!(got.iter().all(|s| s.error.is_none()));
    }

    #[test]
    fn a_snippet_that_will_not_read_is_listed_with_its_error() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        let dir = d.path().join(".engram-notes/snippets");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("good.css"), "b {}").unwrap();
        std::fs::write(dir.join("bad.css"), [0xff, 0xfe, 0x00]).unwrap();
        let got = snippets(&v).unwrap();
        assert_eq!(
            got.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            vec!["bad", "good"]
        );
        assert!(got[0].error.is_some());
        assert_eq!(got[0].css, "");
        assert_eq!(got[1].css, "b {}");
    }

    #[test]
    fn editor_indent_defaults_to_two_spaces() {
        assert_eq!(AppConfig::default().editor.indent, 2);
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        std::fs::write(
            d.path().join(".engram-notes/app.json"),
            r#"{"editor":{"default_mode":"source"}}"#,
        )
        .unwrap();
        let cfg = load_config(&v).unwrap();
        assert_eq!(cfg.editor.default_mode, "source");
        assert_eq!(cfg.editor.indent, 2);
    }

    #[test]
    fn editor_indent_is_clamped_to_a_width_an_editor_can_build() {
        let d = tempfile::tempdir().unwrap();
        let v = Vault::open(d.path()).unwrap();
        let file = d.path().join(".engram-notes/app.json");
        std::fs::write(&file, r#"{"editor":{"indent":0}}"#).unwrap();
        assert_eq!(load_config(&v).unwrap().editor.indent, 1);
        std::fs::write(&file, r#"{"editor":{"indent":99}}"#).unwrap();
        assert_eq!(load_config(&v).unwrap().editor.indent, 8);
    }
}
