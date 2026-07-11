# Single-window workspace (viewer panes over sessions)

**Date:** 2026-07-10
**Status:** approved

## Problem

Cortado currently uses one OS window per surface: `cortado ui` runs in the
user's own terminal, and every `cortado open` spawns a *separate* Ghostty
window attached to the agent's tmux session. On a single-screen machine this
means chasing windows, and it produced a real usability failure: tmux keys
(`C-b %` and friends) are dead in the UI window because the UI is a plain
ratatui app, not tmux — with no visual hint about which window is which.

Live diagnosis confirmed the tmux layer itself is healthy: the `cortado`
socket has default prefix and split bindings, attach works, and
`split-window` works against live agent sessions. The bug is the two-window
UX, not tmux.

## Goal

One Ghostty window, one tmux client, everything inside:

```
┌─ Ghostty ── tmux -L cortado attach -t cortado_workspace ──────────┐
│ ┌─ rail (cortado ui) ─┐ ┌─ viewer: scout_1 ───────────────────┐ │
│ │ Newsletter           │ │ Claude Code (nested attach)         │ │
│ │  ● scout        [2]  │ └─────────────────────────────────────┘ │
│ │ Web                  │ ┌─ viewer: fixer_1 ───────────────────┐ │
│ │  ○ fixer             │ │ Claude Code (nested attach)         │ │
│ └──────────────────────┘ └─────────────────────────────────────┘ │
│ [status: workspace │ panes named per agent          C-b ? help]  │
└────────────────────────────────────────────────────────────────────┘
```

Full native tmux everywhere: `C-b %` / `C-b "` split the workspace window,
zoom/resize/mouse all work, and the rail stays visible beside agents.

## Non-goals

- No change to the agent-session model (one detached tmux session per run on
  the `cortado` socket), naming, logs, or persistence semantics.
- No terminal emulation inside ratatui.
- The old one-window-per-agent behavior remains available
  (`launcher = "ghostty" | "env-terminal" | "print"`).

## Architecture

**The workspace is a viewer.** Agent sessions stay exactly as today. A new
tmux session `cortado_workspace` hosts:

- **Pane 0 (rail):** runs `cortado ui` (the existing ratatui app, unchanged
  keys), fixed width ~34 cols on the left.
- **Viewer panes:** one per opened agent, each running
  `TMUX= tmux -L cortado attach -t <agent-session>` — a nested tmux client on
  the same socket. Killing a viewer pane merely detaches a client; the agent
  session lives on. Killing the whole workspace session likewise never
  touches agents.

**Key routing:** agent sessions get `prefix None` and `status off` set at
spawn, so the *outer* (workspace) tmux owns every key and there is exactly
one status bar. `C-b %` splits the workspace window; a fresh split opens a
shell in the currently-selected agent's worktree when cortado creates it, or
tmux's default otherwise.

**Bootstrap:** `cortado ui` checks `$TMUX`/`$CORTADO_WORKSPACE`. Outside the
workspace it (1) ensures the workspace session exists via
`new-session -A -s cortado_workspace -- cortado ui` (env
`CORTADO_WORKSPACE=1`), (2) launches the configured terminal (Ghostty) once,
attached to it. Inside, it just runs the rail. Re-running `cortado ui`
reattaches to the same workspace (second Ghostty window = second view of the
same client group; tmux handles it).

**Opening agents:** `open_session()` keeps all current logic (session reuse,
bridge regeneration, env/keys, logging). Only the final launcher step
changes: with `launcher = "workspace"` the launcher

1. ensures the workspace session exists (creating it with the rail in pane 0
   if needed — covers `cortado open` from a bare shell),
2. if a viewer pane for that agent session already exists (matched by the
   pane user option `@cortado_session`), selects it instead of duplicating,
3. else `split-window -t cortado_workspace:0 -e TMUX= -- tmux -L cortado
   attach -t <session>`, tags the new pane with `@cortado_session` /
   `@cortado_agent`, re-applies the layout (`main-vertical` with the rail as
   the main pane, `main-pane-width 34`; agents stack in the right column and
   the user may rearrange freely afterwards), and selects it,
4. if no client is attached to the workspace, launches Ghostty attached to it
   (reusing the existing GhosttyLauncher/EnvTerminalLauncher).

**Workspace tmux options** (set on the workspace session at creation, socket
untouched for other sessions): `mouse on`, `status on`,
`pane-border-status top` with `#{@cortado_agent}` in `pane-border-format`
so every pane is labeled with its agent. Killing viewer panes never
destroys the session — the rail pane keeps it alive.

## Component changes

| Component | Change |
|---|---|
| `cortado-tmux` | new methods: `split_window(target, env, cmd) -> pane_id`, `select_pane`, `set_pane_option`, `list_panes(session) -> Vec<PaneInfo{id, options}>`, `set_session_options(session, &[(k,v)])`, `select_layout`. Spawn gains post-spawn session options (`prefix None`, `status off`) for agent sessions. |
| `cortado-term` | new `Launcher::Workspace` implementing the open-agent flow above; Ghostty/env-terminal reused for the one-time window launch. `attach_command` unchanged. |
| `cortado` CLI | `cortado ui`: bootstrap-into-workspace path. `cortado open/kill/sessions`: unchanged semantics. |
| `cortado-core` | `Launcher` enum gains `Workspace` (kebab-case `workspace`); `cortado init` writes it as the default. Existing configs keep whatever they set. |
| README | new Workspace section; keymap note that all tmux keys are native. |

## Error handling

- Workspace session vanished between check and use → one retry via
  `new-session -A`; surfaced in rail status line / CLI stderr after that
  (same error text both places, as today).
- Viewer pane's inner attach fails (agent session died) → tmux prints its
  error in the pane; the rail's stateless 2s refresh already shows truth.
- `cortado kill` with an open viewer → inner client dies with the session;
  the pane closes itself (`remain-on-exit` stays off).
- Non-Ghostty/`$TERMINAL` setups keep working: `launcher = "workspace"` only
  needs *some* terminal for the initial window; if none can be launched, the
  CLI prints the manual attach command (PrintLauncher fallback text).

## Testing

- **Unit (pure):** command-line builders for split/attach/options (same
  style as `ghostty_invocation` tests); `Launcher` config parsing including
  `workspace`; pane-option parsing.
- **Integration (real tmux, like `live_server.rs`):** create workspace →
  open agent → assert viewer pane exists, tagged, agent session has
  `prefix None`; kill viewer pane → agent session survives; open same agent
  twice → no duplicate pane; kill agent → pane disappears; killing workspace
  session leaves agent sessions running.
- **Manual smoke (macOS):** one Ghostty window; `C-b %` and `C-b "` split;
  mouse resize; rail Enter/n/x flows; detach + `cortado ui` reattach.

## Decisions log

- Viewer-workspace chosen over agents-as-panes restructure (breaks
  persistence model) and tabs-only via `link-window` (rail and agent not
  simultaneously visible). User picked viewer-workspace explicitly.
- `prefix None`/`status off` applied to **all** agent sessions at spawn:
  standalone Ghostty attach never needed the inner prefix either, and one
  consistent spawn path beats two.
- Default launcher becomes `workspace` for new installs (user approved the
  recommended flow); existing config files are never rewritten.
