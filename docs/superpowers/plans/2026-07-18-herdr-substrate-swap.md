# M6 — Herdr Substrate Swap Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace cortado's tmux + Ghostty + workspace session layer with Herdr, keeping the data model and orchestration logic intact — ephemeral agent processes over persistent markdown memory.

**Architecture:** A new `cortado-herdr` crate wraps the `herdr` CLI (JSON output on every subcommand) exactly the way `cortado-tmux` wrapped tmux. Commands (`open`, `sessions`, `kill`, `doctor`, `ui`) are rewired to it; `cortado-tmux` and `cortado-term` are then deleted. Per-session model display moves from tmux user options to a `sessions.jsonl` join. The TUI keeps its rail and drops all pane/layout management.

**Tech Stack:** Rust 2021 workspace, `serde_json` (already a workspace dep), `thiserror`, `ratatui`. External: `herdr` ≥ 0.7.4 (Homebrew).

**Spec:** `docs/superpowers/specs/2026-07-18-herdr-substrate-design.md` — read it before starting.

## Global Constraints

- Herdr minimum version: **0.7.4** (verified against `herdr 0.7.4`, protocol 16).
- Transport is the `herdr` CLI only — never the socket directly, never a long-lived connection.
- `cortado-herdr` holds **no state and caches nothing**; every question re-asks Herdr.
- Config: new optional `[herdr]` section (`binary = "herdr"`, `workspace = "Cortado"`); `[tmux]`/`[terminal]` retire with a one-time warning, never an error.
- Test seam env var: `CORTADO_HERDR_SESSION=<name>` targets a named Herdr session (isolated socket) instead of the default one. Production never sets it.
- Secrets (`--env K=V` values) must be redacted in every error/display string, matching `cortado-tmux::display_args`.
- The workspace must build and pass `cargo test` after **every task** — deletion of old crates comes last for exactly this reason.
- `cargo clippy --workspace --all-targets -- -D warnings` clean at every commit (existing project standard).
- All commands below run from the repo root `/Users/donnielane/Documents/DEV/cortado`.
- Current branch: `agent/pane-menu-roster-logo`. The working tree has unrelated uncommitted changes (`Cargo.toml`, `Cargo.lock`, `crates/cortado-term/src/workspace.rs`, `crates/cortado/src/tui/mod.rs`, `crates/cortado/Cargo.toml`) — **`git add` only the files each task names; never `git add -A`.**

## Verified Herdr CLI facts (captured live, 2026-07-18, herdr 0.7.4)

Every `herdr` subcommand prints a one-line JSON envelope on stdout:

- Success: `{"id":"cli:agent:list","result":{...}}`
- Failure: `{"error":{"code":"agent_not_found","message":"agent target definitely_not_real not found"},"id":"cli:agent:get"}` with exit code 1.
- Server unreachable (no socket): non-JSON on stderr — `Error: Os { code: 2, kind: NotFound, message: "No such file or directory" }`, exit 1.

Key result payloads (real captures, trimmed to relevant fields):

```json
// herdr agent start cortado_demo_scout_1 --cwd /tmp --env CORTADO_TEAM=demo -- sh -c 'sleep 120'
{"id":"cli:agent:start","result":{"agent":{"agent_status":"unknown","cwd":"/private/tmp","focused":false,"name":"cortado_demo_scout_1","pane_id":"w1:p2","tab_id":"w1:t1","terminal_id":"term_656e5f61e4bae2","workspace_id":"w1","revision":0,"foreground_cwd":"/private/tmp"},"argv":["sh","-c","sleep 120"],"type":"agent_started"}}

// herdr agent list — a *started* agent has "name"; an integration-*detected* one has "agent" instead
{"id":"cli:agent:list","result":{"agents":[{"agent_status":"unknown","cwd":"/private/tmp","focused":false,"name":"cortado_demo_scout_1","pane_id":"w1:p2","tab_id":"w1:t1","terminal_id":"term_656e5f61e4bae2","workspace_id":"w1","revision":0,"foreground_cwd":"/private/tmp"}],"type":"agent_list"}}

// detected-agent row shape (from the default session; note "agent", no "name"):
{"agent":"claude","agent_status":"working","cwd":"/Users/donnielane","focused":true,"pane_id":"w1:p1","tab_id":"w1:t1","terminal_id":"term_656e4a48b4e171","workspace_id":"w1","terminal_title":"...","terminal_title_stripped":"...","revision":4,"foreground_cwd":"/Users/donnielane"}

// herdr workspace create --label Cortado
{"id":"cli:workspace:create","result":{"type":"workspace_created","workspace":{"workspace_id":"w1","label":"Cortado","number":1,"focused":true,"active_tab_id":"w1:t1","agent_status":"unknown","pane_count":1,"tab_count":1},"tab":{...},"root_pane":{...}}}

// herdr workspace list
{"id":"cli:workspace:list","result":{"type":"workspace_list","workspaces":[{"workspace_id":"w1","label":"~","number":1,"focused":true,"active_tab_id":"w1:t1","agent_status":"working","pane_count":1,"tab_count":1}]}}

// herdr pane close w1:p2
{"id":"cli:pane:close","result":{"type":"ok"}}

// herdr agent focus cortado_demo_scout_1  → {"result":{"agent":{...},"type":"agent_info"}}
```

Non-JSON commands: `herdr --version` → `herdr 0.7.4`; `herdr status server` → lines `status: running` / `version: 0.7.4` / `protocol: 16` / `compatible: yes` / `socket: <path>`.

`agent_status` values: `idle | working | blocked | unknown` (plus `done` accepted by `wait`).

Session isolation (used by integration tests):
- Launch: `herdr --session <name> server` (flag **before** `server`) — runs headless with its own socket at `~/.config/herdr/sessions/<name>/herdr.sock`. Blocks; spawn detached.
- Target: every CLI subcommand accepts `--session <name>` **after** the subcommand words, e.g. `herdr agent list --session cortadotest`.
- Teardown: `herdr session stop <name>` then `herdr session delete <name>`.
- macOS has no `timeout(1)` — poll with a Rust loop, never shell `timeout`.

`herdr agent start` supports: `--cwd PATH --workspace ID --tab ID --split right|down --env KEY=VALUE --focus|--no-focus -- <argv...>`. Focus is NOT automatic (captured `focused:false`); pass `--focus` when the user asked to open.

---

### Task 1: `cortado-herdr` crate — types, argv builders, JSON parsers (pure)

**Files:**
- Create: `crates/cortado-herdr/Cargo.toml`
- Create: `crates/cortado-herdr/src/lib.rs`
- Modify: `Cargo.toml` (workspace members + deps)

