//! Herdr wrapper: shells out to the `herdr` CLI and parses its JSON envelopes.
//! Stateless — every question re-asks Herdr; nothing is cached.
use std::collections::BTreeMap;
use std::path::Path;

#[derive(thiserror::Error, Debug)]
pub enum HerdrError {
    #[error("herdr is not installed — brew install herdr (see `cortado doctor`)")]
    NotInstalled,
    #[error("herdr server is not running: {0}")]
    ServerDown(String),
    #[error("herdr {args} failed: {stderr}")]
    Failed { args: String, stderr: String },
    #[error("cannot parse herdr output: {0}")]
    Parse(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Idle,
    Working,
    Blocked,
    Unknown,
}

impl AgentStatus {
    pub fn parse(s: &str) -> AgentStatus {
        match s {
            "idle" => AgentStatus::Idle,
            "working" => AgentStatus::Working,
            "blocked" => AgentStatus::Blocked,
            _ => AgentStatus::Unknown,
        }
    }
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            AgentStatus::Idle => "idle",
            AgentStatus::Working => "working",
            AgentStatus::Blocked => "blocked",
            AgentStatus::Unknown => "?",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentInfo {
    pub name: String,
    pub pane_id: String,
    pub workspace_id: String,
    pub status: AgentStatus,
}

/// Join args for error display, redacting `--env KEY=VALUE` values (secrets).
pub fn display_args(args: &[String]) -> String {
    let mut out: Vec<String> = Vec::with_capacity(args.len());
    let mut prev_was_env = false;
    for a in args {
        if prev_was_env {
            out.push(match a.split_once('=') {
                Some((k, _)) => format!("{k}=***"),
                None => "***".to_string(),
            });
        } else {
            out.push(a.clone());
        }
        prev_was_env = a == "--env";
    }
    out.join(" ")
}

/// Unwrap the one-line JSON envelope: `{"result": {...}}` or `{"error": {...}}`.
pub fn parse_envelope(stdout: &str) -> Result<serde_json::Value, HerdrError> {
    let v: serde_json::Value = serde_json::from_str(stdout.trim())
        .map_err(|e| HerdrError::Parse(format!("{e}: {}", truncate(stdout))))?;
    if let Some(err) = v.get("error") {
        let msg = err["message"].as_str().unwrap_or("unknown herdr error");
        return Err(HerdrError::Failed {
            args: v["id"].as_str().unwrap_or("?").to_string(),
            stderr: msg.to_string(),
        });
    }
    v.get("result")
        .cloned()
        .ok_or_else(|| HerdrError::Parse(format!("no result field in {}", truncate(stdout))))
}

fn truncate(s: &str) -> String {
    let s = s.trim();
    if s.len() > 200 { format!("{}…", &s[..200]) } else { s.to_string() }
}

fn agent_from_value(a: &serde_json::Value) -> Option<AgentInfo> {
    Some(AgentInfo {
        // Started agents carry "name"; integration-detected panes carry "agent"
        // instead and are not ours — the caller filters on name presence.
        name: a.get("name")?.as_str()?.to_string(),
        pane_id: a.get("pane_id")?.as_str()?.to_string(),
        workspace_id: a.get("workspace_id")?.as_str()?.to_string(),
        status: AgentStatus::parse(a.get("agent_status").and_then(|s| s.as_str()).unwrap_or("")),
    })
}

/// Started (named) agents from an `agent_list` result. Detected agents
/// (no "name" field) are skipped — they are not cortado sessions.
pub fn parse_agent_list(result: &serde_json::Value) -> Result<Vec<AgentInfo>, HerdrError> {
    let arr = result["agents"]
        .as_array()
        .ok_or_else(|| HerdrError::Parse("agent_list without agents array".into()))?;
    Ok(arr.iter().filter_map(agent_from_value).collect())
}

/// The agent object from an `agent_started` (or `agent_info`) result.
pub fn parse_started_agent(result: &serde_json::Value) -> Result<AgentInfo, HerdrError> {
    agent_from_value(&result["agent"])
        .ok_or_else(|| HerdrError::Parse("agent result missing name/pane_id".into()))
}

/// `(workspace_id, label)` pairs from a `workspace_list` result.
pub fn parse_workspace_list(result: &serde_json::Value) -> Result<Vec<(String, String)>, HerdrError> {
    let arr = result["workspaces"]
        .as_array()
        .ok_or_else(|| HerdrError::Parse("workspace_list without workspaces array".into()))?;
    Ok(arr
        .iter()
        .filter_map(|w| {
            Some((
                w.get("workspace_id")?.as_str()?.to_string(),
                w.get("label")?.as_str()?.to_string(),
            ))
        })
        .collect())
}

/// The new workspace's id from a `workspace_created` result.
pub fn parse_workspace_created(result: &serde_json::Value) -> Result<String, HerdrError> {
    result["workspace"]["workspace_id"]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| HerdrError::Parse("workspace_created without workspace_id".into()))
}

/// `herdr status server` is plain text; running iff a `status: running` line exists.
pub fn parse_status_running(text: &str) -> bool {
    text.lines().any(|l| l.trim() == "status: running")
}

/// `herdr --version` → `herdr 0.7.4` → (0, 7).
pub fn parse_herdr_version(text: &str) -> Option<(u32, u32)> {
    let ver = text.trim().strip_prefix("herdr ")?;
    let mut it = ver.split('.');
    Some((it.next()?.parse().ok()?, it.next()?.parse().ok()?))
}

pub struct Herdr {
    #[allow(dead_code)]
    binary: String,
    workspace_label: String,
    /// Named herdr session (isolated socket) — test seam; None = default session.
    session: Option<String>,
}

impl Herdr {
    pub fn new(binary: String, workspace_label: String, session: Option<String>) -> Herdr {
        Herdr { binary, workspace_label, session }
    }

