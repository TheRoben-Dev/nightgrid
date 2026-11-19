//! Frigate NVR: cameras with their frame rates, detector speed, storage and
//! the latest detections. The internal port (5000) needs no login; the
//! authenticated port (8971) takes a Frigate login name and password
//! (`POST /api/login`, session cookie).

use serde_json::{json, Value};

use super::app::{bytes, cap_rows, cookie, n, plural, s, AppHealth, AppRow, AppRowState, AppStatus, Client};
use super::Integration;

async fn session<'a>(i: &'a Integration, password: &str) -> Result<Client<'a>, String> {
    let c = Client::new(i, "Frigate");
    if i.username.is_empty() || password.is_empty() {
        return Ok(c);
    }
    let r = c
        .send(crate::http::Method::Post, "/api/login", Some(&json!({"user": i.username, "password": password})))
        .await?;
    let r = c.check(r)?;
    let token = cookie(&r).ok_or("Frigate didn't start a session")?;
    Ok(c.header("Cookie", token))
}

pub fn build(stats: &Value, events: &Value) -> AppStatus {
    let mut rows: Vec<AppRow> = stats["cameras"]
        .as_object()
        .map(|m| {
            m.iter()
                .map(|(name, c)| {
                    let fps = n(c, "camera_fps").unwrap_or(0.0);
                    let det = n(c, "detection_fps").unwrap_or(0.0);
                    let skipped = n(c, "skipped_fps").unwrap_or(0.0);
                    let (meta, state) = if fps <= 0.0 {
                        ("no frames: camera offline?".to_owned(), AppRowState::Bad)
                    } else if skipped > 0.0 {
                        (format!("{fps:.1} fps · {det:.1} detect fps · {skipped:.1} skipped"), AppRowState::Warn)
                    } else {
                        (format!("{fps:.1} fps · {det:.1} detect fps"), AppRowState::Ok)
                    };
                    AppRow::new(name, name, meta, state)
                })
                .collect()
        })
        .unwrap_or_default();
    rows.sort_by(|a, b| a.title.cmp(&b.title));
    let offline = rows.iter().filter(|r| r.state == AppRowState::Bad).count() as u64;
    let mut st = AppStatus::new(format!(
        "{}{}",
        plural(rows.len() as u64, "camera"),
        if offline > 0 { format!(" · {offline} offline") } else { String::new() }
    ))
    .tile("Cameras", rows.len());
    let speeds: Vec<f64> = stats["detectors"]
        .as_object()
        .map(|m| m.values().filter_map(|d| n(d, "inference_speed")).collect())
        .unwrap_or_default();
    if let Some(ms) = speeds.iter().cloned().reduce(f64::max) {
        st = st.tile("Inference", format!("{ms:.1} ms"));
    }
    if let Some(store) = stats["service"]["storage"].as_object() {
        // Recordings live under /media/frigate/recordings by default.
        let rec = store.iter().find(|(k, _)| k.contains("recordings")).or_else(|| store.iter().next());
        if let Some((_, x)) = rec {
            let (total, used) = (n(x, "total").unwrap_or(0.0), n(x, "used").unwrap_or(0.0));
            if total > 0.0 {
                // Frigate reports MB.
                st = st.tile(
                    "Storage",
                    format!("{} of {}", bytes(used * 1024.0 * 1024.0), bytes(total * 1024.0 * 1024.0)),
                );
                if used / total > 0.9 {
                    st = st.warn("Recording storage is over 90 % full");
                }
            }
        }
    }
    let latest: Vec<String> = events
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .take(3)
        .map(|e| format!("{} on {}", s(e, "label"), s(e, "camera")))
        .collect();
    if !latest.is_empty() {
        st = st.tile("Latest", latest.join(", "));
    }
    if offline > 0 {
        st.health = AppHealth::Bad;
    } else if rows.iter().any(|r| r.state == AppRowState::Warn) {
        st.health = st.health.max(AppHealth::Warn);
    }
    st.version = Some(s(&stats["service"], "version")).filter(|v| !v.is_empty());
    st.more = cap_rows(&mut rows);
    st.rows_title = "Cameras".into();
    st.rows = rows;
    st
}

pub async fn status(i: &Integration, password: &str) -> Result<AppStatus, String> {
    let c = session(i, password).await?;
    let stats = c.get("/api/stats").await?;
    let events = c.get("/api/events?limit=5").await.unwrap_or(Value::Null);
    Ok(build(&stats, &events))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::app::testing::{integration, serve};
    use crate::integrations::IntegrationKind;

    #[test]
    fn cameras() {
        let stats = json!({
            "cameras": {
                "garage": {"camera_fps": 0.0, "detection_fps": 0.0},
                "front": {"camera_fps": 5.1, "detection_fps": 1.2, "skipped_fps": 0.0}
            },
            "detectors": {"coral": {"inference_speed": 8.42}},
            "service": {"version": "0.15.1-abc", "storage": {"/media/frigate/recordings": {"total": 1000000.0, "used": 950000.0}}}
        });
        let st = build(&stats, &json!([{"label": "person", "camera": "front"}]));
        assert_eq!(st.summary, "2 cameras · 1 offline");
        assert_eq!(st.rows[0].title, "front");
        assert_eq!(st.rows[0].meta, "5.1 fps · 1.2 detect fps");
        assert_eq!(st.rows[1].state, AppRowState::Bad);
        assert_eq!(st.health, AppHealth::Bad);
        assert!(st.tiles.iter().any(|t| t.label == "Inference" && t.value == "8.4 ms"));
        assert!(st.tiles.iter().any(|t| t.label == "Latest" && t.value == "person on front"));
        assert_eq!(st.notes, ["Recording storage is over 90 % full"]);
    }

    #[tokio::test]
    async fn login_when_set() {
        let (url, seen) = serve(3, |m, p, head, _| match (m, p) {
            ("POST", "/api/login") => {
                (200, vec![("set-cookie".into(), "frigate_token=t1; Path=/".into())], "{}".into())
            }
            (_, _) if !head.to_ascii_lowercase().contains("cookie: frigate_token=t1") => (401, vec![], String::new()),
            ("GET", "/api/stats") => (200, vec![], r#"{"cameras":{}}"#.into()),
            _ => (200, vec![], "[]".into()),
        })
        .await;
        let mut i = integration(IntegrationKind::Frigate, &url);
        i.username = "admin".into();
        let st = status(&i, "pw").await.unwrap();
        assert_eq!(st.summary, "0 cameras");
        assert_eq!(seen.await.unwrap()[0].body, r#"{"password":"pw","user":"admin"}"#);
    }
}
