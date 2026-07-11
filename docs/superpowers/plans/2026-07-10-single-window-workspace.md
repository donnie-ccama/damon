# Single-Window Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** One Ghostty window hosting everything: the `cortado ui` rail as a left pane of a `cortado_workspace` tmux session, and each opened agent as a viewer pane that nested-attaches to the agent's existing tmux session.

**Architecture:** The workspace is a *viewer* — agent sessions (one detached tmux session per run on the `cortado` socket) are untouched. New tmux primitives in `cortado-tmux`, a `workspace` module + `WorkspaceLauncher` in `cortado-term`, a `Launcher::Workspace` config variant (new default), agent sessions get `prefix None`/`status off` at spawn (workspace mode only), and `cortado ui` bootstraps itself into the workspace when run outside it.

**Tech Stack:** Rust workspace (crates: cortado, cortado-core, cortado-term, cortado-tmux), tmux ≥ 3.2, ratatui. Tests: cargo unit tests + real-tmux integration tests on scratch sockets (pattern: `crates/cortado-tmux/tests/live_server.rs`).

**Spec:** `docs/superpowers/specs/2026-07-10-single-window-workspace-design.md`

## Global Constraints

- tmux ≥ 3.2 (already the project floor; `-e` on `new-session`/`split-window` needs it)
- Workspace session name: `cortado_workspace`; rail width 34 cols; layout `main-vertical`
- Pane tags: user options `@cortado_session` (agent tmux session name) and `@cortado_agent` (team/agent label)
- Agent-session options (workspace mode only): `prefix None`, `status off`
- No behavior change for `launcher = "ghostty" | "env-terminal" | "print"`
- All live tests use scratch sockets `cortado-test-<tag>-<pid>` and `kill_server()` in cleanup; never the real `cortado` socket
- Zero clippy warnings workspace-wide (`cargo clippy --workspace --all-targets`)
- Every commit message ends with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`

---

### Task 1: cortado-tmux pane/window primitives

**Files:**
- Modify: `crates/cortado-tmux/src/lib.rs`
- Test: `crates/cortado-tmux/src/lib.rs` (unit), `crates/cortado-tmux/tests/live_server.rs` (live)

**Interfaces:**
- Consumes: existing `Tmux::run`, `TmuxError`.
- Produces (used by Tasks 3–5):
  - `pub struct PaneInfo { pub id: String, pub session_tag: Option<String> }`
  - `Tmux::split_window(&self, target: &str, env: &BTreeMap<String, String>, command: &[String]) -> Result<String, TmuxError>` (returns pane id `%N`)
  - `Tmux::list_panes(&self, session: &str) -> Result<Vec<PaneInfo>, TmuxError>`
  - `Tmux::select_pane(&self, pane: &str) -> Result<(), TmuxError>`
  - `Tmux::set_pane_option(&self, pane: &str, name: &str, value: &str) -> Result<(), TmuxError>`
  - `Tmux::set_session_options(&self, session: &str, opts: &[(&str, &str)]) -> Result<(), TmuxError>`
  - `Tmux::set_window_option(&self, target: &str, name: &str, value: &str) -> Result<(), TmuxError>`
  - `Tmux::select_layout(&self, target: &str, layout: &str) -> Result<(), TmuxError>`
  - `Tmux::has_client(&self, session: &str) -> Result<bool, TmuxError>`
  - `Tmux::show_session_option(&self, session: &str, name: &str) -> Result<String, TmuxError>` (test helper, also used by Task 5's test)

- [ ] **Step 1: Write the failing unit test** (append inside `mod tests` in `crates/cortado-tmux/src/lib.rs`)

```rust
    #[test]
    fn parses_pane_lines_with_optional_session_tag() {
        assert_eq!(
            parse_pane_line("%3|cortado_newsletter_scout_1"),
            Some(PaneInfo {
                id: "%3".into(),
                session_tag: Some("cortado_newsletter_scout_1".into()),
            })
        );
        // Untagged pane (e.g. the rail) renders an empty trailing field.
        assert_eq!(
            parse_pane_line("%0|"),
            Some(PaneInfo { id: "%0".into(), session_tag: None })
        );
        assert_eq!(parse_pane_line("no-separator"), None);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p cortado-tmux parses_pane_lines 2>&1 | tail -5`
Expected: compile error — `parse_pane_line`/`PaneInfo` not found.

- [ ] **Step 3: Implement** (in `crates/cortado-tmux/src/lib.rs`, after `SessionInfo`)

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneInfo {
    pub id: String,
    /// The `@cortado_session` pane option, if tagged.
    pub session_tag: Option<String>,
}

/// Parse one `#{pane_id}|#{@cortado_session}` line.
fn parse_pane_line(line: &str) -> Option<PaneInfo> {
    let (id, tag) = line.split_once('|')?;
    Some(PaneInfo {
        id: id.to_string(),
        session_tag: (!tag.is_empty()).then(|| tag.to_string()),
    })
}
```

And these methods inside `impl Tmux` (after `set_option`):

```rust
    /// Set several session options in one call (used at spawn/workspace-create).
    pub fn set_session_options(
        &self,
        session: &str,
        opts: &[(&str, &str)],
    ) -> Result<(), TmuxError> {
        for (k, v) in opts {
            self.run(&[
                "set-option".into(),
                "-t".into(),
                session.into(),
                (*k).into(),
                (*v).into(),
            ])?;
        }
        Ok(())
    }

    /// Window-scoped option (e.g. `main-pane-width` on `session:0`).
    pub fn set_window_option(
        &self,
        target: &str,
        name: &str,
        value: &str,
    ) -> Result<(), TmuxError> {
        self.run(&[
            "set-option".into(),
            "-w".into(),
            "-t".into(),
            target.into(),
            name.into(),
            value.into(),
        ])?;
        Ok(())
    }

    /// `split-window -t <target> [-e K=V]... -P -F #{pane_id} -- command...`
    /// Returns the new pane's id (`%N`). Env via `-e` (tmux >= 3.2), same
    /// secrecy rationale as `spawn`.
    pub fn split_window(
        &self,
        target: &str,
        env: &BTreeMap<String, String>,
        command: &[String],
    ) -> Result<String, TmuxError> {
        let mut args: Vec<String> = vec![
            "split-window".into(),
            "-t".into(),
            target.into(),
            "-P".into(),
            "-F".into(),
            "#{pane_id}".into(),
        ];
        for (k, v) in env {
            args.push("-e".into());
            args.push(format!("{k}={v}"));
        }
        args.push("--".into());
        args.extend(command.iter().cloned());
        Ok(self.run(&args)?.trim().to_string())
    }

    pub fn select_pane(&self, pane: &str) -> Result<(), TmuxError> {
        self.run(&["select-pane".into(), "-t".into(), pane.into()])?;
        Ok(())
    }

    /// Pane-scoped user option (`set-option -p`), e.g. `@cortado_session`.
    pub fn set_pane_option(&self, pane: &str, name: &str, value: &str) -> Result<(), TmuxError> {
        self.run(&[
            "set-option".into(),
            "-p".into(),
            "-t".into(),
            pane.into(),
            name.into(),
            value.into(),
        ])?;
        Ok(())
    }

    /// All panes of a session (all windows), with the `@cortado_session` tag.
    pub fn list_panes(&self, session: &str) -> Result<Vec<PaneInfo>, TmuxError> {
        let out = self.run(&[
            "list-panes".into(),
            "-s".into(),
            "-t".into(),
            session.into(),
            "-F".into(),
            "#{pane_id}|#{@cortado_session}".into(),
        ])?;
        Ok(out.lines().filter_map(parse_pane_line).collect())
    }

    pub fn select_layout(&self, target: &str, layout: &str) -> Result<(), TmuxError> {
        self.run(&[
            "select-layout".into(),
            "-t".into(),
            target.into(),
            layout.into(),
        ])?;
        Ok(())
    }

    /// True if any client is attached to the session.
    pub fn has_client(&self, session: &str) -> Result<bool, TmuxError> {
        let out = self.run(&[
            "list-clients".into(),
            "-t".into(),
            session.into(),
            "-F".into(),
            "#{client_name}".into(),
        ])?;
        Ok(out.lines().any(|l| !l.is_empty()))
    }

    /// One session option's value (test/assertion helper).
    pub fn show_session_option(&self, session: &str, name: &str) -> Result<String, TmuxError> {
        let out = self.run(&[
            "show-options".into(),
            "-v".into(),
            "-t".into(),
            session.into(),
            name.into(),
        ])?;
        Ok(out.trim().to_string())
    }
```

- [ ] **Step 4: Run unit test to verify it passes**

Run: `cargo test -p cortado-tmux parses_pane_lines 2>&1 | tail -3`
Expected: `test result: ok. 1 passed`

- [ ] **Step 5: Write the failing live test** (append to `crates/cortado-tmux/tests/live_server.rs`)

```rust
#[test]
fn pane_lifecycle_split_tag_list_layout() {
    let t = scratch("panes");
    let tmp = tempfile::tempdir().unwrap();
    let env = BTreeMap::new();
    t.spawn(
        "cortado_workspace",
        tmp.path(),
        &env,
        &["sleep".to_string(), "30".to_string()],
    )
    .unwrap();

    // Split with env; returned id names a real pane.
    let mut split_env = BTreeMap::new();
    split_env.insert("TMUX".to_string(), String::new());
    let pane = t
        .split_window(
            "cortado_workspace:0",
            &split_env,
            &["sleep".to_string(), "30".to_string()],
        )
        .unwrap();
    assert!(pane.starts_with('%'), "pane id, got {pane}");

    // Tag it; list_panes surfaces the tag; the rail pane stays untagged.
    t.set_pane_option(&pane, "@cortado_session", "cortado_t_a_1")
        .unwrap();
    t.set_pane_option(&pane, "@cortado_agent", "t/a").unwrap();
    let panes = t.list_panes("cortado_workspace").unwrap();
    assert_eq!(panes.len(), 2);
    assert_eq!(
        panes
            .iter()
            .find(|p| p.id == pane)
            .unwrap()
            .session_tag
            .as_deref(),
        Some("cortado_t_a_1")
    );
    assert_eq!(
        panes.iter().filter(|p| p.session_tag.is_none()).count(),
        1
    );

    // Layout + selection + options: exercised for errors, not geometry.
    t.set_window_option("cortado_workspace:0", "main-pane-width", "34")
        .unwrap();
    t.select_layout("cortado_workspace:0", "main-vertical").unwrap();
    t.select_pane(&pane).unwrap();
    t.set_session_options("cortado_workspace", &[("mouse", "on")])
        .unwrap();
    assert_eq!(
        t.show_session_option("cortado_workspace", "mouse").unwrap(),
        "on"
    );
    // No client attached in CI.
    assert!(!t.has_client("cortado_workspace").unwrap());

    t.kill_server().ok();
}
```

- [ ] **Step 6: Run live test to verify it passes**

Run: `cargo test -p cortado-tmux --test live_server pane_lifecycle 2>&1 | tail -3`
Expected: `test result: ok. 1 passed` (implementation already landed in Step 3; this validates it against real tmux)

- [ ] **Step 7: Commit**

```bash
git add crates/cortado-tmux
git commit -m "tmux: pane primitives for the workspace (split/list/tag/layout/clients)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: config — `Launcher::Workspace` + `window` opener key

**Files:**
- Modify: `crates/cortado-core/src/config.rs`
- Test: same file's `mod tests`

**Interfaces:**
- Produces (used by Tasks 4, 5, 6):
  - `Launcher` gains variant `Workspace` (serde kebab-case `workspace`); new default for `TerminalCfg.launcher`
  - `pub enum Window { Ghostty, EnvTerminal, Print }` (kebab-case), `TerminalCfg` gains `pub window: Window` defaulting to `Window::Ghostty` — the one-time OS-window opener used by workspace mode

- [ ] **Step 1: Write the failing tests** (replace `defaults_are_spec_values` and add one; in `crates/cortado-core/src/config.rs` `mod tests`)

```rust
    #[test]
    fn defaults_are_spec_values() {
        let c = Config::default();
        assert_eq!(c.general.root, "~/cortado");
        assert_eq!(c.general.default_runtime, "claude");
        assert_eq!(c.tmux.socket, "cortado");
        assert_eq!(c.terminal.launcher, Launcher::Workspace);
        assert_eq!(c.terminal.window, Window::Ghostty);
    }

    #[test]
    fn workspace_launcher_and_window_parse() {
        let c: Config =
            toml::from_str("[terminal]\nlauncher = \"workspace\"\nwindow = \"print\"\n").unwrap();
        assert_eq!(c.terminal.launcher, Launcher::Workspace);
        assert_eq!(c.terminal.window, Window::Print);
        // Old configs without `window` still parse.
        let c: Config = toml::from_str("[terminal]\nlauncher = \"ghostty\"\n").unwrap();
        assert_eq!(c.terminal.launcher, Launcher::Ghostty);
        assert_eq!(c.terminal.window, Window::Ghostty);
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p cortado-core config 2>&1 | tail -5`
Expected: compile error — no `Workspace` variant / no `Window`.

- [ ] **Step 3: Implement** (in `crates/cortado-core/src/config.rs`)

```rust
#[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct TerminalCfg {
    pub launcher: Launcher,
    /// How workspace mode opens its one OS window. Ignored by other launchers.
    pub window: Window,
}

impl Default for TerminalCfg {
    fn default() -> Self {
        TerminalCfg {
            launcher: Launcher::Workspace,
            window: Window::Ghostty,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Launcher {
    Workspace,
    Ghostty,
    EnvTerminal,
    Print,
}

#[derive(Debug, PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Window {
    Ghostty,
    EnvTerminal,
    Print,
}
```

Also check the existing `partial_file_fills_defaults` test still compiles and passes (it parses `launcher = "print"`, which remains valid).

- [ ] **Step 4: Run tests**

Run: `cargo test -p cortado-core 2>&1 | tail -3`
Expected: all pass. (`default_toml_round_trips` keeps passing because serialize/deserialize both handle the new field.)

- [ ] **Step 5: Fix downstream compile breaks now, minimally.** `cortado-term`'s `launcher_for` matches on `Launcher` and will fail to compile (non-exhaustive match). Add a temporary arm in `crates/cortado-term/src/lib.rs` (replaced in Task 4):

```rust
        L::Workspace => Box::new(PrintLauncher { socket }), // placeholder until WorkspaceLauncher (Task 4)
```

Run: `cargo build --workspace 2>&1 | tail -3` → expected: success.

- [ ] **Step 6: Commit**

```bash
git add crates/cortado-core crates/cortado-term
git commit -m "config: launcher = workspace (new default) + window opener key

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: cortado-term `workspace` module (ensure + open viewer)

**Files:**
- Create: `crates/cortado-term/src/workspace.rs`
- Modify: `crates/cortado-term/src/lib.rs` (add `pub mod workspace;` at top), `crates/cortado-term/Cargo.toml` (add deps)
- Test: unit tests in `workspace.rs`; live tests in `crates/cortado-term/tests/live_workspace.rs` (new)

**Interfaces:**
- Consumes: Task 1 primitives (`split_window`, `list_panes`, `set_pane_option`, `select_pane`, `select_layout`, `set_window_option`, `set_session_options`, `spawn`, `has`).
- Produces (used by Tasks 4–6):
  - `pub const WORKSPACE_SESSION: &str = "cortado_workspace";`
  - `pub const AGENT_SESSION_OPTIONS: &[(&str, &str)] = &[("prefix", "None"), ("status", "off")];`
  - `pub fn viewer_command(socket: &str, session: &str) -> Vec<String>`
  - `pub fn ensure_workspace(tmux: &Tmux, cwd: &Path, rail_command: &[String]) -> anyhow::Result<()>`
  - `pub fn open_viewer(tmux: &Tmux, session: &str, agent_label: &str) -> anyhow::Result<String>`

- [ ] **Step 1: Cargo deps.** In `crates/cortado-term/Cargo.toml` under `[dependencies]` add `cortado-tmux = { path = "../cortado-tmux" }` (match the existing path-dep style used for `cortado-core`). Under `[dev-dependencies]` add `tempfile` (same version the workspace uses elsewhere; check `crates/cortado-tmux/Cargo.toml`).

- [ ] **Step 2: Write the failing unit test** (the `mod tests` block that will sit at the bottom of the new `crates/cortado-term/src/workspace.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewer_command_is_a_nested_attach() {
        assert_eq!(
            viewer_command("cortado", "cortado_t_a_1"),
            vec!["tmux", "-L", "cortado", "attach", "-t", "cortado_t_a_1"]
        );
    }

    #[test]
    fn agent_session_options_disable_inner_prefix_and_status() {
        assert_eq!(
            AGENT_SESSION_OPTIONS,
            &[("prefix", "None"), ("status", "off")]
        );
    }
}
```

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p cortado-term viewer_command 2>&1 | tail -5`
Expected: compile error (module contents missing).