**Interfaces:**
- Consumes: nothing from other crates.
- Produces (used by Tasks 2, 5, 6, 7, 8):
  - `pub enum HerdrError { NotInstalled, ServerDown(String), Failed { args: String, stderr: String }, Parse(String) }`
  - `pub enum AgentStatus { Idle, Working, Blocked, Unknown }` with `AgentStatus::parse(&str) -> AgentStatus` and `Display` ("idle" etc.)
  - `pub struct AgentInfo { pub name: String, pub pane_id: String, pub workspace_id: String, pub status: AgentStatus }`
  - `pub struct Herdr` with `Herdr::new(binary: String, workspace_label: String, session: Option<String>) -> Herdr`
  - Pure fns (free, `pub` for tests): `parse_envelope`, `parse_agent_list`, `parse_started_agent`, `parse_workspace_list`, `parse_workspace_created`, `parse_status_running`, `parse_herdr_version`, `display_args`
  - Argv builder methods (pure, no process): `start_args`, `list_args`, `focus_args`, `close_args`, `send_args`, `read_args`, `wait_status_args`, `workspace_list_args`, `workspace_create_args`, `status_args`, `server_launch_args`

- [ ] **Step 1: Scaffold the crate and register it in the workspace**

`crates/cortado-herdr/Cargo.toml`:

```toml
[package]
name = "cortado-herdr"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
thiserror = { workspace = true }
serde_json = { workspace = true }
```

In the root `Cargo.toml`, add `"crates/cortado-herdr"` to `[workspace] members` (keep `cortado-tmux`/`cortado-term` for now — they go in Task 9) and add to `[workspace.dependencies]`:

```toml
cortado-herdr = { path = "crates/cortado-herdr" }
```

- [ ] **Step 2: Write the failing tests**

Create `crates/cortado-herdr/src/lib.rs` containing ONLY the test module first (so the test names drive the API), then run to see compile failures, then fill in. Tests use the real captured JSON from the header of this plan:

```rust
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
```

- [ ] **Step 3: Run tests to verify they fail to compile**

Run: `cargo test -p cortado-herdr`
Expected: compile errors — `Herdr`, `parse_envelope`, etc. not found.

- [ ] **Step 4: Implement the crate**

Fill `crates/cortado-herdr/src/lib.rs` above the test module:

```rust
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
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p cortado-herdr`
Expected: all tests PASS.

- [ ] **Step 6: Clippy + commit**

Run: `cargo clippy -p cortado-herdr --all-targets -- -D warnings`
Expected: clean.

```bash
git add crates/cortado-herdr Cargo.toml Cargo.lock
git commit -m "feat: cortado-herdr crate — argv builders and JSON parsers for the herdr CLI"
```

---

### Task 2: `cortado-herdr` process execution + `ensure_server`/`ensure_workspace` + integration round-trip

**Files:**
- Modify: `crates/cortado-herdr/src/lib.rs`
- Create: `crates/cortado-herdr/tests/live.rs`

