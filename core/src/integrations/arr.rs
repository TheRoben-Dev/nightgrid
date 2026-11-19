//! The *arr apps: Sonarr, Radarr and Lidarr (download queue, missing items,
//! health, RSS sync and missing searches), Prowlarr (indexers and their
//! failures) and Bazarr (missing subtitles). All take the API key from
//! Settings → General in an `X-Api-Key` header.

use serde_json::{json, Value};

use super::app::{
    bytes, cap_rows, count, duration_hms, n, plural, s, valid_target, AppAction, AppHealth, AppRow, AppRowState,
    AppStatus, Client,
};
use super::{Integration, IntegrationKind};

/// API root and display name per app.
fn api(kind: IntegrationKind) -> (&'static str, &'static str) {
    match kind {
        IntegrationKind::Sonarr => ("/api/v3", "Sonarr"),
        IntegrationKind::Radarr => ("/api/v3", "Radarr"),
        IntegrationKind::Lidarr => ("/api/v1", "Lidarr"),
        IntegrationKind::Prowlarr => ("/api/v1", "Prowlarr"),
        _ => ("/api", "Bazarr"),
    }
}

fn client<'a>(i: &'a Integration, key: &str) -> Client<'a> {
    let (_, name) = api(i.kind);
    let header = if i.kind == IntegrationKind::Bazarr { "X-API-KEY" } else { "X-Api-Key" };
    Client::new(i, name).header(header, key)
}

/// The command each top-level action starts (`POST <api>/command`).
fn command(kind: IntegrationKind, action: &str) -> Option<&'static str> {
    Some(match (kind, action) {
        (IntegrationKind::Sonarr | IntegrationKind::Radarr | IntegrationKind::Lidarr, "rss") => "RssSync",
        (IntegrationKind::Sonarr, "missing") => "MissingEpisodeSearch",
        (IntegrationKind::Radarr, "missing") => "MissingMoviesSearch",
        (IntegrationKind::Lidarr, "missing") => "MissingAlbumSearch",
        _ => return None,
    })
}

fn queue_title(kind: IntegrationKind, r: &Value) -> String {
    let t = match kind {
        IntegrationKind::Sonarr => {
            let series = s(&r["series"], "title");
            let ep = &r["episode"];
            if series.is_empty() {
                String::new()
            } else if ep.is_object() {
                format!(
                    "{series} · S{:02}E{:02} · {}",
                    count(ep, "seasonNumber"),
                    count(ep, "episodeNumber"),
                    s(ep, "title")
                )
            } else {
                series
            }
        }
        IntegrationKind::Radarr => {
            let m = &r["movie"];
            match (s(m, "title"), s(m, "year")) {
                (t, _) if t.is_empty() => String::new(),
                (t, y) if y.is_empty() || y == "0" => t,
                (t, y) => format!("{t} ({y})"),
            }
        }
        IntegrationKind::Lidarr => {
            let (artist, album) = (s(&r["artist"], "artistName"), s(&r["album"], "title"));
            [artist, album].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ")
        }
        _ => String::new(),
    };
    if t.is_empty() {
        s(r, "title")
    } else {
        t
    }
}

pub fn parse_queue(kind: IntegrationKind, v: &Value) -> (Vec<AppRow>, u64) {
    let total = count(v, "totalRecords");
    let rows = v["records"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|r| {
            let status = s(r, "status").to_ascii_lowercase();
            let tracked = s(r, "trackedDownloadStatus").to_ascii_lowercase();
            let state = match (status.as_str(), tracked.as_str()) {
                (_, "error") | ("failed", _) => AppRowState::Bad,
                (_, "warning") | ("warning", _) => AppRowState::Warn,
                ("downloading", _) => AppRowState::Active,
                ("completed", _) => AppRowState::Ok,
                _ => AppRowState::Idle,
            };
            let size = n(r, "size").unwrap_or(0.0);
            let left = n(r, "sizeleft").unwrap_or(0.0);
            let progress = (size > 0.0).then(|| (size - left) / size * 100.0);
            let mut meta = vec![s(r, "trackedDownloadState").to_lowercase()];
            if meta[0].is_empty() {
                meta[0] = status.clone();
            }
            if size > 0.0 {
                meta.push(bytes(size));
            }
            let tl = s(r, "timeleft");
            if !tl.is_empty() && status == "downloading" {
                meta.push(format!("{} left", duration_hms(&tl)));
            }
            let msg = r["statusMessages"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|m| m["messages"].as_array())
                .and_then(|m| m.first())
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| Some(s(r, "errorMessage")).filter(|e| !e.is_empty()));
            if let Some(m) = msg {
                meta.push(m);
            }
            meta.retain(|m| !m.is_empty());
            let title = queue_title(kind, r);
            let id = s(r, "id");
            let mut row = AppRow::new(&id, &title, meta.join(" · "), state).progress(progress);
            if valid_target(&id) {
                row = row.action(AppAction::new("remove", "Remove").confirm(format!(
                    "Remove \"{title}\" from the queue? The download client keeps it; only the app stops tracking it."
                )));
            }
            row
        })
        .collect();
    (rows, total)
}