- [ ] **Step 4: Implement `crates/cortado-term/src/workspace.rs`**

```rust
//! The single-window workspace: a `cortado_workspace` tmux session hosting
//! the rail (cortado ui) in pane 0 and one nested-attach viewer pane per
//! opened agent. The workspace is a *viewer*: agent sessions are untouched,
//! and killing viewer panes only detaches clients.
use cortado_tmux::Tmux;
use std::collections::BTreeMap;
use std::path::Path;

pub const WORKSPACE_SESSION: &str = "cortado_workspace";

/// Applied to agent sessions at spawn (workspace mode only): the outer
/// workspace tmux owns every key and draws the only status bar.
pub const AGENT_SESSION_OPTIONS: &[(&str, &str)] = &[("prefix", "None"), ("status", "off")];

/// Applied to the workspace session at creation.
const WORKSPACE_OPTIONS: &[(&str, &str)] = &[
    ("mouse", "on"),
    ("status", "on"),
    ("pane-border-status", "top"),
    (
        "pane-border-format",
        " #{?#{@cortado_agent},#{@cortado_agent},cortado} ",
    ),
];

const RAIL_WIDTH: &str = "34";

/// Command a viewer pane runs: a nested tmux client for the agent session.
/// `TMUX=` is cleared via split-window env so the inner client starts.
pub fn viewer_command(socket: &str, session: &str) -> Vec<String> {
    ["tmux", "-L", socket, "attach", "-t", session]
        .map(String::from)
        .to_vec()
}

/// Create the workspace session (detached) if missing: rail in pane 0
/// running `rail_command`, workspace options applied. Idempotent.
pub fn ensure_workspace(tmux: &Tmux, cwd: &Path, rail_command: &[String]) -> anyhow::Result<()> {
    if tmux.has(WORKSPACE_SESSION)? {
        return Ok(());
    }
    let mut env: BTreeMap<String, String> = BTreeMap::new();
    env.insert("CORTADO_WORKSPACE".into(), "1".into());
    // The rail must see the same cortado world as the CLI that spawned it.
    for var in ["CORTADO_CONFIG_DIR", "CORTADO_ROOT"] {
        if let Ok(v) = std::env::var(var) {
            env.insert(var.into(), v);
        }
    }
    tmux.spawn(WORKSPACE_SESSION, cwd, &env, rail_command)?;
    tmux.set_session_options(WORKSPACE_SESSION, WORKSPACE_OPTIONS)?;
    Ok(())
}

/// Open (or just focus) the viewer pane for `session`. Returns the pane id.
pub fn open_viewer(tmux: &Tmux, session: &str, agent_label: &str) -> anyhow::Result<String> {
    if let Some(p) = tmux
        .list_panes(WORKSPACE_SESSION)?
        .into_iter()
        .find(|p| p.session_tag.as_deref() == Some(session))
    {
        tmux.select_pane(&p.id)?;
        return Ok(p.id);
    }
    let target = format!("{WORKSPACE_SESSION}:0");
    let mut env: BTreeMap<String, String> = BTreeMap::new();
    env.insert("TMUX".into(), String::new()); // allow the nested client
    let pane = tmux.split_window(&target, &env, &viewer_command(tmux.socket(), session))?;
    tmux.set_pane_option(&pane, "@cortado_session", session)?;
    tmux.set_pane_option(&pane, "@cortado_agent", agent_label)?;
    // Rail (pane 0) is main-vertical's main pane on the left; viewers stack
    // right. The user may rearrange freely afterwards — we only re-apply the
    // layout when *adding* a pane.
    tmux.set_window_option(&target, "main-pane-width", RAIL_WIDTH)?;
    tmux.select_layout(&target, "main-vertical")?;
    tmux.select_pane(&pane)?;
    Ok(pane)
}
```

