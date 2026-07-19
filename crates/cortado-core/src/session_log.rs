//! Read-side of logs/sessions.jsonl: derive per-session facts from the
//! append-only spawn log (the on-disk truth — no live store to drift).
use crate::session_name::SessionName;
use crate::store::Store;
use std::collections::BTreeMap;

/// Model per live session name: the last `spawn` event for that name in its
/// agent's sessions.jsonl. Corrupt lines are skipped; missing logs yield
/// nothing (the TUI renders "?"). One file read per referenced agent.
pub fn models_for(store: &Store, names: &[String]) -> BTreeMap<String, String> {
    let mut wanted: BTreeMap<std::path::PathBuf, Vec<&str>> = BTreeMap::new();
    for name in names {
        if let Some(parsed) = SessionName::parse(name) {
            wanted
                .entry(
                    store
                        .logs_dir(&parsed.team, &parsed.agent)
                        .join("sessions.jsonl"),
                )
                .or_default()
                .push(name);
        }
    }
    let mut out = BTreeMap::new();
    for (path, session_names) in wanted {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        for line in text.lines() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if v["event"] != "spawn" {
                continue;
            }
            let (Some(session), Some(model)) = (v["session"].as_str(), v["model"].as_str()) else {
                continue;
            };
            if session_names.contains(&session) {
                out.insert(session.to_string(), model.to_string()); // later lines overwrite
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::slug::Slug;

    #[test]
    fn last_spawn_wins_and_unknown_names_are_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let store = Store::new(tmp.path().to_path_buf());
        let team = Slug::parse("newsletter").unwrap();
        let agent = Slug::parse("scout").unwrap();
        let dir = store.logs_dir(&team, &agent);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("sessions.jsonl"),
            concat!(
                r#"{"ts":"2026-07-01T00:00:00Z","event":"spawn","session":"cortado_newsletter_scout_1","model":"claude","runtime":"claude"}"#, "\n",
                r#"not json — a corrupt line must be skipped, not fatal"#, "\n",
                r#"{"ts":"2026-07-02T00:00:00Z","event":"spawn","session":"cortado_newsletter_scout_1","model":"kimi","runtime":"claude"}"#, "\n",
                r#"{"ts":"2026-07-02T00:00:00Z","event":"reflect","session":"cortado_newsletter_scout_1","model":"ignored","runtime":"claude"}"#, "\n",
            ),
        )
        .unwrap();

        let names = vec![
            "cortado_newsletter_scout_1".to_string(),
            "cortado_newsletter_scout_2".to_string(), // live but never logged
            "garbage-name".to_string(),               // unparseable → skipped
        ];
        let models = models_for(&store, &names);
        assert_eq!(models.get("cortado_newsletter_scout_1").unwrap(), "kimi");
        assert!(!models.contains_key("cortado_newsletter_scout_2"));
        assert!(!models.contains_key("garbage-name"));
    }
}
