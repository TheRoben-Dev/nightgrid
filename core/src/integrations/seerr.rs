//! Overseerr and Jellyseerr (same API): request counts, pending requests
//! with approve and decline, and update notices. Uses the API key from
//! Settings → General in an `X-Api-Key` header.

use serde_json::{json, Value};

use super::app::{cap_rows, count, s, AppAction, AppRow, AppRowState, AppStatus, Client};
use super::{Integration, IntegrationKind};

/// Titles looked up per read (each is one call to the app).
const TITLE_LOOKUPS: usize = 6;

fn client<'a>(i: &'a Integration, key: &str) -> Client<'a> {
    let name = if i.kind == IntegrationKind::Jellyseerr { "Jellyseerr" } else { "Overseerr" };
    Client::new(i, name).header("X-Api-Key", key)
}

/// Pending requests as rows. `titles` maps a request id to a looked-up title.
pub fn pending_rows(v: &Value, titles: &[(String, String)]) -> Vec<AppRow> {
    v["results"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|r| {
            let id = s(r, "id");
            let kind = if s(r, "type") == "tv" { "Series" } else { "Movie" };
            let title = titles
                .iter()
                .find(|(k, _)| *k == id)
                .map(|(_, t)| t.clone())
                .unwrap_or_else(|| format!("{kind} request #{id}"));
            let by = s(&r["requestedBy"], "displayName");
            let mut meta = vec![kind.to_owned()];
            let seasons = r["seasons"].as_array().map(Vec::len).unwrap_or(0);
            if seasons > 0 {
                meta.push(format!("{seasons} season{}", if seasons == 1 { "" } else { "s" }));
            }
            if r["is4k"].as_bool() == Some(true) {
                meta.push("4K".into());
            }
            if !by.is_empty() {
                meta.push(format!("by {by}"));
            }
            AppRow::new(&id, &title, meta.join(" · "), AppRowState::Idle)
                .action(AppAction::new("approve", "Approve"))
                .action(AppAction::new("decline", "Decline").confirm(format!("Decline the request for \"{title}\"?")))
        })
        .collect()
}

pub fn build(status: &Value, counts: &Value, pending: &Value, titles: &[(String, String)]) -> AppStatus {
    let p = count(counts, "pending");
    let summary = format!("{} pending · {} available", p, count(counts, "available"));
    let mut st = AppStatus::new(summary)
        .tile("Pending", p)
        .tile("Processing", count(counts, "processing"))
        .tile("Available", count(counts, "available"))
        .tile("Total", count(counts, "total"));
    st.version = Some(s(status, "version")).filter(|v| !v.is_empty());
    if status["updateAvailable"].as_bool() == Some(true) {
        st = st.warn("An update is available");
    }
    let mut rows = pending_rows(pending, titles);
    st.more = cap_rows(&mut rows).max((p as usize).saturating_sub(rows.len()) as u32);
    st.rows_title = "Pending requests".into();
    st.rows = rows;
    st
}

fn year(d: &str) -> Option<&str> {
    d.get(..4).filter(|y| y.chars().all(|c| c.is_ascii_digit()))
}

pub async fn status(i: &Integration, key: &str) -> Result<AppStatus, String> {
    let c = client(i, key);
    let status = c.get("/api/v1/status").await?;
    let counts = c.get("/api/v1/request/count").await?;
    let pending = c.get("/api/v1/request?take=25&skip=0&filter=pending&sort=added").await?;
    let mut titles = Vec::new();
    for r in pending["results"].as_array().map(|a| a.as_slice()).unwrap_or_default().iter().take(TITLE_LOOKUPS) {
        let tmdb = s(&r["media"], "tmdbId");
        if tmdb.is_empty() || !tmdb.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let tv = s(r, "type") == "tv";
        let Ok(Some(m)) = c.get_opt(&format!("/api/v1/{}/{tmdb}", if tv { "tv" } else { "movie" })).await else {
            continue;
        };
        let (name, date) =
            if tv { (s(&m, "name"), s(&m, "firstAirDate")) } else { (s(&m, "title"), s(&m, "releaseDate")) };
        if !name.is_empty() {
            let t = match year(&date) {
                Some(y) => format!("{name} ({y})"),
                None => name,
            };
            titles.push((s(r, "id"), t));
        }
    }
    Ok(build(&status, &counts, &pending, &titles))
}

pub async fn act(i: &Integration, key: &str, target: &str, action: &str) -> Result<(), String> {
    if target.is_empty() || !target.chars().all(|c| c.is_ascii_digit()) {
        return Err("not a request".into());
    }
    let what = match action {
        "approve" => "approve",
        "decline" => "decline",
        _ => return Err(format!("unknown action {action}")),
    };
    client(i, key).post(&format!("/api/v1/request/{target}/{what}"), &json!({})).await.map(drop)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::app::testing::{integration, serve};

    #[test]
    fn requests() {
        let pending = json!({"pageInfo": {"results": 2}, "results": [
            {"id": 7, "type": "movie", "is4k": true, "requestedBy": {"displayName": "alice"}, "media": {"tmdbId": 11}},
            {"id": 8, "type": "tv", "seasons": [{"seasonNumber": 1}, {"seasonNumber": 2}], "requestedBy": {"displayName": "bob"}, "media": {"tmdbId": 22}}
        ]});
        let st = build(
            &json!({"version": "1.33.2", "updateAvailable": true}),
            &json!({"total": 40, "pending": 2, "processing": 3, "available": 30}),
            &pending,
            &[("7".into(), "A Film (2019)".into())],
        );
        assert_eq!(st.summary, "2 pending · 30 available");
        assert_eq!(st.rows[0].title, "A Film (2019)");
        assert_eq!(st.rows[0].meta, "Movie · 4K · by alice");
        assert_eq!(st.rows[1].title, "Series request #8");
        assert_eq!(st.rows[1].meta, "Series · 2 seasons · by bob");
        assert!(st.rows[1].actions[1].confirm.is_some());
        assert_eq!(st.notes, ["An update is available"]);
    }

    #[tokio::test]
    async fn titles_and_approve() {
        let (url, seen) = serve(6, |m, p, _, _| match (m, p) {
            ("GET", "/api/v1/status") => (200, vec![], r#"{"version":"2.5.0"}"#.into()),
            ("GET", "/api/v1/request/count") => (200, vec![], r#"{"pending":1,"available":0,"total":1}"#.into()),
            ("GET", p) if p.starts_with("/api/v1/request?") => {
                (200, vec![], r#"{"results":[{"id":3,"type":"tv","media":{"tmdbId":99}}]}"#.into())
            }
            ("GET", "/api/v1/tv/99") => (200, vec![], r#"{"name":"Example Show","firstAirDate":"2021-04-02"}"#.into()),
            ("POST", "/api/v1/request/3/approve") => (200, vec![], "{}".into()),
            _ => (404, vec![], String::new()),
        })
        .await;
        let i = integration(IntegrationKind::Jellyseerr, &url);
        let st = status(&i, "k").await.unwrap();
        assert_eq!(st.rows[0].title, "Example Show (2021)");
        act(&i, "k", "3", "approve").await.unwrap();
        assert!(act(&i, "k", "3/../1", "approve").await.is_err());
        assert!(act(&i, "k", "3", "delete").await.is_err());
        // One spare accept so the server task ends: a probe that 404s.
        let _ = Client::new(&i, "x").get_opt("/end").await;
        let seen = seen.await.unwrap();
        assert!(seen.iter().take(5).all(|r| r.head.to_ascii_lowercase().contains("x-api-key: k")));
    }
}
