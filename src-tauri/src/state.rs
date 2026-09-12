use engram_core::config::AppConfig;
use engram_core::index::Index;
use engram_core::vault::Vault;
use engram_core::watch::Watcher;
use std::sync::Mutex;

pub struct Open {
    pub vault: Vault,
    pub index: Index,
    pub config: AppConfig,
    // Held so the watch thread lives as long as the vault is open.
    pub _watcher: Option<Watcher>,
}

#[derive(Default)]
pub struct AppState {
    pub open: Mutex<Option<Open>>,
}
