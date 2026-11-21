use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use nightgrid_core::docker::DockerCache;
use nightgrid_core::files::remote::SftpCache;
use nightgrid_core::files::transfer::Transfers;
use nightgrid_core::hosts::HostId;
use nightgrid_core::secrets::{KeyringStore, Secret, SecretKind, SecretRef, SecretStore};
use nightgrid_core::ssh::client::ConnectOptions;
use nightgrid_core::ssh::known_hosts::KnownHosts;
use nightgrid_core::ssh::pool::Pool;
use nightgrid_core::store::Store;
use nightgrid_core::term::TermHandle;

pub struct AppState {
    pub store: Mutex<Store>,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
    pub home: Option<PathBuf>,
    pub known_hosts: KnownHosts,
    pub pool: Pool,
    pub keychain: KeyringStore,
    /// Secrets entered this run (for reconnects without re-prompting). Memory
    /// only, zeroized on drop; keyed by `(kind, hop label)`.
    pub secret_cache: Mutex<HashMap<(SecretKind, String), Arc<Secret>>>,
    /// Hop label → stored host id, filled when a host is resolved, so a
    /// prompt answer can be saved to the right keychain entry.
    pub label_hosts: Mutex<HashMap<String, HostId>>,
    pub terms: Mutex<HashMap<u32, TermHandle>>,
    /// One SFTP session per host, on the pooled connection.
    pub sftp: SftpCache,
    /// Created on first use (needs the app handle for its resolver).
    pub transfers: OnceLock<Transfers>,
    /// One Docker engine client per host, on the pooled connection.
    pub docker: DockerCache,
    /// Running log streams, by id (aborted on stop).
    pub log_streams: Mutex<HashMap<u32, tauri::async_runtime::JoinHandle<()>>>,
    /// Host metric pollers and their UI channel.
    pub metrics: Arc<crate::metrics_cmd::MetricsHub>,
    /// Service health checker and its UI channel.
    pub services: Arc<crate::services_cmd::ServicesHub>,
    /// Running port forwards and their UI channel.
    pub tunnels: Arc<crate::tunnels_cmd::TunnelHub>,
    /// Alert tracker, its loop and UI channel.
    pub alerts: Arc<crate::alerts_cmd::AlertsHub>,
    /// UPS readings and the shutdown countdown (M10s).
    pub ups: Arc<crate::ups_cmd::UpsHub>,
    pub uptime: Arc<crate::uptime_cmd::UptimeHub>,
    pub logsearch: Arc<crate::logsearch_cmd::LogSearchHub>,
    pub netwatch: Arc<crate::netwatch_cmd::NetWatchHub>,
    pub firewall: Arc<crate::firewall_cmd::FirewallHub>,
    /// Journal follows and package runs: stop switch by id.
    pub system_runs: crate::system_cmd::SystemRuns,
    /// Running compose commands: cancel switch by run id.
    pub compose_runs: Mutex<HashMap<u32, tokio::sync::watch::Sender<bool>>>,
    /// Phone status page server.
    pub status_server: Arc<crate::status_cmd::StatusServer>,
    /// Built-in Tailscale client (M10j).
    /// What servers said while logging in (Tailscale SSH approval links), per host.
    pub auth_notices: Mutex<HashMap<HostId, (nightgrid_core::ssh::client::AuthNotice, std::time::Instant)>>,
    pub tailnet: Arc<crate::tailnet_cmd::TailnetHub>,
    /// Profile sync between devices (D-205).
    pub sync: Arc<crate::sync_cmd::SyncHub>,
    /// Backup file picked for import (opened and previewed, not yet imported).
    pub pending_backup: Mutex<Option<PathBuf>>,
    next_term: AtomicU32,
}

impl AppState {
    pub fn open(data_dir: &Path, log_dir: PathBuf) -> nightgrid_core::Result<Self> {
        let store = Store::open(&data_dir.join("nightgrid.db"))?;
        let home = std::env::home_dir();
        let system_kh = home.as_ref().map(|h| h.join(".ssh").join("known_hosts"));
        Ok(Self {
            store: Mutex::new(store),
            data_dir: data_dir.to_owned(),
            log_dir,
            known_hosts: KnownHosts::new(data_dir.join("known_hosts"), system_kh),
            home,
            pool: Pool::new(),
            keychain: KeyringStore::new(),
            secret_cache: Mutex::default(),
            label_hosts: Mutex::default(),
            terms: Mutex::default(),
            sftp: SftpCache::new(),
            transfers: OnceLock::new(),
            docker: DockerCache::new(),
            log_streams: Mutex::default(),
            compose_runs: Mutex::default(),
            pending_backup: Mutex::default(),
            system_runs: Default::default(),
            metrics: Arc::default(),
            services: Arc::default(),
            tunnels: Arc::default(),
            alerts: Arc::default(),
            ups: Arc::default(),
            uptime: Arc::default(),
            logsearch: Arc::default(),
            netwatch: Arc::default(),
            firewall: Arc::default(),
            status_server: Arc::default(),
            auth_notices: Mutex::default(),
            tailnet: Arc::default(),
            sync: Arc::default(),
            next_term: AtomicU32::new(1),
        })
    }

    pub fn store(&self) -> std::sync::MutexGuard<'_, Store> {
        self.store.lock().expect("store poisoned")
    }

    pub fn connect_options(&self) -> ConnectOptions {
        ConnectOptions::new(self.known_hosts.clone())
    }

    pub fn next_term_id(&self) -> u32 {
        self.next_term.fetch_add(1, Ordering::Relaxed)
    }

    /// Secret for a hop: this run's cache first, then the keychain for hosts
    /// that opted in to "remember".
    pub fn lookup_secret(
        &self,
        remembering: &HashSet<HostId>,
        host: Option<HostId>,
        kind: SecretKind,
        label: &str,
    ) -> Option<Arc<Secret>> {
        if let Some(id) = host {
            self.label_hosts.lock().expect("poisoned").insert(label.to_owned(), id);
        }
        if let Some(s) = self.secret_cache.lock().expect("poisoned").get(&(kind, label.to_owned())) {
            return Some(s.clone());
        }
        let id = host.filter(|id| remembering.contains(id))?;
        match self.keychain.get(&SecretRef::new(kind, id.to_string())) {
            Ok(Some(s)) => Some(Arc::new(s)),
            Ok(None) => None,
            Err(e) => {
                tracing::warn!("keychain read failed: {e}");
                None
            }
        }
    }

    /// Forgets cached and stored secrets for a host.
    pub fn forget_host_secrets(&self, id: HostId) {
        let labels: Vec<String> = self
            .label_hosts
            .lock()
            .expect("poisoned")
            .iter()
            .filter(|(_, h)| **h == id)
            .map(|(l, _)| l.clone())
            .collect();
        self.secret_cache.lock().expect("poisoned").retain(|(_, l), _| !labels.contains(l));
        for kind in [SecretKind::HostPassword, SecretKind::KeyPassphrase, SecretKind::SudoPassword] {
            if let Err(e) = self.keychain.delete(&SecretRef::new(kind, id.to_string())) {
                tracing::warn!("keychain delete failed: {e}");
            }
        }
    }
}
