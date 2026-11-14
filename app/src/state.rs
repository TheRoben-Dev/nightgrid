use std::path::{Path, PathBuf};
use std::sync::Mutex;

use nightgrid_core::store::Store;

pub struct AppState {
    pub store: Mutex<Store>,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
}

impl AppState {
    pub fn open(data_dir: &Path, log_dir: PathBuf) -> nightgrid_core::Result<Self> {
        let store = Store::open(&data_dir.join("nightgrid.db"))?;
        Ok(Self { store: Mutex::new(store), data_dir: data_dir.to_owned(), log_dir })
    }
}