And in `crates/cortado-term/src/lib.rs` line 1 area, add:

```rust
pub mod workspace;
```

- [ ] **Step 5: Run unit tests**

Run: `cargo test -p cortado-term 2>&1 | tail -3`
Expected: pass (existing + 2 new).

- [ ] **Step 6: Write the live test** — create `crates/cortado-term/tests/live_workspace.rs`

```rust
use cortado_term::workspace;
use cortado_tmux::Tmux;
use std::collections::BTreeMap;

fn scratch(tag: &str) -> Tmux {
    Tmux::new(format!("cortado-test-{}-{}", tag, std::process::id()))
}

fn sleep_cmd() -> Vec<String> {
    vec!["sleep".to_string(), "30".to_string()]
}

#[test]
fn workspace_viewer_lifecycle() {
    let t = scratch("wsviewer");
    let tmp = tempfile::tempdir().unwrap();

    // A fake agent session, as open_session would have spawned it.
    t.spawn("cortado_t_a_1", tmp.path(), &BTreeMap::new(), &sleep_cmd())
        .unwrap();

    // ensure_workspace is idempotent and applies options.
    workspace::ensure_workspace(&t, tmp.path(), &sleep_cmd()).unwrap();
    workspace::ensure_workspace(&t, tmp.path(), &sleep_cmd()).unwrap();
    assert!(t.has(workspace::WORKSPACE_SESSION).unwrap());
    assert_eq!(
        t.show_session_option(workspace::WORKSPACE_SESSION, "mouse")
            .unwrap(),
        "on"
    );

    // First open splits a tagged viewer pane; second open reuses it.
    let p1 = workspace::open_viewer(&t, "cortado_t_a_1", "t/a").unwrap();
    let p2 = workspace::open_viewer(&t, "cortado_t_a_1", "t/a").unwrap();
    assert_eq!(p1, p2, "same agent must not get a duplicate viewer pane");
    let panes = t.list_panes(workspace::WORKSPACE_SESSION).unwrap();
    assert_eq!(panes.len(), 2, "rail + one viewer, got {panes:?}");

    // Killing the workspace never touches the agent session.
    t.kill(workspace::WORKSPACE_SESSION).unwrap();
    assert!(t.has("cortado_t_a_1").unwrap());

    t.kill_server().ok();
}

#[test]
fn viewer_pane_attaches_nested_client() {
    let t = scratch("wsnest");
    let tmp = tempfile::tempdir().unwrap();
    t.spawn("cortado_t_a_1", tmp.path(), &BTreeMap::new(), &sleep_cmd())
        .unwrap();
    workspace::ensure_workspace(&t, tmp.path(), &sleep_cmd()).unwrap();
    workspace::open_viewer(&t, "cortado_t_a_1", "t/a").unwrap();

    // The viewer's inner `tmux attach` becomes a client of the agent session.
    // Poll briefly: the pane process needs a moment to start.
    let mut attached = false;
    for _ in 0..20 {
        if t.has_client("cortado_t_a_1").unwrap() {
            attached = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    assert!(attached, "viewer pane never attached to the agent session");

    t.kill_server().ok();
}
```

