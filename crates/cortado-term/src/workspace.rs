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
