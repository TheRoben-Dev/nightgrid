//! NIGHTGRID core: UI-agnostic logic for the homelab command center.
//!
//! Everything that touches SSH, Docker, metrics, storage or secrets lives here
//! so it can be unit-tested without a webview. The Tauri crate (`app/`) is a
//! thin layer of commands and events over this crate.

pub mod alerts;
pub mod bookmarks;
pub mod docker;
pub mod edit_hook;
pub mod error;
pub mod files;
pub mod hosts;
pub mod http;
pub mod integrations;
pub mod metrics;
pub mod secrets;
pub mod services;
pub mod settings;
pub mod ssh;
pub mod stack_dirs;
pub mod store;
pub mod system;
pub mod term;
pub mod tunnels;

pub use error::{Error, Result};