**Interfaces:**
- Consumes: Task 1's builders/parsers.
- Produces (used by Tasks 5, 6, 8):
  - `impl Herdr` gains: `ensure_server(&self) -> Result<(), HerdrError>`, `ensure_workspace(&self) -> Result<String, HerdrError>`, `start(&self, name, cwd, env, command, workspace_id, focus) -> Result<AgentInfo, HerdrError>`, `list(&self) -> Result<Vec<AgentInfo>, HerdrError>`, `focus(&self, name) -> Result<(), HerdrError>`, `close(&self, pane_id) -> Result<(), HerdrError>`, `send(&self, name, text) -> Result<(), HerdrError>`, `read(&self, name, lines) -> Result<String, HerdrError>`, `wait_status(&self, name, status, timeout_ms) -> Result<(), HerdrError>`
  - Test helper (in `tests/live.rs`, copy into other crates' tests as needed): `IsoSession` guard struct.

- [ ] **Step 1: Implement `run` and the public methods**

Add to `impl Herdr` (and `use std::process::Command;` at top):

```rust
    fn run(&self, args: &[String]) -> Result<serde_json::Value, HerdrError> {
        let out = Command::new(&self.binary).args(args).output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                HerdrError::NotInstalled
            } else {
                HerdrError::Failed {
                    args: display_args(args),
                    stderr: e.to_string(),
                }
            }
        })?;
        let stdout = String::from_utf8_lossy(&out.stdout).to_string();
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        if !out.status.success() {
            // JSON error envelope on stdout beats raw stderr.
            if let Ok(serde_json::Value::Object(_)) = serde_json::from_str(&stdout) {
                return match parse_envelope(&stdout) {
                    Err(e) => Err(e),
                    Ok(_) => Err(HerdrError::Failed {
                        args: display_args(args),
                        stderr,
                    }),
                };
            }
            // No socket → the server for this session is not running.
            if stderr.contains("No such file or directory") || stderr.contains("Connection refused") {
                return Err(HerdrError::ServerDown(stderr));
            }
            return Err(HerdrError::Failed { args: display_args(args), stderr });
        }
        parse_envelope(&stdout)
    }

    /// Plain-text commands (`status server`); success text returned as-is.
    fn run_text(&self, args: &[String]) -> Result<String, HerdrError> {
        let out = Command::new(&self.binary).args(args).output().map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                HerdrError::NotInstalled
            } else {
                HerdrError::Failed { args: display_args(args), stderr: e.to_string() }
            }
        })?;
        if !out.status.success() {
            return Err(HerdrError::ServerDown(
                String::from_utf8_lossy(&out.stderr).trim().to_string(),
            ));
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    /// Idempotent: running server → Ok. Down → spawn `herdr server` detached
    /// and poll until it answers (max ~5s). Mirrors tmux's implicit server start.
    pub fn ensure_server(&self) -> Result<(), HerdrError> {
        if matches!(self.run_text(&self.status_args()), Ok(t) if parse_status_running(&t)) {
            return Ok(());
        }
        Command::new(&self.binary)
            .args(self.server_launch_args())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    HerdrError::NotInstalled
                } else {
                    HerdrError::ServerDown(format!("could not launch herdr server: {e}"))
                }
            })?;
        for _ in 0..50 {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if matches!(self.run_text(&self.status_args()), Ok(t) if parse_status_running(&t)) {
                return Ok(());
            }
        }
        Err(HerdrError::ServerDown(
            "herdr server did not come up within 5s".into(),
        ))
    }

    /// Find the workspace with our label, or create it. Returns workspace_id.
    pub fn ensure_workspace(&self) -> Result<String, HerdrError> {
        let list = parse_workspace_list(&self.run(&self.workspace_list_args())?)?;
        if let Some((id, _)) = list.into_iter().find(|(_, l)| l == &self.workspace_label) {
            return Ok(id);
        }
        parse_workspace_created(&self.run(&self.workspace_create_args())?)
    }

    pub fn start(
        &self,
        name: &str,
        cwd: &Path,
        env: &BTreeMap<String, String>,
        command: &[String],
        workspace_id: &str,
        focus: bool,
    ) -> Result<AgentInfo, HerdrError> {
        parse_started_agent(&self.run(&self.start_args(name, cwd, env, command, workspace_id, focus))?)
    }

    pub fn list(&self) -> Result<Vec<AgentInfo>, HerdrError> {
        parse_agent_list(&self.run(&self.list_args())?)
    }

    pub fn focus(&self, name: &str) -> Result<(), HerdrError> {
        self.run(&self.focus_args(name)).map(|_| ())
    }

    pub fn close(&self, pane_id: &str) -> Result<(), HerdrError> {
        self.run(&self.close_args(pane_id)).map(|_| ())
    }

    pub fn send(&self, name: &str, text: &str) -> Result<(), HerdrError> {
        self.run(&self.send_args(name, text)).map(|_| ())
    }

    pub fn read(&self, name: &str, lines: u32) -> Result<String, HerdrError> {
        let result = self.run(&self.read_args(name, lines))?;
        // agent read returns text content; take any string field named "text"
        // or fall back to the raw JSON for the caller to interpret.
        Ok(result
            .get("text")
            .and_then(|t| t.as_str())
            .map(str::to_string)
            .unwrap_or_else(|| result.to_string()))
    }

    pub fn wait_status(&self, name: &str, status: &str, timeout_ms: u64) -> Result<(), HerdrError> {
        self.run(&self.wait_status_args(name, status, timeout_ms)).map(|_| ())
    }
```

Note on `read`: the exact result field name for `agent read` was not captured live. The integration test in Step 2 asserts on it — if the field is not `text`, adjust `read` to the real field and update this comment. Do NOT guess silently: print the raw JSON in the test failure.

- [ ] **Step 2: Write the gated integration test**

`crates/cortado-herdr/tests/live.rs` — skips (passes trivially) when `herdr` is absent, mirroring the real-tmux test pattern:

```rust
//! Round-trip against a real, isolated herdr server (named session).
//! Skipped when herdr is not installed.
use cortado_herdr::{AgentStatus, Herdr};
use std::process::Command;

fn herdr_available() -> bool {
    Command::new("herdr").arg("--version").output().map(|o| o.status.success()).unwrap_or(false)
}

/// Starts `herdr --session <name> server` detached; stops + deletes on drop.
struct IsoSession {
    name: String,
}

impl IsoSession {
    fn start() -> IsoSession {
        let name = format!("cortadotest{}", std::process::id());
        let h = Herdr::new("herdr".into(), "Cortado".into(), Some(name.clone()));
        h.ensure_server().expect("isolated herdr server should start");
        IsoSession { name }
    }
    fn herdr(&self) -> Herdr {
        Herdr::new("herdr".into(), "Cortado".into(), Some(self.name.clone()))
    }
}

impl Drop for IsoSession {
    fn drop(&mut self) {
        Command::new("herdr").args(["session", "stop", &self.name]).output().ok();
        Command::new("herdr").args(["session", "delete", &self.name]).output().ok();
    }
}

#[test]
fn full_agent_round_trip() {
    if !herdr_available() {
        eprintln!("skipping: herdr not installed");
        return;
    }
    let iso = IsoSession::start();
    let h = iso.herdr();

    // ensure_workspace is idempotent
    let ws = h.ensure_workspace().unwrap();
    assert_eq!(h.ensure_workspace().unwrap(), ws);

    // start a dummy agent
    let mut env = std::collections::BTreeMap::new();
    env.insert("CORTADO_TEAM".to_string(), "demo".to_string());
    let started = h
        .start(
            "cortado_demo_scout_1",
            std::path::Path::new("/tmp"),
            &env,
            &["sh".to_string(), "-c".to_string(), "sleep 120".to_string()],
            &ws,
            false,
        )
        .unwrap();
    assert_eq!(started.name, "cortado_demo_scout_1");

    // list sees it
    let live = h.list().unwrap();
    assert_eq!(live.len(), 1);
    assert_eq!(live[0].name, "cortado_demo_scout_1");
    assert!(matches!(
        live[0].status,
        AgentStatus::Unknown | AgentStatus::Idle | AgentStatus::Working
    ));

    // focus works
    h.focus("cortado_demo_scout_1").unwrap();

    // send + read plumbing (M7 consumers; verify they do not error)
    h.send("cortado_demo_scout_1", "echo hi").unwrap();
    let text = h.read("cortado_demo_scout_1", 50).unwrap();
    assert!(!text.is_empty(), "agent read returned empty; raw output shape changed?");

    // close removes it
    h.close(&live[0].pane_id).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(500));
    assert!(h.list().unwrap().is_empty());
}

#[test]
fn server_down_is_typed() {
    if !herdr_available() {
        eprintln!("skipping: herdr not installed");
        return;
    }
    let h = Herdr::new("herdr".into(), "Cortado".into(), Some("neverstarted999".into()));
    match h.list() {
        Err(cortado_herdr::HerdrError::ServerDown(_)) => {}
        other => panic!("expected ServerDown, got {other:?}"),
    }
    // Clean up the session dir herdr may have scaffolded for the name probe.
    Command::new("herdr").args(["session", "delete", "neverstarted999"]).output().ok();
}

#[test]
fn missing_binary_is_not_installed() {
    let h = Herdr::new("definitely-not-herdr-xyz".into(), "Cortado".into(), None);
    match h.list() {
        Err(cortado_herdr::HerdrError::NotInstalled) => {}
        other => panic!("expected NotInstalled, got {other:?}"),
    }
}
```

- [ ] **Step 3: Run the integration tests**

Run: `cargo test -p cortado-herdr --test live -- --test-threads=1`
Expected: 3 tests PASS (herdr is installed on this machine). If `full_agent_round_trip` fails on the `read` assertion, print the raw JSON, fix the field name in `Herdr::read`, re-run.

- [ ] **Step 4: Clippy + commit**

Run: `cargo clippy -p cortado-herdr --all-targets -- -D warnings` then `cargo test -p cortado-herdr`

```bash
git add crates/cortado-herdr
git commit -m "feat: cortado-herdr live execution — ensure_server/workspace, start/list/focus/close/send/read/wait"
```

---

### Task 3: Config — add `[herdr]`, retire `[tmux]`/`[terminal]` with a one-time warning

**Files:**
- Modify: `crates/cortado-core/src/config.rs`
- Modify: `crates/cortado/src/commands/init.rs` (only if it mentions tmux/terminal config text — check with `grep -n "tmux\|terminal" crates/cortado/src/commands/init.rs`)

**Interfaces:**
- Consumes: nothing new.
- Produces (used by Tasks 5–8):
  - `Config { general: General, herdr: HerdrCfg }` — `tmux`/`terminal` fields and the `TmuxCfg`/`TerminalCfg`/`Launcher`/`Window` types are **removed**.
  - `pub struct HerdrCfg { pub binary: String, pub workspace: String }` (defaults `"herdr"` / `"Cortado"`).
  - `Config::load() -> Result<Config, CoreError>` unchanged signature.
  - `pub fn obsolete_sections(path: &Path) -> Vec<&'static str>` — names of retired sections present in the user's config file (empty when file missing/clean).
  - `Config::herdr_session() -> Option<String>` — reads `CORTADO_HERDR_SESSION` env (test seam).

- [ ] **Step 1: Write the failing tests**

Replace/extend the `#[cfg(test)]` block in `config.rs` — update `defaults_are_spec_values`, delete `workspace_launcher_and_window_parse`, add:

```rust
    #[test]
    fn defaults_are_spec_values() {
        let c = Config::default();
        assert_eq!(c.general.root, "~/cortado");
        assert_eq!(c.general.default_runtime, "claude");
        assert_eq!(c.herdr.binary, "herdr");
        assert_eq!(c.herdr.workspace, "Cortado");
    }

    #[test]
    fn herdr_section_parses_and_fills_defaults() {
        let c: Config = toml::from_str("[herdr]\nbinary = \"/opt/herdr\"\n").unwrap();
        assert_eq!(c.herdr.binary, "/opt/herdr");
        assert_eq!(c.herdr.workspace, "Cortado");
    }

    #[test]
    fn old_tmux_terminal_sections_still_parse_but_are_reported() {
        // Old config files must keep loading (serde ignores unknown sections)…
        let text = "[tmux]\nsocket = \"cortado\"\n[terminal]\nlauncher = \"workspace\"\n";
        let c: Config = toml::from_str(text).unwrap();
        assert_eq!(c, Config::default());
        // …and obsolete_sections names what should be deleted.
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("config.toml");
        std::fs::write(&path, text).unwrap();
        assert_eq!(obsolete_sections(&path), vec!["tmux", "terminal"]);
        assert!(obsolete_sections(&tmp.path().join("missing.toml")).is_empty());
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p cortado-core config`
Expected: FAIL — `herdr` field, `obsolete_sections` missing. (Compile errors in the `cortado` / `cortado-term` crates are expected — they are fixed in Tasks 5–9; for THIS task run only `cargo test -p cortado-core`.)

- [ ] **Step 3: Implement**

In `config.rs`: delete `TmuxCfg`, `TerminalCfg`, `Launcher`, `Window` and the `tmux`/`terminal` fields; add:

```rust
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct HerdrCfg {
    /// Path or name of the herdr binary.
    pub binary: String,
    /// Label of the Herdr workspace cortado owns.
    pub workspace: String,
}

impl Default for HerdrCfg {
    fn default() -> Self {
        HerdrCfg { binary: "herdr".into(), workspace: "Cortado".into() }
    }
}

/// Retired config sections present in the file at `path` (best-effort:
/// unreadable/invalid files report nothing — load() owns real errors).
pub fn obsolete_sections(path: &Path) -> Vec<&'static str> {
    let Ok(text) = std::fs::read_to_string(path) else { return Vec::new() };
    let Ok(v) = text.parse::<toml::Table>() else { return Vec::new() };
    ["tmux", "terminal"].into_iter().filter(|s| v.contains_key(*s)).collect()
}
```

and in `Config`: `pub herdr: HerdrCfg`, plus:

```rust
    /// Test seam: target a named herdr session (isolated socket).
    pub fn herdr_session() -> Option<String> {
        std::env::var("CORTADO_HERDR_SESSION").ok().filter(|s| !s.is_empty())
    }
```

- [ ] **Step 4: Verify core passes**

Run: `cargo test -p cortado-core`
Expected: PASS (workspace-wide build still broken until Tasks 5–9 — that is expected; do not fix other crates here).

- [ ] **Step 5: Commit**

```bash
git add crates/cortado-core/src/config.rs
git commit -m "feat: config gains [herdr], retires [tmux]/[terminal] with detection helper"
```

---

### Task 4: Model provenance — `sessions.jsonl` join replaces `@cortado_model`

tmux stored each session's model in a `@cortado_model` user option; Herdr has no equivalent. The append-only spawn log already records `{event: "spawn", session, model, runtime}` per session — derive model from it (stateless: the log on disk is the truth).

**Files:**
- Create: `crates/cortado-core/src/session_log.rs`
- Modify: `crates/cortado-core/src/lib.rs` (add `pub mod session_log;`)

**Interfaces:**
- Consumes: `Store::logs_dir`, `SessionName::parse`.
- Produces (used by Tasks 6, 8):
  - `pub fn models_for(store: &Store, names: &[String]) -> BTreeMap<String, String>` — for each cortado session name, the model of its **last** `spawn` event; names with no log entry are absent from the map.

- [ ] **Step 1: Write the failing test**

`crates/cortado-core/src/session_log.rs`:

```rust
//! Read-side of logs/sessions.jsonl: derive per-session facts from the
//! append-only spawn log (the on-disk truth — no live store to drift).
use crate::session_name::SessionName;
use crate::store::Store;
use std::collections::BTreeMap;

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
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p cortado-core session_log`
Expected: FAIL — `models_for` not defined.

- [ ] **Step 3: Implement**

Above the test module:

```rust
/// Model per live session name: the last `spawn` event for that name in its
/// agent's sessions.jsonl. Corrupt lines are skipped; missing logs yield
/// nothing (the TUI renders "?"). One file read per referenced agent.
pub fn models_for(store: &Store, names: &[String]) -> BTreeMap<String, String> {
    let mut wanted: BTreeMap<std::path::PathBuf, Vec<&str>> = BTreeMap::new();
    for name in names {
        if let Some(parsed) = SessionName::parse(name) {
            wanted
                .entry(store.logs_dir(&parsed.team, &parsed.agent).join("sessions.jsonl"))
                .or_default()
                .push(name);
        }
    }
    let mut out = BTreeMap::new();
    for (path, session_names) in wanted {
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        for line in text.lines() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else { continue };
            if v["event"] != "spawn" {
                continue;
            }
            let (Some(session), Some(model)) = (v["session"].as_str(), v["model"].as_str()) else {
                continue;
            };
            if session_names.iter().any(|n| *n == session) {
                out.insert(session.to_string(), model.to_string()); // later lines overwrite
            }
        }
    }
    out
}
```

Register in `crates/cortado-core/src/lib.rs`: `pub mod session_log;`

- [ ] **Step 4: Run to verify pass, then commit**

Run: `cargo test -p cortado-core` — PASS.

```bash
git add crates/cortado-core/src/session_log.rs crates/cortado-core/src/lib.rs
git commit -m "feat: derive per-session model from sessions.jsonl (replaces tmux @cortado_model)"
```

---

### Task 5: Rewire `cortado open` to Herdr

**Files:**
- Modify: `crates/cortado/src/commands/open.rs`
- Modify: `crates/cortado/Cargo.toml` (add `cortado-herdr = { workspace = true }`)

**Interfaces:**
- Consumes: `Herdr` (Tasks 1–2), `Config.herdr` + `Config::herdr_session()` (Task 3).
- Produces: `open_session(reference, model_key, fresh) -> anyhow::Result<OpenOutcome>` — same signature as today (TUI calls it). Also `pub fn warn_obsolete_config()` (used by main/TUI entry).

- [ ] **Step 1: Rewrite `open_session`**

Keep everything except the tmux/launcher parts. New body (unchanged helpers `resolve_model_env`, `find_executable`, `runtime_display`, `append_log`, `SessionEvent` stay as-is; delete the `use cortado_tmux::Tmux;` import and add `use cortado_herdr::Herdr;`):

```rust
pub fn open_session(
    reference: &str,
    model_key: Option<&str>,
    fresh: bool,
) -> anyhow::Result<OpenOutcome> {
    let mut warnings: Vec<String> = Vec::new();
    let config = Config::load()?;
    let store = Store::new(config.root()?);
    let entry = store.resolve(reference)?;
    let agent = entry
        .agent
        .as_ref()
        .map_err(|e| anyhow::anyhow!("agent.toml invalid: {e}"))?;

    let models = ModelsFile::load()?;
    let key = model_key.unwrap_or(&agent.agent.default_model);
    let model = models
        .get(key)
        .ok_or_else(|| anyhow::anyhow!("unknown model {key:?} (see models.toml)"))?;
    let runtime = match model.runtime.as_str() {
        "claude" => RuntimeId::Claude,
        "codex" => RuntimeId::Codex,
        "opencode" => RuntimeId::Opencode,
        other => anyhow::bail!("unknown runtime {other:?} in models.toml"),
    };

    let herdr = Herdr::new(
        config.herdr.binary.clone(),
        config.herdr.workspace.clone(),
        Config::herdr_session(),
    );
    herdr.ensure_server()?;
    let live = herdr.list()?;
    let mine: Vec<&cortado_herdr::AgentInfo> = live
        .iter()
        .filter(|a| {
            SessionName::parse(&a.name).is_some_and(|n| n.team == entry.team && n.agent == entry.slug)
        })
        .collect();

    let session = if !fresh && !mine.is_empty() {
        // most recent = highest n (numeric, not lexical)
        let best = mine
            .iter()
            .max_by_key(|a| SessionName::parse(&a.name).map(|n| n.n).unwrap_or(0))
            .unwrap();
        herdr.focus(&best.name)?;
        best.name.clone()
    } else {
        // Regenerate bridges from canonical memory before every spawn.
        let worktree = store.worktree_dir(&entry.team, &entry.slug);
        let memory = store.memory_dir(&entry.team, &entry.slug);
        let cortado_exe = std::env::current_exe()?.display().to_string();
        let bridges = write_bridges(runtime, &agent.agent.name, &memory, &worktree, &cortado_exe)?;
        warnings.extend(bridges.warnings.iter().cloned());
        let names: Vec<String> = bridges
            .written
            .iter()
            .filter_map(|p| p.strip_prefix(&worktree).ok())
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        cortado_git::exclude(
            &worktree,
            &names.iter().map(String::as_str).collect::<Vec<_>>(),
        )?;

        let live_names: Vec<String> = live.iter().map(|a| a.name.clone()).collect();
        let name = SessionName::next_free(&entry.team, &entry.slug, &live_names).encode();
        let mut env: BTreeMap<String, String> = model
            .env
            .iter()
            .map(|(k, v)| resolve_model_env(key, k, v))
            .collect::<anyhow::Result<_>>()?;
        env.insert("CORTADO_TEAM".into(), entry.team.to_string());
        env.insert("CORTADO_AGENT".into(), entry.slug.to_string());
        env.insert("CORTADO_MODEL".into(), key.to_string());
        env.insert("CORTADO_SESSION".into(), name.clone());

        let binary = runtime.binary();
        if find_executable(&binary).is_none() {
            let install = match runtime {
                RuntimeId::Opencode if cfg!(target_os = "macos") => {
                    "install it with `brew install anomalyco/tap/opencode`"
                }
                RuntimeId::Opencode => "install OpenCode from https://opencode.ai/docs",
                RuntimeId::Codex => "install Codex, or ensure `codex` is on PATH",
                RuntimeId::Claude => "install Claude Code, or ensure `claude` is on PATH",
            };
            anyhow::bail!(
                "{} runtime executable {binary:?} was not found; {install}, or set CORTADO_BIN_{}",
                runtime_display(runtime),
                runtime.as_str().to_uppercase()
            );
        }
        let mut command = vec![binary];
        // Test seam: extra args for substitute binaries (e.g. sleep 30).
        let args_var = format!("CORTADO_{}_ARGS", runtime.as_str().to_uppercase());
        if let Ok(extra) = std::env::var(&args_var) {
            command.extend(extra.split_whitespace().map(String::from));
        }

        let workspace_id = herdr.ensure_workspace()?;
        let started = match herdr.start(&name, &worktree, &env, &command, &workspace_id, true) {
            Ok(info) => info,
            Err(e) => {
                // Best-effort cleanup of any half-created pane.
                if let Ok(after) = herdr.list() {
                    if let Some(a) = after.iter().find(|a| a.name == name) {
                        herdr.close(&a.pane_id).ok();
                    }
                }
                return Err(e.into());
            }
        };
        debug_assert_eq!(started.name, name);

        let event = SessionEvent {
            ts: chrono::Utc::now(),
            event: "spawn",
            session: &name,
            model: key,
            runtime: runtime.as_str(),
        };
        if let Err(log_err) = append_log(&store, &entry.team, &entry.slug, &event) {
            warnings.push(format!(
                "session created but log append failed: {log_err:#}"
            ));
        }
        name
    };

    Ok(OpenOutcome { session, warnings })
}
```

Also update `run()` (top of file) to print the obsolete-config warning once:

```rust
pub fn run(reference: &str, model_key: Option<&str>, new: bool) -> anyhow::Result<()> {
    warn_obsolete_config();
    let out = open_session(reference, model_key, new)?;
    for w in &out.warnings {
        eprintln!("warning: {w}");
    }
    println!("session {}", out.session);
    Ok(())
}

/// One-line nudge when the config file still has retired sections.
pub fn warn_obsolete_config() {
    if let Ok(dir) = Config::config_dir() {
        let stale = cortado_core::config::obsolete_sections(&dir.join("config.toml"));
        if !stale.is_empty() {
            eprintln!(
                "note: config sections [{}] are obsolete since the Herdr substrate swap — remove them from config.toml",
                stale.join("], [")
            );
        }
    }
}
```

Note: `find_executable` stays even though tmux is gone — Herdr also reports a successful start before a missing child argv[0] exits, so pre-validating the runtime binary is still load-bearing.

- [ ] **Step 2: Build this crate only**

Run: `cargo check -p cortado 2>&1 | head -40`
Expected: `open.rs` compiles; remaining errors point ONLY at `sessions.rs`, `doctor.rs`, `tui/*` (fixed in the next tasks). If open.rs errors remain, fix them now.

- [ ] **Step 3: Commit**

```bash
git add crates/cortado/src/commands/open.rs crates/cortado/Cargo.toml
git commit -m "feat: cortado open spawns/focuses Herdr agent panes"
```

---

### Task 6: Rewire `cortado sessions` + `cortado kill`

**Files:**
- Modify: `crates/cortado/src/commands/sessions.rs`

**Interfaces:**
- Consumes: `Herdr::{list, close}`, `session_log::models_for`.
- Produces: `ls()`, `kill(target)`, `kill_agent(reference) -> KillOutcome` — same signatures (TUI uses `kill_agent`).

- [ ] **Step 1: Rewrite the file**

```rust
use cortado_core::config::Config;
use cortado_core::session_log::models_for;
use cortado_core::session_name::SessionName;
use cortado_core::store::Store;
use cortado_herdr::Herdr;

fn herdr(config: &Config) -> Herdr {
    Herdr::new(
        config.herdr.binary.clone(),
        config.herdr.workspace.clone(),
        Config::herdr_session(),
    )
}

pub fn ls() -> anyhow::Result<()> {
    let config = Config::load()?;
    let store = Store::new(config.root()?);
    let live = herdr(&config).list()?;
    let names: Vec<String> = live.iter().map(|a| a.name.clone()).collect();
    let models = models_for(&store, &names);
    for a in &live {
        if let Some(parsed) = SessionName::parse(&a.name) {
            println!(
                "{:<40} {}/{:<20} {:<8} {}",
                a.name,
                parsed.team,
                parsed.agent,
                a.status,
                models.get(&a.name).map(String::as_str).unwrap_or("?"),
            );
        }
    }
    Ok(())
}

pub struct KillOutcome {
    pub killed: Vec<String>,
    pub failed: Vec<String>,
}

/// Close every live pane of team/agent (or unique bare slug).
pub fn kill_agent(reference: &str) -> anyhow::Result<KillOutcome> {
    let config = Config::load()?;
    let h = herdr(&config);
    let store = Store::new(config.root()?);
    let entry = store.resolve(reference)?;
    let mut out = KillOutcome { killed: Vec::new(), failed: Vec::new() };
    for a in h.list()? {
        if SessionName::parse(&a.name).is_some_and(|n| n.team == entry.team && n.agent == entry.slug)
        {
            match h.close(&a.pane_id) {
                Ok(()) => out.killed.push(a.name),
                Err(e) => out.failed.push(format!("{}: {e}", a.name)),
            }
        }
    }
    Ok(out)
}

/// Close one session by exact name, or every session of team/agent | bare slug.
pub fn kill(target: &str) -> anyhow::Result<()> {
    let config = Config::load()?;
    let h = herdr(&config);
    if SessionName::parse(target).is_some() {
        let live = h.list()?;
        let Some(a) = live.iter().find(|a| a.name == target) else {
            println!("no live session named {target}");
            return Ok(());
        };
        h.close(&a.pane_id)?;
        println!("killed {target}");
        return Ok(());
    }
    let out = kill_agent(target)?;
    for name in &out.killed {
        println!("killed {name}");
    }
    if !out.failed.is_empty() {
        anyhow::bail!(
            "killed {}, failed {}: {}",
            out.killed.len(),
            out.failed.len(),
            out.failed.join("; ")
        );
    }
    if out.killed.is_empty() {
        println!("no live sessions for {target}");
    }
    Ok(())
}
```

Behavior note (intentional change): exact-name `kill` of a dead session now prints `no live session named …` instead of erroring — with ephemeral sessions this is the common case, not a failure.

- [ ] **Step 2: Build check + commit**

Run: `cargo check -p cortado 2>&1 | head -30` — `sessions.rs` clean; remaining errors only in `doctor.rs`/`tui/*`.

```bash
git add crates/cortado/src/commands/sessions.rs
git commit -m "feat: sessions/kill drive Herdr panes; ls shows live agent status"
```

---

### Task 7: Doctor — require herdr, drop tmux/Ghostty

**Files:**
- Modify: `crates/cortado/src/commands/doctor.rs`

**Interfaces:**
- Consumes: `cortado_herdr::{parse_herdr_version, parse_status_running}`.
- Produces: `run()` (CLI entry, unchanged name).

- [ ] **Step 1: Replace tmux/ghostty checks**

Delete `check_tmux` and `check_ghostty`. Change `REQUIRED` and add `check_herdr` (reuse the existing `found`, `hint`, `CheckStatus`, `CheckResult` machinery — read the whole file first):

```rust
const REQUIRED: [&str; 2] = ["git", "herdr"];

/// herdr ≥ 0.7 present; detail notes whether the server is up (informational
/// only — cortado auto-starts it on open).
fn check_herdr() -> CheckResult {
    let version_out = Command::new("herdr").arg("--version").output();
    let Ok(out) = version_out else {
        return CheckResult {
            name: "herdr",
            status: CheckStatus::Missing,
            hint: Some(hint("herdr")),
        };
    };
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    match cortado_herdr::parse_herdr_version(&text) {
        Some((ma, mi)) if (ma, mi) >= (0, 7) => {
            let server = Command::new("herdr")
                .args(["status", "server"])
                .output()
                .map(|o| cortado_herdr::parse_status_running(&String::from_utf8_lossy(&o.stdout)))
                .unwrap_or(false);
            let detail = if server {
                format!("({ma}.{mi}, server running)")
            } else {
                format!("({ma}.{mi}, server not running — starts on `cortado open`)")
            };
            CheckResult { name: "herdr", status: CheckStatus::Ok(detail), hint: None }
        }
        Some(found) => CheckResult {
            name: "herdr",
            status: CheckStatus::TooOld { found, need: (0, 7) },
            hint: Some(hint("herdr")),
        },
        None => CheckResult {
            name: "herdr",
            status: CheckStatus::Missing,
            hint: Some(hint("herdr")),
        },
    }
}
```

Wire `check_herdr()` into the checks list where `check_tmux()`/`check_ghostty()` were called (find with `grep -n "check_tmux\|check_ghostty" crates/cortado/src/commands/doctor.rs`), and update any doctor unit tests in the file that reference tmux/ghostty (delete or adapt to herdr). The herdr binary check honors the default binary name only — doctor is a machine health check, not a config linter.

- [ ] **Step 2: Build + run doctor tests + commit**

Run: `cargo check -p cortado 2>&1 | head -30` — doctor clean; remaining errors only in `tui/*`.
Run: `cargo test -p cortado doctor` — PASS (adapt any assertions still naming tmux).

```bash
git add crates/cortado/src/commands/doctor.rs
git commit -m "feat: doctor requires git+herdr, drops tmux/ghostty checks"
```

---

### Task 8: TUI slim — Herdr-sourced snapshot, status badges, no pane management

**Files:**
- Modify: `crates/cortado/src/tui/snapshot.rs`
- Modify: `crates/cortado/src/tui/app.rs`
- Modify: `crates/cortado/src/tui/view.rs`
- Modify: `crates/cortado/src/tui/mod.rs`
- Modify: `crates/cortado/src/commands/mod.rs` / `crates/cortado/src/main.rs` only if they reference removed launcher config (find with `grep -n "launcher\|cortado_term\|Launcher" crates/cortado/src/main.rs crates/cortado/src/commands/mod.rs`)

**Interfaces:**
- Consumes: `Herdr::{list, close}`, `AgentStatus`, `models_for`, `open_session`, `kill_agent`.
- Produces: `Snapshot::build(store, live: &[LiveSession], models)` (same name, new `LiveSession`).

This task touches the largest files (view.rs is 1214 lines) — read each file fully before editing. The compiler drives most of it: change the data types, then fix every use site it reports.

- [ ] **Step 1: Update snapshot types and tests**

In `snapshot.rs`:

```rust
// old:                          // new:
pub struct LiveSession {         pub struct LiveSession {
    pub name: String,                pub name: String,
    pub created_unix: i64,           pub status: cortado_herdr::AgentStatus,
    pub model: Option<String>,       pub pane_id: String,
}                                    pub model: Option<String>,
                                 }

pub struct SessionRow {          pub struct SessionRow {
    pub name: String,                pub name: String,
    pub n: u32,                      pub n: u32,
    pub created_unix: i64,           pub status: cortado_herdr::AgentStatus,
    pub model: String,               pub pane_id: String,
}                                    pub model: String,
                                 }
```

Replace `live_sessions(tmux)` with:

```rust
/// One LiveSession per herdr-started cortado agent; model joined from the
/// append-only spawn log (sessions.jsonl) — no live store to drift.
pub fn live_sessions(
    herdr: &cortado_herdr::Herdr,
    store: &Store,
) -> Result<Vec<LiveSession>, cortado_herdr::HerdrError> {
    let live = herdr.list()?;
    let names: Vec<String> = live.iter().map(|a| a.name.clone()).collect();
    let mut models = cortado_core::session_log::models_for(store, &names);
    Ok(live
        .into_iter()
        .map(|a| LiveSession {
            model: models.remove(&a.name),
            name: a.name,
            status: a.status,
            pane_id: a.pane_id,
        })
        .collect())
}
```

Update `Snapshot::build`'s SessionRow construction to carry `status`/`pane_id` through instead of `created_unix`. Update the unit tests: in `joins_agents_with_their_sessions` replace `created_unix: 100` etc. with `status: cortado_herdr::AgentStatus::Working, pane_id: "w1:p2".into()` (and assert `agent.sessions[1].status == AgentStatus::Working`). **Delete** `builds_from_a_real_tmux_server` and add its herdr replacement using the `IsoSession` pattern from Task 2 (copy the guard struct into this test module; start one dummy agent named `cortado_newsletter_scout_1`, build the snapshot, assert one session row whose `model` is `None` — no sessions.jsonl entry exists — rendered "?" by the view).

Add `cortado-herdr` to the `use` lines; drop `use cortado_tmux::Tmux;`.

- [ ] **Step 2: Fix app.rs and mod.rs compile errors**

Run `cargo check -p cortado 2>&1 | grep "^error" | head -30` and fix each site:

- Wherever the TUI constructed `Tmux::new(config.tmux.socket…)`, construct `Herdr::new(config.herdr.binary.clone(), config.herdr.workspace.clone(), Config::herdr_session())` once and pass it down.
- The 2s-tick refresh calls `live_sessions(&herdr, &store)` (new signature).
- The kill action (`x`) already goes through `commands::sessions::kill_agent` — unchanged.
- The open action (`Enter`, `n`) already goes through `commands::open::open_session` — unchanged; delete any code that afterwards launched a viewer pane or called into `cortado_term` (grep for `cortado_term`, `viewer`, `workspace` in `tui/` and remove those call sites — Herdr now owns presentation; opening focuses the pane itself).
- Uptime display: `SessionRow.created_unix` is gone. Sessions tab columns become `name  model  status` (status replaces uptime — Herdr does not expose creation time; the spec trades uptime for live status).
- `tui/mod.rs` workspace-rail integration (the tmux workspace glue, if present after the uncommitted local edits): remove anything referencing `cortado_term::workspace`.
- Call `commands::open::warn_obsolete_config()` once at TUI startup (same nudge as the CLI).

- [ ] **Step 3: Status badges in view.rs**

Where the rail rendered a green live-count badge per agent, keep the count but color by aggregate status; where the Sessions tab rendered the uptime column, render the status word. Add one pure helper in `view.rs` (unit-testable, follows the file's existing style):

```rust
use cortado_herdr::AgentStatus;

/// Badge color for an agent's session set: any blocked → red, else any
/// working → green, else any idle → yellow, else dim.
pub(crate) fn badge_style(sessions: &[crate::tui::snapshot::SessionRow]) -> ratatui::style::Color {
    use ratatui::style::Color;
    if sessions.iter().any(|s| s.status == AgentStatus::Blocked) {
        Color::Red
    } else if sessions.iter().any(|s| s.status == AgentStatus::Working) {
        Color::Green
    } else if sessions.iter().any(|s| s.status == AgentStatus::Idle) {
        Color::Yellow
    } else {
        Color::DarkGray
    }
}
```

with test:

```rust
    #[test]
    fn badge_prefers_blocked_over_working_over_idle() {
        use cortado_herdr::AgentStatus::*;
        let row = |status| crate::tui::snapshot::SessionRow {
            name: "cortado_t_a_1".into(), n: 1, status, pane_id: "w1:p2".into(), model: "m".into(),
        };
        assert_eq!(badge_style(&[row(Idle), row(Working)]), ratatui::style::Color::Green);
        assert_eq!(badge_style(&[row(Blocked), row(Working)]), ratatui::style::Color::Red);
        assert_eq!(badge_style(&[row(Idle)]), ratatui::style::Color::Yellow);
        assert_eq!(badge_style(&[]), ratatui::style::Color::DarkGray);
    }
```

- [ ] **Step 4: Whole-crate green**

Run: `cargo test -p cortado`
Expected: PASS — including adapted TUI render tests. `cargo check -p cortado` must be error-free. If `cortado` still depends on `cortado-tmux`/`cortado-term` in Cargo.toml, remove those two dep lines now (the crates themselves are deleted next task).

- [ ] **Step 5: Commit**

```bash
git add crates/cortado/src/tui crates/cortado/src/main.rs crates/cortado/src/commands/mod.rs crates/cortado/Cargo.toml
git commit -m "feat: TUI rail reads Herdr — status badges, no pane management"
```

---

### Task 9: Delete `cortado-tmux` + `cortado-term`; workspace-wide green

**Files:**
- Delete: `crates/cortado-tmux/`, `crates/cortado-term/`
- Modify: `Cargo.toml` (members + workspace deps)
- Modify: `crates/cortado-core/src/slug.rs` (doc-comment mention of tmux, if any — check `grep -n tmux crates/cortado-core/src/slug.rs`)

- [ ] **Step 1: Remove crates and references**

```bash
git rm -r crates/cortado-tmux crates/cortado-term
```

Edit root `Cargo.toml`: remove both from `members` and `[workspace.dependencies]`. Then verify no source references remain:

Run: `grep -rn "cortado_tmux\|cortado_term" crates/ --include="*.rs"`
Expected: no output. (A tmux *word* in a doc comment about session-name history may stay if it describes history; code references must be zero.)

- [ ] **Step 2: Full workspace verification**

Run: `cargo build --workspace && cargo test --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: all green. This is the always-green gate for the substrate swap — nothing tmux remains.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock crates/cortado-core/src/slug.rs
git commit -m "feat!: remove tmux/Ghostty substrate — Herdr is the session layer"
```

---

### Task 10: CLI smoke test, docs, and E2E checklist

**Files:**
- Create: `crates/cortado/tests/herdr_cli.rs`
- Modify: `README.md`
- Modify: `crates/cortado/src/commands/init.rs` (drop tmux scaffold text if present)

- [ ] **Step 1: End-to-end CLI test against an isolated herdr session**

**Before writing this test**, run `ls crates/cortado/tests/ && grep -rn "CORTADO_BIN\|CORTADO_CLAUDE_ARGS\|PATH" crates/cortado/tests/*.rs | head -20` and reuse the exact runtime-substitution mechanism the tmux-era tests used (the old tests spawned real sessions with a substitute binary — same trick, new substrate). Delete any old tmux-based integration test files that remain (`grep -l tmux crates/cortado/tests/*.rs`).

`crates/cortado/tests/herdr_cli.rs` (pattern-match the existing assert_cmd tests; the env-seam lines below state the intent — swap in the repo's actual seam names found by the grep above):

```rust
//! cortado CLI ↔ real herdr round-trip on an isolated named session.
use assert_cmd::Command;

fn herdr_available() -> bool {
    std::process::Command::new("herdr").arg("--version").output()
        .map(|o| o.status.success()).unwrap_or(false)
}

struct IsoSession { name: String }
impl IsoSession {
    fn start() -> IsoSession {
        // cortado open auto-starts this session's server via ensure_server.
        IsoSession { name: format!("cortadocli{}", std::process::id()) }
    }
}
impl Drop for IsoSession {
    fn drop(&mut self) {
        std::process::Command::new("herdr").args(["session", "stop", &self.name]).output().ok();
        std::process::Command::new("herdr").args(["session", "delete", &self.name]).output().ok();
    }
}

#[test]
fn open_sessions_kill_round_trip() {
    if !herdr_available() {
        eprintln!("skipping: herdr not installed");
        return;
    }
    let iso = IsoSession::start();
    let root = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    let envs: Vec<(&str, String)> = vec![
        ("CORTADO_ROOT", root.path().to_str().unwrap().to_string()),
        ("CORTADO_CONFIG_DIR", cfg.path().to_str().unwrap().to_string()),
        ("CORTADO_HERDR_SESSION", iso.name.clone()),
        // Runtime substitute: use the SAME seam the old tmux tests used
        // (found via the grep above) so `open` spawns a long-lived dummy
        // process instead of real Claude Code.
    ];

    let run = |args: &[&str], envs: &[(&str, String)]| {
        let mut c = Command::cargo_bin("cortado").unwrap();
        for (k, v) in envs { c.env(k, v); }
        c.args(args).assert()
    };

    run(&["team", "new", "Demo"], &envs).success();
    run(&["agent", "new", "demo/Scout", "--repo-new"], &envs).success();
    run(&["open", "scout"], &envs).success()
        .stdout(predicates::str::contains("cortado_demo_scout_1"));
    run(&["sessions"], &envs).success()
        .stdout(predicates::str::contains("demo/scout"));
    run(&["kill", "scout"], &envs).success()
        .stdout(predicates::str::contains("killed"));
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p cortado --test herdr_cli -- --test-threads=1`
Expected: PASS.

- [ ] **Step 3: README + init updates**

- Requirements table: remove tmux + Ghostty rows; add `herdr ≥ 0.7.4 | session layer & window | yes` with Homebrew install note.
- Install sections: `brew install herdr` replaces `brew install tmux ghostty`; Arch note: install herdr per https://herdr.dev.
- Replace the "Workspace" section with a short "Herdr workspace" section: cortado owns a workspace labeled `Cortado` (rail left, agents right); all layout/moving/zooming is native Herdr; closing panes/clients never loses memory — agents are ephemeral processes over persistent memory (one paragraph; borrow phrasing from the spec summary).
- TUI section: drop the pane-management key rows (right-click menu, C-b keys); badges now show idle/working/blocked; Sessions tab shows status instead of uptime.
- Configuration section: show the new default config (`[general]` + `[herdr]`); document `CORTADO_HERDR_SESSION` as a test seam alongside the existing escape hatches; delete `[tmux]`/`[terminal]` docs.
- Features bullet "Sessions that survive" → rewrite: sessions live in the Herdr server; the durable thing is memory; `cortado open` materializes an agent on demand.
- Roadmap: add `M6 (shipped) — Herdr substrate swap: …` once E2E passes.
- Add migration note: existing tmux-era sessions — finish them out, then `tmux -L cortado kill-server`; cortado no longer sees them.
- `init.rs`: confirm the scaffolded default config matches the new `Config::default_toml()` (it is generated — likely no change; verify by running `cortado init` against a temp `CORTADO_CONFIG_DIR` and reading the file).

- [ ] **Step 4: Manual E2E (macOS, real Herdr + Claude Code)**

Run each and check off:

1. `cargo install --path crates/cortado`
2. `cortado doctor` → git ok, herdr ok (0.7.x), runtimes listed; no tmux/ghostty rows.
3. In a fresh terminal: `herdr` (opens the client) — then `cortado ui` inside a pane: rail renders, teams listed.
4. `cortado open <existing agent>` from another pane → agent pane appears right of the rail, Claude Code boots with memory imports, `CORTADO_SESSION` env set.
5. Rail badge turns green (working) while the agent thinks; yellow (idle) when it waits.
6. Quit Claude Code in the pane → **verify the Stop-hook reflection ran** (check the agent's `MEMORY.md` mtime/content) → observe whether the pane auto-closes → rail shows the agent dormant.
7. `cortado open` the same agent again → `_1` was closed so the name is free → verify it spawns `_1` fresh and the memory written in step 6 is loaded.
8. Close the entire Herdr client window with an agent live → `herdr` again → pane still there, agent still running → `cortado sessions` sees it.
9. `cortado kill <agent>` → pane closes.

If step 6 shows the pane does NOT auto-close when the runtime exits (Herdr may keep an exited pane visible), record the observed behavior in the README's Herdr section; do not hack a workaround into this milestone.

- [ ] **Step 5: Final commit**

```bash
git add README.md crates/cortado/tests/herdr_cli.rs crates/cortado/src/commands/init.rs
git commit -m "feat: herdr CLI round-trip test, README for the Herdr substrate, M6 roadmap entry"
```

---

## Self-review checklist (ran at plan-writing time)

- **Spec coverage:** crate swap (T1/T2/T9), CLI transport only (T1/T2), config retire+add (T3), open/sessions/kill (T5/T6), doctor (T7), slim TUI + badges (T8), M7 plumbing built-now-unused (T1/T2 `send/read/wait_status`), Stop-hook verification (T10 E2E step 6), migration notes + README (T10); the licensing note needs no code. Model display had no spec line but tmux `@cortado_model` forced a decision — resolved via a sessions.jsonl join (T4), consistent with "stateless and honest."
- **Known open risks, stated not hidden:** `agent read` result field name (T2 Step 1 note); pane behavior on child exit (T10 Step 4 item 6); runtime-substitute seam name in old tests (T10 Step 1 note). Each has an explicit verify-and-adapt instruction.
- **Type consistency:** `AgentInfo{name,pane_id,workspace_id,status}` used identically in T1/T2/T5/T6/T8; `LiveSession`/`SessionRow` gain `status`+`pane_id`, lose `created_unix`, everywhere; `models_for(store, &[String]) -> BTreeMap<String, String>` matches T4/T6/T8 call sites.
