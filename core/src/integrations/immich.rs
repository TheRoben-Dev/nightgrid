//! Immich: library size and background jobs (thumbnails, face detection,
//! ...), with an `x-api-key` from Account Settings → API Keys. Statistics and
//! jobs need an admin's key; with a user key the card shows what it can.

use serde_json::Value;

use super::app::{bytes, cap_rows, count, plural, s, AppHealth, AppRow, AppRowState, AppStatus, Client};
use super::Integration;

fn version(v: &Value) -> Option<String> {
    let full = s(v, "version");
    if !full.is_empty() {
        return Some(full);
    }
    let (a, b, c) = (s(v, "major"), s(v, "minor"), s(v, "patch"));
    (!a.is_empty()).then(|| format!("v{a}.{b}.{c}"))
}

/// "thumbnailGeneration" → "Thumbnail generation".
fn job_name(k: &str) -> String {
    let mut out = String::new();
    for (n, ch) in k.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            out.push(' ');
            out.push(ch.to_ascii_lowercase());
        } else if n == 0 {
            out.push(ch.to_ascii_uppercase());
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn build(about: &Value, stats: Option<&Value>, jobs: Option<&Value>) -> AppStatus {
    let mut rows = Vec::new();
    let mut failed = 0;
    if let Some(map) = jobs.and_then(Value::as_object) {
        for (k, j) in map {
            let c = &j["jobCounts"];
            let (active, waiting, f) =
                (count(c, "active"), count(c, "waiting") + count(c, "delayed"), count(c, "failed"));
            failed += f;
            let paused = j["queueStatus"]["isPaused"].as_bool().unwrap_or(false);
            if active + waiting + f == 0 && !paused {
                continue;
            }
            let mut meta = Vec::new();
            if active > 0 {
                meta.push(format!("{active} running"));
            }
            if waiting > 0 {
                meta.push(format!("{waiting} waiting"));
            }
            if f > 0 {
                meta.push(format!("{f} failed"));
            }
            if paused {
                meta.push("paused".into());
            }
            let state = if f > 0 {
                AppRowState::Warn
            } else if active > 0 {
                AppRowState::Active
            } else {
                AppRowState::Idle
            };
            rows.push(AppRow::new(k, job_name(k), meta.join(" · "), state));
        }
    }
    let running = rows.iter().filter(|r| r.state == AppRowState::Active).count() as u64;
    let mut st = AppStatus::new(match stats {
        Some(st) => format!(
            "{} · {} · {}",
            plural(count(st, "photos"), "photo"),
            plural(count(st, "videos"), "video"),
            bytes(count(st, "usage") as f64)
        ),
        None => "Reachable".to_owned(),
    });
    if let Some(x) = stats {
        st = st
            .tile("Photos", count(x, "photos"))
            .tile("Videos", count(x, "videos"))
            .tile("Storage", bytes(count(x, "usage") as f64))
            .tile("Users", x["usageByUser"].as_array().map(Vec::len).unwrap_or(0));
    }
    if jobs.is_some() {
        st = st.tile("Jobs running", running);
    }
    if failed > 0 {
        st.health = AppHealth::Warn;
        st.notes.push(format!("{} failed", plural(failed, "job")));
    }
    if stats.is_none() && jobs.is_none() {
        st.notes.push("Use an admin's API key to see library size and jobs".into());
    }
    st.version = version(about);
    st.more = cap_rows(&mut rows);
    st.rows_title = "Jobs".into();
    st.rows = rows;
    st
}

pub async fn status(i: &Integration, key: &str) -> Result<AppStatus, String> {
    let c = Client::new(i, "Immich").header("x-api-key", key);
    let about = match c.get_opt("/api/server/about").await? {
        Some(v) => v,
        None => c.get("/api/server/version").await?,
    };
    let stats = c.get("/api/server/statistics").await.ok();
    let jobs = c.get("/api/jobs").await.ok();
    Ok(build(&about, stats.as_ref(), jobs.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn library_and_jobs() {
        let stats = json!({"photos": 12034, "videos": 410, "usage": 107374182400u64, "usageByUser": [{}, {}]});
        let jobs = json!({
            "thumbnailGeneration": {"jobCounts": {"active": 2, "waiting": 40, "failed": 0}, "queueStatus": {"isPaused": false}},
            "faceDetection": {"jobCounts": {"active": 0, "waiting": 0, "failed": 3}, "queueStatus": {"isPaused": false}},
            "library": {"jobCounts": {"active": 0, "waiting": 0, "failed": 0}, "queueStatus": {"isPaused": false}}
        });
        let st = build(&json!({"version": "v1.132.3"}), Some(&stats), Some(&jobs));
        assert_eq!(st.summary, "12034 photos · 410 videos · 100 GB");
        assert_eq!(st.rows.len(), 2);
        let t: Vec<_> = st.rows.iter().map(|r| (r.title.as_str(), r.meta.as_str())).collect();
        assert!(t.contains(&("Thumbnail generation", "2 running · 40 waiting")));
        assert!(t.contains(&("Face detection", "3 failed")));
        assert_eq!(st.health, AppHealth::Warn);
        let user = build(&json!({"major": 1, "minor": 90, "patch": 0}), None, None);
        assert_eq!((user.summary.as_str(), user.version.as_deref()), ("Reachable", Some("v1.90.0")));
    }
}