/// AppHealth checks → notes and overall health.
pub fn apply_health(st: &mut AppStatus, v: &Value) {
    for h in v.as_array().map(|a| a.as_slice()).unwrap_or_default() {
        let level = match s(h, "type").to_ascii_lowercase().as_str() {
            "error" => AppHealth::Bad,
            "warning" => AppHealth::Warn,
            _ => continue,
        };
        st.health = st.health.max(level);
        st.notes.push(s(h, "message"));
    }
}

pub fn build_downloader(
    kind: IntegrationKind,
    sys: &Value,
    queue: &Value,
    health: &Value,
    missing: Option<&Value>,
) -> AppStatus {
    let (mut rows, total) = parse_queue(kind, queue);
    let missing = missing.map(|m| count(m, "totalRecords"));
    let mut summary = if total == 0 { "Queue empty".to_owned() } else { format!("{total} in queue") };
    if let Some(m) = missing.filter(|m| *m > 0) {
        summary.push_str(&format!(" · {m} missing"));
    }
    let mut st = AppStatus::new(summary).tile("Queue", total);
    if let Some(m) = missing {
        st = st.tile("Missing", m);
    }
    let failing = rows.iter().filter(|r| matches!(r.state, AppRowState::Warn | AppRowState::Bad)).count() as u64;
    if failing > 0 {
        st = st.tile("Need attention", failing);
    }
    st.version = Some(s(sys, "version")).filter(|v| !v.is_empty());
    apply_health(&mut st, health);
    st.more = cap_rows(&mut rows).max(total.saturating_sub(rows.len() as u64) as u32);
    st.rows_title = "Queue".into();
    st.rows = rows;
    let what = match kind {
        IntegrationKind::Sonarr => "episode",
        IntegrationKind::Radarr => "movie",
        _ => "album",
    };
    st.actions = vec![
        AppAction::new("rss", "RSS sync"),
        AppAction::new("missing", "Search missing")
            .confirm(format!("Search for every missing {what}? It runs in the background and uses indexer API calls.")),
    ];
    st
}

pub fn build_prowlarr(sys: &Value, indexers: &Value, failing: &Value, health: &Value) -> AppStatus {
    let list = indexers.as_array().map(|a| a.as_slice()).unwrap_or_default();
    let enabled = list.iter().filter(|x| x["enable"].as_bool().unwrap_or(false)).count() as u64;
    let name_of = |id: &str| {
        list.iter().find(|x| s(x, "id") == id).map(|x| s(x, "name")).unwrap_or_else(|| format!("Indexer {id}"))
    };
    let mut rows: Vec<AppRow> = failing
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|f| {
            let id = s(f, "indexerId");
            let until = s(f, "disabledTill");
            let meta = if until.is_empty() {
                "failing".to_owned()
            } else {
                format!("disabled until {}", until.replace('T', " ").trim_end_matches('Z'))
            };
            AppRow::new(&id, name_of(&id), meta, AppRowState::Warn)
        })
        .collect();
    let bad = rows.len() as u64;
    let summary = format!(
        "{}/{} enabled{}",
        enabled,
        list.len(),
        if bad > 0 { format!(" · {bad} failing") } else { String::new() }
    );
    let mut st = AppStatus::new(summary).tile("Indexers", list.len()).tile("Enabled", enabled).tile("Failing", bad);
    st.version = Some(s(sys, "version")).filter(|v| !v.is_empty());
    apply_health(&mut st, health);
    if bad > 0 {
        st.health = st.health.max(AppHealth::Warn);
    }
    st.more = cap_rows(&mut rows);
    st.rows_title = "Failing indexers".into();
    st.rows = rows;
    st.actions = vec![AppAction::new("testall", "Test indexers")];
    st
}