- [ ] **Step 7: Run live tests**

Run: `cargo test -p cortado-term --test live_workspace 2>&1 | tail -3`
Expected: `test result: ok. 2 passed`

- [ ] **Step 8: Commit**

```bash
git add crates/cortado-term
git commit -m "term: workspace module — ensure session, nested-attach viewer panes

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: `WorkspaceLauncher` + window opener wiring

**Files:**
- Modify: `crates/cortado-term/src/lib.rs`
- Modify: `crates/cortado/src/commands/open.rs:136-137` (the `launcher_for` call)
- Test: unit tests in `crates/cortado-term/src/lib.rs`

**Interfaces:**
- Consumes: Task 2 (`Launcher::Workspace`, `Window`, `TerminalCfg`), Task 3 (`ensure_workspace`, `open_viewer`, `WORKSPACE_SESSION`).
- Produces:
  - `pub struct WorkspaceLauncher { pub socket: String, pub window: cortado_core::config::Window }` implementing `TerminalLauncher`
  - `pub fn open_window(window: cortado_core::config::Window, socket: &str, session: &str) -> anyhow::Result<()>` — opens ONE OS window attached to `session` via Ghostty / `$TERMINAL` / print (used by both `WorkspaceLauncher` and Task 6's `cortado ui` bootstrap)
  - **Signature change:** `launcher_for(cfg: &cortado_core::config::TerminalCfg, socket: String) -> Box<dyn TerminalLauncher>` (was `cfg: Launcher`)

- [ ] **Step 1: Write the failing unit test** (in `crates/cortado-term/src/lib.rs` `mod tests`)

```rust
    #[test]
    fn open_window_print_never_fails_and_workspace_launcher_constructs() {
        open_window(
            cortado_core::config::Window::Print,
            "cortado",
            workspace::WORKSPACE_SESSION,
        )
        .unwrap();
        let _ = WorkspaceLauncher {
            socket: "cortado".into(),
            window: cortado_core::config::Window::Print,
        };
    }
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p cortado-term open_window_print 2>&1 | tail -5`
Expected: compile error — `open_window`/`WorkspaceLauncher` not found.

- [ ] **Step 3: Implement** (in `crates/cortado-term/src/lib.rs`)

```rust
/// Open one OS window attached to `session`, per the configured window kind.
pub fn open_window(
    window: cortado_core::config::Window,
    socket: &str,
    session: &str,
) -> anyhow::Result<()> {
    use cortado_core::config::Window as W;
    let launcher: Box<dyn TerminalLauncher> = match window {
        W::Ghostty => Box::new(GhosttyLauncher { socket: socket.to_string() }),
        W::EnvTerminal => Box::new(EnvTerminalLauncher { socket: socket.to_string() }),
        W::Print => Box::new(PrintLauncher { socket: socket.to_string() }),
    };
    launcher.open(session, session)
}

