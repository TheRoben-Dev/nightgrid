//! Portainer: deep links only, plus a reachability/version check
//! (`GET /api/system/status`, which needs no token).

use serde::{Deserialize, Serialize};

use super::{http_error, Integration};
use crate::http::{send, Request};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PortainerStatus {
    pub version: Option<String>,
}

/// Container page in Portainer's UI.
pub fn container_url(i: &Integration, container_id: &str) -> String {
    format!("{}/#!/{}/docker/containers/{}", i.base_url, i.endpoint_id, container_id)
}

/// Stacks page in Portainer's UI.
pub fn stacks_url(i: &Integration) -> String {
    format!("{}/#!/{}/docker/stacks", i.base_url, i.endpoint_id)
}

pub async fn status(i: &Integration) -> Result<PortainerStatus, String> {
    let r = send(
        Request::get(format!("{}/api/system/status", i.base_url))
            .header("Accept", "application/json")
            .allow_untrusted(i.allow_untrusted),
    )
    .await?;
    if !r.ok() {
        return Err(http_error("Portainer", r.status));
    }
    let v: serde_json::Value = r.json()?;
    Ok(PortainerStatus { version: v["Version"].as_str().map(str::to_owned) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links() {
        let i = Integration {
            id: 1,
            kind: super::super::IntegrationKind::Portainer,
            name: "Portainer".into(),
            base_url: "https://portainer.lan:9443".into(),
            username: String::new(),
            slug: String::new(),
            endpoint_id: 2,
            host_id: Some(1),
            allow_untrusted: true,
        };
        assert_eq!(container_url(&i, "abc123"), "https://portainer.lan:9443/#!/2/docker/containers/abc123");
        assert_eq!(stacks_url(&i), "https://portainer.lan:9443/#!/2/docker/stacks");
    }
}