    pub fn workspace_label(&self) -> &str {
        &self.workspace_label
    }

    /// `--session <name>` goes AFTER the subcommand words for CLI calls.
    fn with_session(&self, mut args: Vec<String>) -> Vec<String> {
        if let Some(s) = &self.session {
            args.push("--session".into());
            args.push(s.clone());
        }
        args
    }

    pub fn list_args(&self) -> Vec<String> {
        self.with_session(vec!["agent".into(), "list".into()])
    }

    pub fn focus_args(&self, name: &str) -> Vec<String> {
        self.with_session(vec!["agent".into(), "focus".into(), name.into()])
    }

    pub fn close_args(&self, pane_id: &str) -> Vec<String> {
        self.with_session(vec!["pane".into(), "close".into(), pane_id.into()])
    }

    pub fn send_args(&self, name: &str, text: &str) -> Vec<String> {
        self.with_session(vec!["agent".into(), "send".into(), name.into(), text.into()])
    }

    pub fn read_args(&self, name: &str, lines: u32) -> Vec<String> {
        self.with_session(vec![
            "agent".into(), "read".into(), name.into(),
            "--lines".into(), lines.to_string(),
        ])
    }

    pub fn wait_status_args(&self, name: &str, status: &str, timeout_ms: u64) -> Vec<String> {
        self.with_session(vec![
            "agent".into(), "wait".into(), name.into(),
            "--status".into(), status.into(),
            "--timeout".into(), timeout_ms.to_string(),
        ])
    }

    pub fn workspace_list_args(&self) -> Vec<String> {
        self.with_session(vec!["workspace".into(), "list".into()])
    }

    pub fn workspace_create_args(&self) -> Vec<String> {
        self.with_session(vec![
            "workspace".into(), "create".into(),
            "--label".into(), self.workspace_label.clone(),
        ])
    }

    pub fn status_args(&self) -> Vec<String> {
        self.with_session(vec!["status".into(), "server".into()])
    }