/// Single-window mode: agents open as viewer panes inside the
/// `cortado_workspace` session; at most one OS window is ever launched.
pub struct WorkspaceLauncher {
    pub socket: String,
    pub window: cortado_core::config::Window,
}

impl TerminalLauncher for WorkspaceLauncher {
    fn open(&self, session: &str, title: &str) -> anyhow::Result<()> {
        let tmux = cortado_tmux::Tmux::new(self.socket.clone());
        let rail = vec![
            std::env::current_exe()?.display().to_string(),
            "ui".to_string(),
        ];
        let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
        workspace::ensure_workspace(&tmux, std::path::Path::new(&home), &rail)?;
        workspace::open_viewer(&tmux, session, title)?;
        if !tmux.has_client(workspace::WORKSPACE_SESSION)? {
            open_window(self.window, &self.socket, workspace::WORKSPACE_SESSION)?;
        }
        Ok(())
    }
}
```

Replace `launcher_for` (including the Task 2 placeholder arm):

```rust
pub fn launcher_for(
    cfg: &cortado_core::config::TerminalCfg,
    socket: String,
) -> Box<dyn TerminalLauncher> {
    use cortado_core::config::Launcher as L;
    match cfg.launcher {
        L::Workspace => Box::new(WorkspaceLauncher {
            socket,
            window: cfg.window,
        }),
        L::Ghostty => Box::new(GhosttyLauncher { socket }),
        L::EnvTerminal => Box::new(EnvTerminalLauncher { socket }),
        L::Print => Box::new(PrintLauncher { socket }),
    }
}
```

Update the call site `crates/cortado/src/commands/open.rs:136`:

```rust
    cortado_term::launcher_for(&config.terminal, config.tmux.socket.clone())
        .open(&session, &format!("{}/{}", entry.team, entry.slug))?;
