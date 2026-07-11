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
    ("status-style", "bg=#1a1b26,fg=#78798c"),
    ("status-left", " #[fg=#d79a68,bold]CORTADO#[default] "),
    ("status-left-length", "16"),
    ("pane-border-status", "top"),
    ("pane-border-style", "fg=#454658"),
    ("pane-active-border-style", "fg=#d79a68"),
    ("menu-style", "bg=#1a1b26,fg=#c0caf5"),
    ("menu-selected-style", "bg=#d79a68,fg=#1a1b26,bold"),
    ("menu-border-style", "fg=#454658"),
    (
        "pane-border-format",
        " #[fg=#78798c]#{?#{@cortado_agent},◇ #{@cortado_agent},roster}#[default] ",
    ),
    (
        "status-right",
        "#[fg=#a9a1d6]right-click#[default] pane menu · #[fg=#d79a68,bold]C-b H/L#[default] resize ",
    ),
];

/// Fixed rail (sidebar) width in columns.
pub const RAIL_WIDTH: u16 = 34;

/// Repeatable resize keys bound on the cortado tmux server: `C-b H/L`
/// widen/narrow by 5 columns, `C-b J/K` grow/shrink by 3 rows. Communicated
/// in the workspace status bar and the README.
const RESIZE_BINDINGS: &[(&str, &[&str])] = &[
    ("H", &["resize-pane", "-L", "5"]),
    ("L", &["resize-pane", "-R", "5"]),
    ("J", &["resize-pane", "-D", "3"]),
    ("K", &["resize-pane", "-U", "3"]),
];

/// Right-clicking anywhere except the roster opens a Cortado-styled pane menu.
/// `-t =` makes the pane under the mouse the command target. Layout actions
/// keep the roster as the main pane; splits intentionally open scratch shells.
const PANE_MENU_BINDING: &[&str] = &[
    "if-shell",
    "-F",
    "-t",
    "=",
    "#{!=:#{@cortado_roster},1}",
    "display-menu -T '#[fg=#d79a68,bold] CORTADO ' -t = -x M -y M '#{?window_zoomed_flag,Restore pane,Zoom pane}' z 'resize-pane -Z -t =' '' 'Split left / right (shell)' h 'split-window -h -c \"#{pane_current_path}\" -t =' 'Split top / bottom (shell)' v 'split-window -v -c \"#{pane_current_path}\" -t =' '' 'Stack viewers vertically' s 'set-option -w -t = main-pane-width 34 ; select-layout -t = main-vertical' 'Viewers side by side' b 'select-layout -t = even-horizontal ; resize-pane -t \"{top-left}\" -x 34' '' 'Close pane' x 'kill-pane -t ='",
];

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
    if !tmux.has(WORKSPACE_SESSION)? {
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
    }
    // The server is running now (bind-key can't start one). Bindings are
    // server-wide and idempotent: (re)apply every time so upgrades reach
    // already-running servers.
    if let Some(rail) = tmux.list_panes(WORKSPACE_SESSION)?.first() {
        tmux.set_pane_option(&rail.id, "@cortado_roster", "1")?;
    }
    for (key, command) in RESIZE_BINDINGS {
        tmux.bind_key(true, key, command)?;
    }
    tmux.bind_root_key("MouseDown3Pane", PANE_MENU_BINDING)?;
    Ok(())
}

/// Equal widths for the `n` panes sharing the space right of the rail.
/// `total` is the window width; each of the `n` panes gets an equal share of
/// what remains after the rail and the `n` one-column pane borders, with the
/// last pane absorbing the rounding remainder.
pub fn viewer_widths(total: u16, n: usize) -> Vec<u16> {
    if n == 0 {
        return Vec::new();
    }
    let n16 = n as u16;
    let avail = total.saturating_sub(RAIL_WIDTH + n16).max(n16);
    let each = avail / n16;
    let mut widths = vec![each; n];
    widths[n - 1] = avail - each * (n16 - 1);
    widths
}

