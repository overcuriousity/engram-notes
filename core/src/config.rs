//! Vault-level settings under `.engram-notes/`, and where derived state goes.

use crate::vault::Vault;
use crate::{Error, Result};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct EditorConfig {
    pub default_mode: String,
}

impl Default for EditorConfig {
    fn default() -> Self {
        EditorConfig {
            default_mode: "live".into(),
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
pub struct AppConfig {
    pub editor: EditorConfig,
    pub daily_notes: DailyNotes,
    pub hotkeys: BTreeMap<String, String>,
    pub theme: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        AppConfig {
            editor: EditorConfig::default(),
            daily_notes: DailyNotes::default(),
            hotkeys: BTreeMap::new(),
            theme: "system".into(),
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
        Some(cfg) => Ok(cfg),
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
}