```

- [ ] **Step 4: Build + test**

Run: `cargo test --workspace 2>&1 | grep -E "test result|error" | tail -10`
Expected: all green. If any existing test constructed `launcher_for(Launcher::Print, ...)`, update it to pass a `TerminalCfg { launcher: Launcher::Print, window: Window::Print }`.

- [ ] **Step 5: Commit**

```bash
git add crates/cortado-term crates/cortado/src/commands/open.rs
git commit -m "term: WorkspaceLauncher — agents open as viewer panes, one OS window

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: agent sessions get `prefix None` / `status off` (workspace mode)

**Files:**
- Modify: `crates/cortado/src/commands/open.rs` (after the `set_option @cortado_model` block, ~line 119)
- Test: `crates/cortado/tests/cli_open.rs`

**Interfaces:**
- Consumes: `Tmux::set_session_options`, `Tmux::show_session_option` (Task 1), `workspace::AGENT_SESSION_OPTIONS` (Task 3), `Launcher::Workspace` (Task 2).
- Produces: no new API; a spawn-time behavior contract — in workspace mode every agent session has `prefix None` and `status off`.

- [ ] **Step 1: Read `crates/cortado/tests/cli_open.rs` first** to reuse its existing scaffolding (scratch socket, `CORTADO_CONFIG_DIR` temp config, `CORTADO_CLAUDE_ARGS` sleep seam). Follow its established helper pattern exactly — do not invent a new one.

- [ ] **Step 2: Write the failing test** (append to `crates/cortado/tests/cli_open.rs`, arranged with that file's own helpers — team/agent creation, temp config dir, scratch socket, `CORTADO_CLAUDE_ARGS` seam — with the config's `[terminal]` block set to `launcher = "workspace"` and `window = "print"`; then `cortado open <agent>` via assert_cmd). Assertions:

```rust
    // Assert: the agent session exists AND has the inner-tmux options.
    let tmux = cortado_tmux::Tmux::new(socket.clone());
    let sessions = tmux.list().unwrap();
    let agent_session = sessions
        .iter()
        .find(|s| s.starts_with("cortado_") && *s != "cortado_workspace")
        .expect("agent session spawned");
    assert_eq!(
        tmux.show_session_option(agent_session, "prefix").unwrap(),
        "None"
    );
    assert_eq!(
        tmux.show_session_option(agent_session, "status").unwrap(),
        "off"
    );
    // And the workspace exists with a viewer pane for it.
    let panes = tmux.list_panes("cortado_workspace").unwrap();
    assert!(panes
        .iter()
        .any(|p| p.session_tag.as_deref() == Some(agent_session.as_str())));

    tmux.kill_server().ok();
```

