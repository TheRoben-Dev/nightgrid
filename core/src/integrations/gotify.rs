//! Gotify: health, applications and the latest messages, with delete. Needs a
//! client token (Gotify → Clients), sent as `X-Gotify-Key`.

use serde_json::Value;

use super::app::{count, plural, s, AppAction, AppHealth, AppRow, AppRowState, AppStatus, Client};
use super::Integration;

fn client<'a>(i: &'a Integration, key: &str) -> Client<'a> {
    Client::new(i, "Gotify").header("X-Gotify-Key", key)
}

pub fn build(version: &Value, health: &Value, apps: &Value, messages: &Value) -> AppStatus {
    let apps: Vec<&Value> = apps.as_array().map(|a| a.iter().collect()).unwrap_or_default();
    let app_name = |id: &str| apps.iter().find(|a| s(a, "id") == id).map(|a| s(a, "name")).unwrap_or_default();
    let rows: Vec<AppRow> = messages["messages"]
        .as_array()
        .map(|a| a.as_slice())
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let p = count(m, "priority");
            let state = match p {
                8.. => AppRowState::Bad,
                4..=7 => AppRowState::Warn,
                _ => AppRowState::Ok,
            };
            let title = Some(s(m, "title")).filter(|t| !t.is_empty()).unwrap_or_else(|| s(m, "message"));
            let date = s(m, "date");
            let when = date.get(..16).unwrap_or(&date).replace('T', " ");
            let meta = [app_name(&s(m, "appid")), format!("priority {p}"), when, s(m, "message")]
                .into_iter()
                .filter(|x| !x.is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            AppRow::new(s(m, "id"), title, meta, state).action(AppAction::new("delete", "Delete"))
        })
        .collect();
    let total = count(&messages["paging"], "size").max(rows.len() as u64);
    let mut st = AppStatus::new(format!("{} · {}", plural(total, "message"), plural(apps.len() as u64, "app")))
        .tile("Messages", total)
        .tile("Apps", apps.len());
    for (k, label) in [("health", "Gotify"), ("database", "Its database")] {
        let v = s(health, k);
        if !v.is_empty() && v != "green" {
            st.health = AppHealth::Bad;
            st.notes.push(format!("{label} reports {v}"));
        }
    }
    st.version = Some(s(version, "version")).filter(|v| !v.is_empty());
    st.more = (total as usize).saturating_sub(rows.len()) as u32;
    st.rows_title = "Latest messages".into();
    st.rows = rows;
    st.actions = vec![AppAction::new("delete-all", "Delete all").confirm("Delete every Gotify message for all apps?")];
    st
}

pub async fn status(i: &Integration, key: &str) -> Result<AppStatus, String> {
    let c = client(i, key);
    let version = c.get("/version").await.unwrap_or(Value::Null);
    let health = c.get("/health").await.unwrap_or(Value::Null);
    let apps = c.get("/application").await?;
    let messages = c.get("/message?limit=15").await?;
    Ok(build(&version, &health, &apps, &messages))
}

pub async fn act(i: &Integration, key: &str, target: &str, action: &str) -> Result<(), String> {
    let c = client(i, key);
    match action {
        "delete" if !target.is_empty() && target.chars().all(|c| c.is_ascii_digit()) => {
            c.delete(&format!("/message/{target}")).await.map(drop)
        }
        "delete-all" => c.delete("/message").await.map(drop),
        _ => Err(format!("unknown action {action}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn messages() {
        let msgs = json!({"messages": [
            {"id": 9, "appid": 1, "title": "Backup failed", "message": "restic exit 1", "priority": 8, "date": "2026-11-19T03:00:12.5+01:00"},
            {"id": 8, "appid": 2, "title": "", "message": "Hello", "priority": 0, "date": "2026-11-18T10:00:00Z"}
        ], "paging": {"size": 40}});
        let apps = json!([{"id": 1, "name": "Backups"}, {"id": 2, "name": "Test"}]);
        let st = build(&json!({"version": "2.6.1"}), &json!({"health": "green", "database": "green"}), &apps, &msgs);
        assert_eq!(st.summary, "40 messages · 2 apps");
        assert_eq!(st.rows[0].meta, "Backups · priority 8 · 2026-11-19 03:00 · restic exit 1");
        assert_eq!(st.rows[0].state, AppRowState::Bad);
        assert_eq!(st.rows[1].title, "Hello");
        assert_eq!(st.more, 38);
        assert_eq!(st.health, AppHealth::Ok);
        let sick = build(&json!({}), &json!({"health": "orange", "database": "red"}), &json!([]), &json!({}));
        assert_eq!(sick.notes, ["Gotify reports orange", "Its database reports red"]);
    }
}
