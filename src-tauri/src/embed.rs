//! One background thread that embeds passages, lowest priority, paused while
//! the user types.

use crate::state::AppState;
use engram_core::config::EmbedConfig;
use engram_core::embed::{Embedder, fastembed::FastEmbedder};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct EmbedStatus {
    pub model: Option<String>,
    /// `off`, `loading`, `ready` or `error`.
    pub state: &'static str,
    pub pending: usize,
    pub error: Option<String>,
}

impl Default for EmbedStatus {
    fn default() -> Self {
        EmbedStatus {
            model: None,
            state: "off",
            pending: 0,
            error: None,
        }
    }
}

#[derive(Default)]
pub struct Embed {
    pub status: Mutex<EmbedStatus>,
    pub embedder: Mutex<Option<Box<dyn Embedder>>>,
    pub typing: Mutex<Option<Instant>>,
    stop: AtomicBool,
}

impl Embed {
    /// The queue waits two seconds after the last keystroke.
    fn typing_now(&self) -> bool {
        self.typing
            .lock()
            .unwrap()
            .is_some_and(|t| t.elapsed() < Duration::from_secs(2))
    }

    pub fn stop(&self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

fn publish(app: &AppHandle, embed: &Embed, f: impl FnOnce(&mut EmbedStatus)) {
    let status = {
        let mut s = embed.status.lock().unwrap();
        f(&mut s);
        s.clone()
    };
    let _ = app.emit("embed-status", status);
}

/// Loads the model, then drains the queue for as long as the vault is open.
pub fn spawn(app: AppHandle, embed: Arc<Embed>, cfg: EmbedConfig, models_dir: std::path::PathBuf) {
    std::thread::spawn(move || {
        publish(&app, &embed, |s| {
            s.state = "loading";
            s.error = None;
        });
        let loaded = match cfg.model_dir.as_deref() {
            Some(dir) => {
                let dir = std::path::Path::new(dir);
                FastEmbedder::from_dir(dir, &engram_core::embed::fastembed::dir_id(dir))
            }
            None => FastEmbedder::from_dir(
                &models_dir.join("embedder"),
                engram_core::embed::fastembed::EMBEDDER_ID,
            ),
        };
        let model: Box<dyn Embedder> = match loaded {
            Ok(m) => Box::new(m),
            Err(e) => {
                publish(&app, &embed, |s| {
                    s.state = "error";
                    s.error = Some(e.to_string());
                });
                return;
            }
        };
        let id = model.id();
        {
            let state = app.state::<AppState>();
            let mut guard = state.open.lock().unwrap();
            let Some(open) = guard.as_mut() else { return };
            if let Err(e) = open.index.set_model_id(&id) {
                drop(guard);
                publish(&app, &embed, |s| {
                    s.state = "error";
                    s.error = Some(e.to_string());
                });
                return;
            }
        }
        // One owner from here on: the queue and the query path both go through
        // this mutex.
        *embed.embedder.lock().unwrap() = Some(model);
        publish(&app, &embed, |s| {
            s.state = "ready";
            s.model = Some(id);
        });

        let batch = cfg.batch.max(1);
        while !embed.stop.load(Ordering::Relaxed) {
            if embed.typing_now() {
                std::thread::sleep(Duration::from_millis(500));
                continue;
            }
            // The index lock is held to take work and to write it back, never
            // across the embedding itself.
            let pending = {
                let state = app.state::<AppState>();
                let mut guard = state.open.lock().unwrap();
                let Some(open) = guard.as_mut() else { break };
                open.index.pending_vectors(batch).unwrap_or_default()
            };
            if pending.is_empty() {
                publish(&app, &embed, |s| s.pending = 0);
                std::thread::sleep(Duration::from_millis(750));
                continue;
            }
            let texts: Vec<String> = pending.iter().map(|p| p.text.clone()).collect();
            let vectors = {
                let mut guard = embed.embedder.lock().unwrap();
                let Some(m) = guard.as_mut() else { break };
                m.embed_documents(&texts)
            };
            match vectors {
                Ok(vectors) => {
                    let left = {
                        let state = app.state::<AppState>();
                        let mut guard = state.open.lock().unwrap();
                        let Some(open) = guard.as_mut() else { break };
                        let rows: Vec<(String, Vec<f32>)> =
                            pending.into_iter().map(|p| p.hash).zip(vectors).collect();
                        let _ = open.index.put_vectors(&rows);
                        open.index.pending_count().unwrap_or(0)
                    };
                    publish(&app, &embed, |s| s.pending = left);
                }
                Err(e) => {
                    // A failing embedding never blocks editing.
                    publish(&app, &embed, |s| {
                        s.state = "error";
                        s.error = Some(e.to_string());
                    });
                    std::thread::sleep(Duration::from_secs(5));
                }
            }
        }
    });
}
