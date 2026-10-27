//! Shared application state and the original process-wide mutation/update gates.

use crate::models::{DatabaseGame, Settings};
use crate::processes::RunningDummy;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, RwLock};

// Services acquire this before library read/modify/write or launch/setup operations.
// IPC adapters must not also acquire it: doing so would recursively lock the same mutex.
pub(crate) static DATA_OPERATIONS: Mutex<()> = Mutex::new(());
pub(crate) static UPDATING: AtomicBool = AtomicBool::new(false);

pub(crate) struct AppPaths {
    pub(crate) games_dir: PathBuf,
    pub(crate) settings_path: PathBuf,
    pub(crate) my_games_path: PathBuf,
    pub(crate) gamelist_path: PathBuf,
}

pub(crate) struct AppState {
    pub(crate) paths: AppPaths,
    pub(crate) settings: Mutex<Settings>,
    pub(crate) database: RwLock<Vec<DatabaseGame>>,
    pub(crate) running: Arc<Mutex<HashMap<String, RunningDummy>>>,
}
