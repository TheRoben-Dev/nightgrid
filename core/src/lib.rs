//! NIGHTGRID core: UI-agnostic logic for the homelab command center.
//!
//! Everything that touches SSH, Docker, metrics, storage or secrets lives here
//! so it can be unit-tested without a webview. The Tauri crate (`app/`) is a
//! thin layer of commands and events over this crate.

pub mod error;
pub mod secrets;
pub mod settings;
pub mod store;

pub use error::{Error, Result};