pub fn build_bazarr(sys: &Value, badges: &Value, health: Option<&Value>) -> AppStatus {
    let (eps, movies, providers) = (count(badges, "episodes"), count(badges, "movies"), count(badges, "providers"));
    let summary = if eps + movies == 0 {
        "No missing subtitles".to_owned()
    } else {
        format!("{} missing subtitles", eps + movies)
    };
    let mut st = AppStatus::new(summary).tile("Episodes", eps).tile("Movies", movies);
    if providers > 0 {
        st = st.tile("Throttled providers", providers).warn(format!("{} throttled", plural(providers, "provider")));
    }
    st.version = Some(s(&sys["data"], "bazarr_version")).filter(|v| !v.is_empty());
    for h in health.and_then(|h| h["data"].as_array()).map(|a| a.as_slice()).unwrap_or_default() {
        let msg = [s(h, "object"), s(h, "issue")].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(": ");
        st = st.warn(msg);
    }
    st
}

pub async fn status(i: &Integration, key: &str) -> Result<AppStatus, String> {
    let c = client(i, key);
    let (root, _) = api(i.kind);
    match i.kind {
        IntegrationKind::Prowlarr => {
            let sys = c.get(&format!("{root}/system/status")).await?;
            let indexers = c.get(&format!("{root}/indexer")).await?;
            let failing = c.get(&format!("{root}/indexerstatus")).await?;
            let health = c.get(&format!("{root}/health")).await?;
            Ok(build_prowlarr(&sys, &indexers, &failing, &health))
        }
        IntegrationKind::Bazarr => {
            let sys = c.get("/api/system/status").await?;
            let badges = c.get("/api/badges").await?;
            let health = c.get_opt("/api/system/health").await.ok().flatten();
            Ok(build_bazarr(&sys, &badges, health.as_ref()))
        }
        kind => {
            let include = match kind {
                IntegrationKind::Sonarr => "includeSeries=true&includeEpisode=true",
                IntegrationKind::Radarr => "includeMovie=true",
                _ => "includeArtist=true&includeAlbum=true",
            };
            let sys = c.get(&format!("{root}/system/status")).await?;
            let queue = c.get(&format!("{root}/queue?page=1&pageSize=50&{include}")).await?;
            let health = c.get(&format!("{root}/health")).await?;
            // Radarr only gained wanted/missing in v5.
            let missing = c.get_opt(&format!("{root}/wanted/missing?page=1&pageSize=1")).await.ok().flatten();
            Ok(build_downloader(kind, &sys, &queue, &health, missing.as_ref()))
        }
    }
}

