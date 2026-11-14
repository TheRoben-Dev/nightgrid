//! NIGHTGRID core: UI-agnostic logic for the homelab command center.
//!
//! Everything that touches SSH, Docker, metrics, storage or secrets lives here
//! so it can be unit-tested without a webview. The Tauri crate (`app/`) is a
//! thin layer of commands and events over this crate.

pub mod bookmarks;
pub mod docker;
pub mod edit_hook;
pub mod error;
pub mod files;
pub mod hosts;
pub mod secrets;
pub mod settings;
pub mod ssh;
pub mod stack_dirs;
pub mod store;
pub mod term;

pub use error::{Error, Result};
