//! One background thread that embeds passages, lowest priority, paused while
//! the user types.

use crate::state::AppState;
use engram_core::config::{EmbedConfig, SearchConfig};
use engram_core::embed::fastembed::{EMBEDDER_ID, FastEmbedder, FastReranker, RERANKER_ID, dir_id};
use engram_core::embed::{Embedder, Reranker, TimedReranker};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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
    /// `off`, `loading`, `ready`, `slow` or `error`.
    pub rerank: &'static str,
    /// The last measured run, and the one that switched it off when `slow`.
    pub rerank_ms: Option<u64>,
    /// How many hits a search rescores now: the setting, halved until it fits.
    pub rerank_n: Option<usize>,
}

impl Default for EmbedStatus {
    fn default() -> Self {
        EmbedStatus {
            model: None,
            state: "off",
            pending: 0,
            error: None,
            rerank: "off",
            rerank_ms: None,
            rerank_n: None,
        }
    }
}

#[derive(Default)]
pub struct Embed {
    pub status: Mutex<EmbedStatus>,
    pub embedder: Mutex<Option<Box<dyn Embedder>>>,
    pub reranker: Mutex<Option<TimedReranker>>,
    /// The batch the reranker is held to; the search command halves it when a
    /// run goes over budget.
    pub rerank_n: AtomicUsize,
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

pub(crate) fn publish(app: &AppHandle, embed: &Embed, f: impl FnOnce(&mut EmbedStatus)) {
    let status = {
        let mut s = embed.status.lock().unwrap();
        f(&mut s);
        s.clone()
    };
    let _ = app.emit("embed-status", status);
}

/// Publishes only while `embed` is still the live one. A search holds the
/// `Embed` it started with, and picking a new model folder meanwhile swaps in
/// another whose thread publishes `loading`: the finishing search would put the
/// old model back as `ready` for the length of the load.
pub(crate) fn publish_live(app: &AppHandle, embed: &Arc<Embed>, f: impl FnOnce(&mut EmbedStatus)) {
    let live = {
        let state = app.state::<AppState>();
        let slot = state.embed.lock().unwrap();
        Arc::ptr_eq(&slot, embed)
    };
    if live {
        publish(app, embed, f);
    }
}

/// The folder a model loads from and the id it gets: an override folder is
/// named after itself, the bundled one after the model.
fn model_folder(
    override_dir: Option<&str>,
    resources: &Path,
    bundled: &str,
    id: &str,
) -> (PathBuf, String) {
    match override_dir {
        Some(d) => {
            let d = PathBuf::from(d);
            let id = dir_id(&d);
            (d, id)
        }
        None => (resources.join("models").join(bundled), id.to_owned()),
    }
}

/// Below this many pairs the cross-encoder has too little to say, so a
/// machine that cannot fit them in the budget searches by fusion order.
pub const RERANK_MIN: usize = 5;

/// What the reranker reads of one hit, `n` times: what one search costs.
fn warm_up_batch(n: usize) -> Vec<String> {
    let text = "the vault keeps a folder of markdown notes and an index that can be rebuilt "
        .repeat(16)
        .chars()
        .take(engram_core::search::RERANK_CHARS)
        .collect::<String>();
    vec![text; n.max(1)]
}

/// Loads both models, then drains the queue for as long as the vault is open.
pub fn spawn(app: AppHandle, embed: Arc<Embed>, cfg: EmbedConfig, search: SearchConfig) {
    std::thread::spawn(move || {
        publish(&app, &embed, |s| {
            s.state = "loading";
            s.error = None;
            s.rerank = "loading";
            s.rerank_ms = None;
        });
        let resources = match app.path().resource_dir() {
            Ok(r) => r,
            Err(e) => {
                publish(&app, &embed, |s| {
                    s.state = "error";
                    s.error = Some(format!("no resource folder: {e}"));
                    s.rerank = "error";
                });
                return;
            }
        };
        let (dir, id) = model_folder(
            cfg.model_dir.as_deref(),
            &resources,
            "embedder",
            EMBEDDER_ID,
        );
        let model: Box<dyn Embedder> = match FastEmbedder::from_dir(&dir, &id) {
            Ok(m) => Box::new(m),
            Err(e) => {
                publish(&app, &embed, |s| {
                    s.state = "error";
                    s.error = Some(format!("{e} (looked in {})", dir.display()));
                    s.rerank = "off";
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

        // On a thread of its own: loading the cross-encoder and measuring it
        // against the budget is seconds of work on a slow machine, and the
        // queue is at its longest the moment a vault opens.
        std::thread::spawn({
            let (app, embed) = (app.clone(), Arc::clone(&embed));
            let (cfg, search, resources) = (cfg.clone(), search.clone(), resources.clone());
            move || load_reranker(&app, &embed, &cfg, &search, &resources)
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

/// Loads the reranker and scores one search's worth of pairs, halving the
/// batch until a run fits the budget. A run cannot be interrupted, so the
/// budget is enforced by measurement; under `RERANK_MIN` pairs the reranker
/// is not installed and the machine searches by fusion order.
fn load_reranker(
    app: &AppHandle,
    embed: &Embed,
    cfg: &EmbedConfig,
    search: &SearchConfig,
    resources: &Path,
) {
    let (dir, id) = model_folder(
        cfg.reranker_dir.as_deref(),
        resources,
        "reranker",
        RERANKER_ID,
    );
    let loaded = match FastReranker::from_dir(&dir, &id) {
        Ok(r) => r,
        Err(e) => {
            publish(app, embed, |s| {
                s.rerank = "error";
                s.error = Some(format!("{e} (looked in {})", dir.display()));
            });
            return;
        }
    };
    let mut timed = TimedReranker::new(Box::new(loaded));
    let mut n = search.rerank_n.max(RERANK_MIN);
    // The first run also warms ONNX Runtime, so it is not the one measured.
    if let Err(e) = timed.score("warm up", &warm_up_batch(RERANK_MIN)) {
        publish(app, embed, |s| {
            s.rerank = "error";
            s.error = Some(e.to_string());
        });
        return;
    }
    let ms = loop {
        if timed
            .score("a query about the notes", &warm_up_batch(n))
            .is_err()
        {
            break None;
        }
        // Taken, so the calibration's reading is not left behind for the
        // first search to mistake for its own.
        let ms = timed.take_last().unwrap_or_default().as_millis() as u64;
        if ms <= search.rerank_budget_ms {
            break Some(ms);
        }
        if n / 2 < RERANK_MIN {
            publish(app, embed, |s| {
                s.rerank = "slow";
                s.rerank_ms = Some(ms);
                s.rerank_n = None;
            });
            return;
        }
        n /= 2;
    };
    let Some(ms) = ms else {
        publish(app, embed, |s| {
            s.rerank = "error";
            s.error = timed.take_last_error();
        });
        return;
    };
    // The vault can close while the measuring runs; nothing is installed into
    // an `Embed` whose thread has been told to stop.
    if embed.stop.load(Ordering::Relaxed) {
        return;
    }
    embed.rerank_n.store(n, Ordering::Relaxed);
    *embed.reranker.lock().unwrap() = Some(timed);
    publish(app, embed, |s| {
        s.rerank = "ready";
        s.rerank_ms = Some(ms);
        s.rerank_n = Some(n);
    });
}