pub async fn act(i: &Integration, key: &str, target: &str, action: &str) -> Result<(), String> {
    let c = client(i, key);
    let (root, _) = api(i.kind);
    match (i.kind, action) {
        (IntegrationKind::Prowlarr, "testall") => c.post_empty(&format!("{root}/indexer/testall")).await.map(drop),
        (IntegrationKind::Sonarr | IntegrationKind::Radarr | IntegrationKind::Lidarr, "remove") => {
            if !valid_target(target) || !target.chars().all(|c| c.is_ascii_digit()) {
                return Err("not a queue item".into());
            }
            c.delete(&format!("{root}/queue/{target}?removeFromClient=false&blocklist=false")).await.map(drop)
        }
        (kind, a) => match command(kind, a) {
            Some(name) => c.post(&format!("{root}/command"), &json!({ "name": name })).await.map(drop),
            None => Err(format!("unknown action {action}")),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::app::testing::{integration, serve};

    #[test]
    fn sonarr_queue() {
        let q: Value = serde_json::from_str(include_str!("fixtures/sonarr_queue.json")).unwrap();
        let health = json!([
            {"source": "IndexerStatusCheck", "type": "warning", "message": "Indexers unavailable due to failures: Example"},
            {"source": "UpdateCheck", "type": "notice", "message": "ignored"}
        ]);
        let st = build_downloader(
            IntegrationKind::Sonarr,
            &json!({"version": "4.0.15.2941"}),
            &q,
            &health,
            Some(&json!({"totalRecords": 12})),
        );
        assert_eq!(st.summary, "3 in queue · 12 missing");
        assert_eq!(st.health, AppHealth::Warn);
        assert_eq!(st.notes, ["Indexers unavailable due to failures: Example"]);
        assert_eq!(st.rows[0].title, "Example Show · S02E05 · The Fifth One");
        assert_eq!(st.rows[0].state, AppRowState::Active);
        assert_eq!(st.rows[0].meta, "downloading · 1.0 GB · 12m 30s left");
        assert!((st.rows[0].progress.unwrap() - 75.0).abs() < 0.01);
        assert_eq!(st.rows[1].state, AppRowState::Warn);
        assert!(st.rows[1].meta.contains("No files found are eligible for import"));
        assert_eq!(st.rows[2].title, "Some.Release.Name.S01E01");
        assert_eq!(st.rows[0].actions[0].id, "remove");
        assert!(st.actions.iter().any(|a| a.id == "missing" && a.confirm.is_some()));
        assert_eq!(command(IntegrationKind::Radarr, "missing"), Some("MissingMoviesSearch"));
        assert_eq!(command(IntegrationKind::Prowlarr, "missing"), None);
    }

    #[test]
    fn radarr_and_lidarr_titles() {
        let r = json!({"title": "x", "movie": {"title": "A Film", "year": 2019}});
        assert_eq!(queue_title(IntegrationKind::Radarr, &r), "A Film (2019)");
        let l = json!({"title": "x", "artist": {"artistName": "Some Band"}, "album": {"title": "First"}});
        assert_eq!(queue_title(IntegrationKind::Lidarr, &l), "Some Band · First");
    }

    #[test]
    fn prowlarr() {
        let idx = json!([{"id": 1, "name": "Alpha", "enable": true}, {"id": 2, "name": "Beta", "enable": true}, {"id": 3, "name": "Gamma", "enable": false}]);
        let failing = json!([{"indexerId": 2, "disabledTill": "2026-11-19T20:00:00Z"}]);
        let st = build_prowlarr(&json!({"version": "1.30.0"}), &idx, &failing, &json!([]));
        assert_eq!(st.summary, "2/3 enabled · 1 failing");
        assert_eq!(st.rows[0].title, "Beta");
        assert_eq!(st.rows[0].meta, "disabled until 2026-11-19 20:00:00");
        assert_eq!(st.health, AppHealth::Warn);
    }

    #[test]
    fn bazarr() {
        let st = build_bazarr(
            &json!({"data": {"bazarr_version": "1.5.1"}}),
            &json!({"episodes": 7, "movies": 2, "providers": 1}),
            Some(&json!({"data": [{"object": "Sonarr", "issue": "unreachable"}]})),
        );
        assert_eq!(st.summary, "9 missing subtitles");
        assert_eq!(st.version.as_deref(), Some("1.5.1"));
        assert_eq!(st.notes, ["1 provider throttled", "Sonarr: unreachable"]);
    }

    #[tokio::test]
    async fn requests_and_actions() {
        let (url, seen) = serve(5, |m, p, _, _| match (m, p) {
            ("GET", "/api/v3/system/status") => (200, vec![], r#"{"version":"5.2"}"#.into()),
            ("GET", p) if p.starts_with("/api/v3/queue") => (200, vec![], r#"{"totalRecords":0,"records":[]}"#.into()),
            ("GET", "/api/v3/health") => (200, vec![], "[]".into()),
            ("POST", "/api/v3/command") => (201, vec![], r#"{"id":1}"#.into()),
            _ => (404, vec![], String::new()),
        })
        .await;
        let i = integration(IntegrationKind::Radarr, &url);
        let st = status(&i, "k").await.unwrap();
        assert_eq!(st.summary, "Queue empty");
        assert!(st.tiles.iter().all(|t| t.label != "Missing"), "old Radarr without wanted/missing");
        act(&i, "k", "", "missing").await.unwrap();
        assert!(act(&i, "k", "../x", "remove").await.is_err());
        assert!(act(&i, "k", "", "nope").await.is_err());
        let seen = seen.await.unwrap();
        assert!(seen.iter().all(|r| r.head.to_ascii_lowercase().contains("x-api-key: k")));
        assert_eq!(seen[4].body, r#"{"name":"MissingMoviesSearch"}"#);
    }
}