The executor inlines real arrangement code found in Step 1 — no comment placeholders may survive into the committed test. Note: with `launcher = "workspace"`, the workspace's rail pane runs `<test cortado binary> ui`; inside the pane `CORTADO_WORKSPACE=1` is set by `ensure_workspace`, so the rail draws (or exits on missing tty inside tmux — either is harmless to this test), and `CORTADO_CONFIG_DIR`/`CORTADO_ROOT` are propagated into the pane env by `ensure_workspace`.

- [ ] **Step 3: Run to verify failure**

Run: `cargo test -p cortado --test cli_open workspace_mode 2>&1 | tail -5`
Expected: FAIL — `prefix` is the default, not `None`.

- [ ] **Step 4: Implement** (in `crates/cortado/src/commands/open.rs`, right after the `@cortado_model` `set_option` block)

```rust
        // Workspace mode: the outer workspace tmux owns all keys and draws
        // the only status bar; the agent session must not compete.
        if config.terminal.launcher == cortado_core::config::Launcher::Workspace {
            if let Err(e) =
                tmux.set_session_options(&name, cortado_term::workspace::AGENT_SESSION_OPTIONS)
            {
                warnings.push(format!("could not set workspace session options: {e}"));
            }
        }
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p cortado --test cli_open 2>&1 | tail -3`
Expected: all pass, including the new one.

- [ ] **Step 6: Commit**

```bash
git add crates/cortado
git commit -m "open: disable inner prefix/status on agent sessions in workspace mode

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: `cortado ui` bootstraps into the workspace

**Files:**
- Modify: `crates/cortado/src/tui/mod.rs` (`run()` + two new fns)
- Modify: `docs/superpowers/specs/2026-07-10-single-window-workspace-design.md` (decisions log)
- Test: unit test in `crates/cortado/src/tui/mod.rs`; live test in `crates/cortado/tests/cli_ui.rs`

**Interfaces:**
- Consumes: Task 3 (`ensure_workspace`, `WORKSPACE_SESSION`), Task 4 (`open_window`), Task 2 (`Launcher::Workspace`, `Window`).
- Produces: `cortado ui` behavior — outside the workspace (no `$CORTADO_WORKSPACE`, no `$TMUX`) with `launcher = "workspace"`, it creates the workspace and opens the window instead of running the rail inline.

- [ ] **Step 1: Write the failing unit test** (new `mod tests` at the bottom of `crates/cortado/src/tui/mod.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use cortado_core::config::Launcher;

    #[test]
    fn bootstraps_only_outside_workspace_and_tmux_in_workspace_mode() {
        let ws = |l: Launcher| {
            let mut c = Config::default();
            c.terminal.launcher = l;
            c
        };
        assert!(should_bootstrap(&ws(Launcher::Workspace), false, false));
        assert!(!should_bootstrap(&ws(Launcher::Workspace), true, false)); // already the rail
        assert!(!should_bootstrap(&ws(Launcher::Workspace), false, true)); // user is in tmux
        assert!(!should_bootstrap(&ws(Launcher::Ghostty), false, false)); // legacy mode
    }
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test -p cortado should_bootstrap 2>&1 | tail -5`
Expected: compile error — `should_bootstrap` not found.

- [ ] **Step 3: Implement** (in `crates/cortado/src/tui/mod.rs`)

Replace the top of `run()` (note: exactly one `Config::load()` remains):

```rust
pub fn run() -> anyhow::Result<()> {
    let config = Config::load()?;
    if should_bootstrap(
        &config,
        std::env::var_os("CORTADO_WORKSPACE").is_some(),
        std::env::var_os("TMUX").is_some(),
    ) {
        return bootstrap_workspace(&config);
    }
    if !std::io::stdout().is_terminal() {
        anyhow::bail!("cortado ui needs an interactive terminal");
    }
    // ratatui::init() enables raw mode + alternate screen and installs a
    // panic hook that restores the terminal.
    let terminal = ratatui::init();
    let result = event_loop(terminal, &config);
    ratatui::restore();
    result
}

/// Workspace mode, invoked from a plain shell: become the workspace instead
/// of drawing the rail inline. Inside the workspace pane (CORTADO_WORKSPACE)
/// or any tmux ($TMUX) we draw the rail directly.
fn should_bootstrap(config: &Config, in_workspace_pane: bool, in_tmux: bool) -> bool {
    config.terminal.launcher == cortado_core::config::Launcher::Workspace
        && !in_workspace_pane
        && !in_tmux
}