/// Open (or just focus) the viewer pane for `session`. Returns the pane id.
///
/// New panes split horizontally off the rightmost pane, then every non-rail
/// pane is resized to an equal share of the width right of the rail — a new
/// agent halves the space two ways, a third splits it three ways, and so on.
pub fn open_viewer(tmux: &Tmux, session: &str, agent_label: &str) -> anyhow::Result<String> {
    let panes = tmux.list_panes(WORKSPACE_SESSION)?;
    if let Some(p) = panes
        .iter()
        .find(|p| p.session_tag.as_deref() == Some(session))
    {
        tmux.select_pane(&p.id)?;
        return Ok(p.id.clone());
    }
    // Rightmost pane = last in index order (we always append on the right).
    let split_from = panes
        .last()
        .map(|p| p.id.clone())
        .unwrap_or_else(|| format!("{WORKSPACE_SESSION}:0"));
    let mut env: BTreeMap<String, String> = BTreeMap::new();
    env.insert("TMUX".into(), String::new()); // allow the nested client
    let pane = tmux.split_window(
        &split_from,
        true,
        &env,
        &viewer_command(tmux.socket(), session),
    )?;
    tmux.set_pane_option(&pane, "@cortado_session", session)?;
    tmux.set_pane_option(&pane, "@cortado_agent", agent_label)?;
    balance(tmux)?;
    tmux.select_pane(&pane)?;
    Ok(pane)
}

/// Re-balance the workspace: rail pinned to `RAIL_WIDTH`, every other pane
/// an equal share of the remaining width. The rail is the first pane in
/// index order (it is created with the session and never killed by cortado).
pub fn balance(tmux: &Tmux) -> anyhow::Result<()> {
    let panes = tmux.list_panes(WORKSPACE_SESSION)?;
    let Some((rail, others)) = panes.split_first() else {
        return Ok(());
    };
    if others.is_empty() {
        return Ok(());
    }
    let total = tmux.window_width(&format!("{WORKSPACE_SESSION}:0"))?;
    tmux.resize_pane_width(&rail.id, RAIL_WIDTH)?;
    let widths = viewer_widths(total, others.len());
    for (p, w) in others.iter().zip(widths) {
        tmux.resize_pane_width(&p.id, w)?;
    }
    Ok(())
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
    fn viewer_widths_share_the_space_right_of_the_rail_equally() {
        // 200 cols: rail 34 + 2 borders -> 164 for two viewers.
        assert_eq!(viewer_widths(200, 2), vec![82, 82]);
        // Rounding remainder goes to the last pane: 163 / 2 = 81 r 1.
        assert_eq!(viewer_widths(199, 2), vec![81, 82]);
        // One viewer takes everything right of the rail.
        assert_eq!(viewer_widths(120, 1), vec![85]);
        // Three viewers: 200 - 34 - 3 = 163 -> 54, 54, 55.
        assert_eq!(viewer_widths(200, 3), vec![54, 54, 55]);
        // Degenerate cases never return zero widths or panic.
        assert_eq!(viewer_widths(10, 2), vec![1, 1]);
        assert!(viewer_widths(80, 0).is_empty());
    }

    #[test]
    fn agent_session_options_disable_inner_prefix_and_status() {
        assert_eq!(
            AGENT_SESSION_OPTIONS,
            &[("prefix", "None"), ("status", "off")]
        );
    }

    #[test]
    fn pane_menu_is_scoped_to_tagged_agent_panes() {
        assert_eq!(PANE_MENU_BINDING[0], "if-shell");
        assert!(PANE_MENU_BINDING.contains(&"#{!=:#{@cortado_roster},1}"));
        assert!(PANE_MENU_BINDING
            .iter()
            .any(|arg| arg.contains("Restore pane,Zoom pane")
                && arg.contains("Split left / right")
                && arg.contains("Split top / bottom")
                && arg.contains("Stack viewers vertically")
                && arg.contains("Viewers side by side")
                && arg.contains("Close pane")
                && arg.contains("kill-pane -t =")));
    }
}
