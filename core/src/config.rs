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
}

impl Default for SearchConfig {
    fn default() -> Self {
        SearchConfig {
            candidate_multiplier: 3,
            rrf_k: crate::search::fuse::RRF_K,
            cliff_factor: crate::search::fuse::CLIFF_FACTOR,
            cliff_min_share: crate::search::fuse::CLIFF_MIN_SHARE,
            similarity_floor: 0.83,
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
    /// A folder with the ONNX file and tokenizer, for machines with no network.
    pub model_dir: Option<String>,
    pub batch: usize,
}

impl Default for EmbedConfig {
    fn default() -> Self {
        EmbedConfig {
            model_dir: None,
            batch: 32,
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct AppConfig {
    pub editor: EditorConfig,
    pub daily_notes: DailyNotes,
    pub hotkeys: BTreeMap<String, String>,
    pub theme: String,
    pub search: SearchConfig,
    pub memory: MemoryConfig,
    pub embed: EmbedConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            editor: EditorConfig::default(),
            daily_notes: DailyNotes::default(),
            hotkeys: BTreeMap::new(),
            theme: "system".into(),
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
    let name = today.format(&cfg.daily_notes.format).to_string();
    let folder = cfg.daily_notes.folder.trim_matches('/');
    if folder.is_empty() {
        format!("{name}.md")
    } else {
        format!("{folder}/{name}.md")
    }
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
        assert!(cfg.memory.enabled);
        assert_eq!(cfg.memory.activation_half_life_days, 30.0);
        assert_eq!(cfg.memory.assoc_half_life_days, 90.0);
        assert_eq!(cfg.memory.sitting_gap_secs, 1800);
        assert_eq!(cfg.memory.prime_lift, 2);
        assert_eq!(cfg.memory.spread_max, 3);
        assert_eq!(cfg.embed.batch, 32);
        assert_eq!(cfg.embed.model_dir, None);
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