fn bootstrap_workspace(config: &Config) -> anyhow::Result<()> {
    let tmux = Tmux::new(config.tmux.socket.clone());
    let rail = vec![
        std::env::current_exe()?.display().to_string(),
        "ui".to_string(),
    ];
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    cortado_term::workspace::ensure_workspace(&tmux, std::path::Path::new(&home), &rail)?;
    cortado_term::open_window(
        config.terminal.window,
        &config.tmux.socket,
        cortado_term::workspace::WORKSPACE_SESSION,
    )?;
    println!("workspace {}", cortado_term::workspace::WORKSPACE_SESSION);
    Ok(())
}
```

- [ ] **Step 4: Fix `cli_ui.rs`.** Read `crates/cortado/tests/cli_ui.rs`. The no-tty test must keep testing the rail path: ensure its temp config sets `launcher = "print"` (add a config file if the test relied on defaults — the default is now `workspace`). Then append a bootstrap test that: writes a temp config with `launcher = "workspace"`, `window = "print"`, and a scratch socket (`cortado-test-uiboot-<pid>`); runs `cortado ui` with no tty via assert_cmd; asserts success with stdout containing `workspace cortado_workspace` and `attach with: tmux -L`; asserts `Tmux::new(socket).has("cortado_workspace")`; and finishes with `kill_server()`. Use `cli_open.rs`'s config-dir helper pattern — inline real code, no comment placeholders.

- [ ] **Step 5: Run tests**

Run: `cargo test -p cortado 2>&1 | grep -E "test result"`
Expected: all green.

- [ ] **Step 6: Amend the spec's decisions log** (in `docs/superpowers/specs/2026-07-10-single-window-workspace-design.md`, replace the second bullet)

```markdown
- `prefix None`/`status off` applied to agent sessions **only when spawned in
  workspace mode**: in legacy one-window-per-agent mode the inner session IS
  the window's tmux, and disabling its prefix would kill splits/detach there —
  the very bug this project fixes. Sessions spawned under one mode and viewed
  under the other are a documented edge case.
```

- [ ] **Step 7: Commit**

```bash
git add crates/cortado docs/superpowers/specs/2026-07-10-single-window-workspace-design.md
git commit -m "ui: bootstrap cortado ui into the single-window workspace

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 7: docs, lint, full verification, PR

**Files:**
- Modify: `README.md` (Configuration + TUI sections, roadmap)
- Test: full suite + clippy + fmt

- [ ] **Step 1: README.** In the Configuration block, update the `[terminal]` defaults and document both keys:

```toml
[terminal]
launcher = "workspace"    # workspace | ghostty | env-terminal | print
window = "ghostty"        # workspace mode's one OS window: ghostty | env-terminal | print
```

Add a `## Workspace` section after `## TUI` (keep README voice: short, factual):

```markdown
## Workspace

With `launcher = "workspace"` (the default), everything lives in **one**
terminal window: `cortado ui` runs as a slim rail pane inside a
`cortado_workspace` tmux session, and every opened agent becomes a viewer
pane beside it, nested-attached to the agent's own tmux session. All tmux
keys are native — `C-b %` / `C-b "` split, `C-b z` zooms an agent
full-screen, the mouse resizes panes — and every pane is labeled with its
agent. Closing a viewer pane (or the whole window) only detaches; agents
keep running, exactly as before. `cortado open` from any shell lands the
agent in the workspace, starting it if needed. The old
one-window-per-agent behavior remains: set `launcher = "ghostty"`.
```

Update `## Status & roadmap` with a new shipped line for the workspace.

- [ ] **Step 2: Full verification**

```bash
cargo fmt --all
cargo clippy --workspace --all-targets 2>&1 | tail -5   # expect: zero warnings
cargo test --workspace 2>&1 | grep -E "test result|FAILED"  # expect: all ok
```

- [ ] **Step 3: Commit + push + PR**

```bash
git add README.md
git commit -m "docs: workspace single-window mode

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
git push -u origin workspace-single-window
gh pr create --head workspace-single-window --base main \
  --title "Single-window workspace: rail + agent viewer panes in one tmux client" \
  --body "Implements docs/superpowers/specs/2026-07-10-single-window-workspace-design.md"
```

- [ ] **Step 4: Manual smoke checklist for the user (macOS)** — include in the PR body / final report:
  - `cortado ui` from a plain shell → exactly one Ghostty window opens, rail on the left
  - Rail `Enter` on an agent → viewer pane appears right of the rail, Claude Code visible, pane labeled with the agent
  - `C-b %` and `C-b "` split the window; `C-b z` zooms; mouse-drag resizes
  - Close the window; `cortado sessions` still lists the agent; `cortado ui` reattaches with panes intact
  - `x` kill in the rail → viewer pane closes by itself

---

## Self-review notes

- **Spec coverage:** viewer workspace (T3/T4), key routing + inner options (T5), bootstrap (T6), config/default (T2), tmux primitives (T1), README (T7). Error paths: workspace idempotence via `has()` check; tmux errors surface through the existing status-line/CLI plumbing — no new code needed. Tests map to the spec's testing section (T1/T3/T5/T6 live + T7 manual).
- **Intentional deviation from spec:** agent-session options applied only in workspace mode; T6 amends the spec's decisions log with the rationale (legacy mode's inner session IS the window's tmux — disabling its prefix would recreate the original bug).
- **Type consistency:** `ensure_workspace(&Tmux, &Path, &[String])`, `open_viewer(&Tmux, &str, &str) -> anyhow::Result<String>`, `launcher_for(&TerminalCfg, String)`, `open_window(Window, &str, &str)` — used identically across T4/T5/T6.
