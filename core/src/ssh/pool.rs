//! One shared, multiplexed SSH connection per host. Terminal channels (and
//! later SFTP, metrics and Docker forwarding) all ride the same connection.

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use super::client::{ConnectError, Connection};
use crate::hosts::HostId;

type Slot = Arc<tokio::sync::Mutex<Option<Arc<Connection>>>>;

#[derive(Default)]
pub struct Pool {
    slots: Mutex<HashMap<HostId, Slot>>,
}

impl Pool {
    pub fn new() -> Self {
        Self::default()
    }

    fn slot(&self, id: HostId) -> Slot {
        self.slots.lock().expect("pool poisoned").entry(id).or_default().clone()
    }

    /// Returns the live connection for `id`, connecting with `connect` if there
    /// is none (or it died). Concurrent callers for the same host share one
    /// attempt (so after a drop, the first tab to retry reconnects for all);
    /// different hosts connect in parallel.
    pub async fn get<F, Fut>(&self, id: HostId, connect: F) -> Result<Arc<Connection>, ConnectError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Connection, ConnectError>>,
    {
        let slot = self.slot(id);
        let mut guard = slot.lock().await;
        if let Some(c) = guard.as_ref().filter(|c| !c.is_closed()) {
            return Ok(c.clone());
        }
        let c = Arc::new(connect().await?);
        *guard = Some(c.clone());
        Ok(c)
    }

    pub fn is_connected(&self, id: HostId) -> bool {
        let slot = self.slots.lock().expect("pool poisoned").get(&id).cloned();
        slot.and_then(|s| s.try_lock().ok().and_then(|g| g.as_ref().map(|c| !c.is_closed()))).unwrap_or(false)
    }

    /// Hosts with a live connection.
    pub fn connected(&self) -> Vec<HostId> {
        let ids: Vec<HostId> = self.slots.lock().expect("pool poisoned").keys().copied().collect();
        ids.into_iter().filter(|id| self.is_connected(*id)).collect()
    }

    /// Drops and disconnects the host's connection (all its channels close).
    pub async fn disconnect(&self, id: HostId) {
        let slot = self.slots.lock().expect("pool poisoned").remove(&id);
        if let Some(slot) = slot {
            if let Some(c) = slot.lock().await.take() {
                c.disconnect().await;
            }
        }
    }
}
