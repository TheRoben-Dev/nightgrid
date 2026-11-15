//! NIGHTGRID core: UI-agnostic logic for the homelab command center.
//!
//! Everything that touches SSH, Docker, metrics, storage or secrets lives here
//! so it can be unit-tested without a webview. The Tauri crate (`app/`) is a
//! thin layer of commands and events over this crate.

pub mod alerts;
pub mod automation;
pub mod backup;
pub mod backup_jobs;
pub mod bookmarks;
pub mod confdiff;
pub mod device_labels;
pub mod docker;
pub mod edit_hook;
pub mod error;
pub mod expiry;
pub mod files;
pub mod helper;
pub mod hosts;
pub mod http;
pub mod integrations;
pub mod logwatch;
pub mod metrics;
pub mod netmap;
pub mod notes;
pub mod power;
pub mod power_schedule;
pub mod redact;
pub mod secrets;
pub mod services;
pub mod settings;
pub mod ssh;
pub mod stack_dirs;
pub mod status_page;
pub mod store;
pub mod system;
pub mod tailnet;
pub mod term;
pub mod timeline;
pub mod tunnels;

pub use error::{Error, Result};