    /// Launch form: `herdr [--session X] server` (flag BEFORE `server`).
    pub fn server_launch_args(&self) -> Vec<String> {
        let mut args = Vec::new();
        if let Some(s) = &self.session {
            args.push("--session".into());
            args.push(s.clone());
        }
        args.push("server".into());
        args
    }

    pub fn start_args(
        &self,
        name: &str,
        cwd: &Path,
        env: &BTreeMap<String, String>,
        command: &[String],
        workspace_id: &str,
        focus: bool,
    ) -> Vec<String> {
        let mut args: Vec<String> = vec![
            "agent".into(), "start".into(), name.into(),
            "--cwd".into(), cwd.to_string_lossy().into_owned(),
            "--workspace".into(), workspace_id.into(),
            "--split".into(), "right".into(),
        ];
        args.push(if focus { "--focus".into() } else { "--no-focus".into() });
        for (k, v) in env {
            args.push("--env".into());
            args.push(format!("{k}={v}"));
        }
        // --session must precede `--` (everything after -- is the agent argv).
        let mut args = self.with_session(args);
        args.push("--".into());
        args.extend(command.iter().cloned());
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STARTED: &str = r#"{"id":"cli:agent:start","result":{"agent":{"agent_status":"unknown","cwd":"/private/tmp","focused":false,"name":"cortado_demo_scout_1","pane_id":"w1:p2","tab_id":"w1:t1","terminal_id":"term_656e5f61e4bae2","workspace_id":"w1","revision":0,"foreground_cwd":"/private/tmp"},"argv":["sh","-c","sleep 120"],"type":"agent_started"}}"#;
    const LIST_MIXED: &str = r#"{"id":"cli:agent:list","result":{"agents":[{"agent_status":"unknown","cwd":"/private/tmp","focused":false,"name":"cortado_demo_scout_1","pane_id":"w1:p2","tab_id":"w1:t1","terminal_id":"t1","workspace_id":"w1","revision":0,"foreground_cwd":"/private/tmp"},{"agent":"claude","agent_status":"working","cwd":"/Users/x","focused":true,"pane_id":"w1:p1","tab_id":"w1:t1","terminal_id":"t2","workspace_id":"w1","revision":4,"foreground_cwd":"/Users/x"}],"type":"agent_list"}}"#;
    const WS_LIST: &str = r#"{"id":"cli:workspace:list","result":{"type":"workspace_list","workspaces":[{"workspace_id":"w1","label":"~","number":1,"focused":true,"active_tab_id":"w1:t1","agent_status":"working","pane_count":1,"tab_count":1}]}}"#;
    const WS_CREATED: &str = r#"{"id":"cli:workspace:create","result":{"type":"workspace_created","workspace":{"workspace_id":"w2","label":"Cortado","number":2,"focused":true,"active_tab_id":"w2:t1","agent_status":"unknown","pane_count":1,"tab_count":1}}}"#;
    const ERR: &str = r#"{"error":{"code":"agent_not_found","message":"agent target nope not found"},"id":"cli:agent:get"}"#;

    fn h() -> Herdr {
        Herdr::new("herdr".into(), "Cortado".into(), None)
    }
    fn h_sess() -> Herdr {
        Herdr::new("herdr".into(), "Cortado".into(), Some("t1".into()))
    }

    #[test]
    fn envelope_unwraps_result() {
        let v = parse_envelope(STARTED).unwrap();
        assert_eq!(v["type"], "agent_started");
    }

    #[test]
    fn envelope_surfaces_herdr_errors() {
        match parse_envelope(ERR) {
            Err(HerdrError::Failed { stderr, .. }) => {
                assert!(stderr.contains("agent target nope not found"))
            }
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn envelope_rejects_garbage() {
        assert!(matches!(parse_envelope("not json"), Err(HerdrError::Parse(_))));
    }

    #[test]
    fn agent_list_keeps_named_agents_only() {
        let v = parse_envelope(LIST_MIXED).unwrap();
        let agents = parse_agent_list(&v).unwrap();
        // The detected "claude" row has no "name" — it is not a cortado-started agent.
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].name, "cortado_demo_scout_1");
        assert_eq!(agents[0].pane_id, "w1:p2");
        assert_eq!(agents[0].workspace_id, "w1");
        assert_eq!(agents[0].status, AgentStatus::Unknown);
    }

    #[test]
    fn started_agent_parses() {
        let v = parse_envelope(STARTED).unwrap();
        let a = parse_started_agent(&v).unwrap();
        assert_eq!(a.name, "cortado_demo_scout_1");
        assert_eq!(a.pane_id, "w1:p2");
    }

    #[test]
    fn workspace_list_finds_label() {
        let v = parse_envelope(WS_LIST).unwrap();
        let all = parse_workspace_list(&v).unwrap();
        assert_eq!(all, vec![("w1".to_string(), "~".to_string())]);
    }

    #[test]
    fn workspace_created_yields_id() {
        let v = parse_envelope(WS_CREATED).unwrap();
        assert_eq!(parse_workspace_created(&v).unwrap(), "w2");
    }

    #[test]
    fn status_parses() {
        assert_eq!(AgentStatus::parse("idle"), AgentStatus::Idle);
        assert_eq!(AgentStatus::parse("working"), AgentStatus::Working);
        assert_eq!(AgentStatus::parse("blocked"), AgentStatus::Blocked);
        assert_eq!(AgentStatus::parse("anything-else"), AgentStatus::Unknown);
        assert_eq!(AgentStatus::Working.to_string(), "working");
    }

    #[test]
    fn server_status_text_parses() {
        assert!(parse_status_running("status: running\nversion: 0.7.4\n"));
        assert!(!parse_status_running("status: stopped\n"));
        assert_eq!(parse_herdr_version("herdr 0.7.4"), Some((0, 7)));
        assert_eq!(parse_herdr_version("nonsense"), None);
    }

    #[test]
    fn start_args_shape() {
        let mut env = std::collections::BTreeMap::new();
        env.insert("CORTADO_TEAM".to_string(), "demo".to_string());
        let args = h().start_args(
            "cortado_demo_scout_1",
            std::path::Path::new("/tmp/wt"),
            &env,
            &["claude".to_string()],
            "w2",
            true,
        );
        assert_eq!(
            args,
            vec![
                "agent", "start", "cortado_demo_scout_1", "--cwd", "/tmp/wt",
                "--workspace", "w2", "--split", "right", "--focus",
                "--env", "CORTADO_TEAM=demo", "--", "claude"
            ]
        );
    }

    #[test]
    fn session_flag_appends_to_cli_args_only() {
        assert_eq!(h().list_args(), vec!["agent", "list"]);
        assert_eq!(h_sess().list_args(), vec!["agent", "list", "--session", "t1"]);
        // Launch form puts --session BEFORE `server` (verified herdr behavior).
        assert_eq!(h_sess().server_launch_args(), vec!["--session", "t1", "server"]);
        assert_eq!(h().server_launch_args(), vec!["server"]);
    }

    #[test]
    fn start_args_session_precedes_double_dash() {
        let env = std::collections::BTreeMap::new();
        let args = h_sess().start_args(
            "n", std::path::Path::new("/w"), &env, &["sh".to_string()], "w1", false,
        );
        let dd = args.iter().position(|a| a == "--").unwrap();
        let sess = args.iter().position(|a| a == "--session").unwrap();
        assert!(sess < dd);
        assert!(args.contains(&"--no-focus".to_string()));
    }

    #[test]
    fn env_values_redacted_in_display() {
        let shown = display_args(&[
            "agent".into(), "start".into(), "x".into(),
            "--env".into(), "OPENROUTER_API_KEY=sk-secret".into(),
        ]);
        assert!(shown.contains("OPENROUTER_API_KEY=***"));
        assert!(!shown.contains("sk-secret"));
    }
}
