//! Secrets & rotation (1.1). Filled in with the Security view.

use tauri::AppHandle;

/// Records that a secret Nightgrid keeps was set or replaced just now.
pub fn stamp(_app: &AppHandle, _key: &str) {}
