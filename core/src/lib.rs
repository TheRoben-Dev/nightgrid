//! NIGHTGRID core: UI-agnostic logic for the homelab command center.
//!
//! Everything that touches SSH, Docker, metrics, storage or secrets lives here
//! so it can be unit-tested without a webview. The Tauri crate (`app/`) is a
//! thin layer of commands and events over this crate.

pub mod access;
pub mod alerts;
pub mod automation;
pub mod backup;
pub mod backup_jobs;
pub mod backup_restore;
pub mod blueprint;
pub mod bookmarks;
pub mod cmdwidget;
pub mod confdiff;
pub mod dashboard;
pub mod depgraph;
pub mod device_labels;
pub mod diagnostics;
pub mod diskforecast;
pub mod docker;
pub mod edit_hook;
pub mod envvault;
pub mod error;
pub mod expiry;
pub mod files;
pub mod helper;
pub mod hosts;
pub mod http;
pub mod integrations;
pub mod journal;
pub mod logsearch;
pub mod logwatch;
pub mod metrics;
pub mod netident;
pub mod netinv;
pub mod netlayout;
pub mod netmap;
pub mod netperf;
pub mod netsec;
pub mod nettools;
pub mod notes;
pub mod power;
pub mod power_schedule;
pub mod powercost;
pub mod proxmox;
pub mod quest;
pub mod redact;
pub mod secrets;
pub mod services;
pub mod settings;
pub mod sla;
pub mod ssh;
pub mod stack_dirs;
pub mod status_page;
pub mod store;
pub mod system;
pub mod tailnet;
pub mod term;
pub mod timeline;
pub mod tunnels;
pub mod ups;
pub mod uptime;
pub mod vnc;
pub mod webui;

pub use error::{Error, Result};
