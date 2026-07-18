# M6 — Herdr Substrate Swap

**Date:** 2026-07-18
**Status:** Approved design, pending implementation plan

## Summary

Replace cortado's tmux + Ghostty + workspace session layer with
[Herdr](https://herdr.dev) — a terminal workspace manager purpose-built for
AI coding agents, with a first-class CLI/socket API. Cortado keeps its data
model (`cortado-core`) and orchestration logic intact; only the
session/presentation substrate changes. This is a clean break: `cortado-tmux`
and `cortado-term` are deleted, and Herdr becomes a hard requirement.

Alongside the substrate swap, the philosophy of sessions changes:
**agents are ephemeral processes over persistent memory.** Cortado no longer
engineers for keeping agent processes alive across days. An agent at rest is
a folder and zero processes; continuity flows through its markdown memory,
not through a living terminal.

## Motivation

- Cortado hand-rolled on tmux exactly the plumbing Herdr ships natively:
  named agent panes, persistence across client disconnects, layout, labels.
- Herdr adds capabilities tmux cannot offer: agent status detection
  (idle / working / blocked), `agent send`/`read`/`wait`, notifications, and
  built-in integrations for Claude Code, Codex, and OpenCode.
- Idle tmux sessions burn RAM and context for no benefit. The memory files
  are the durable identity; the process never needed to be.
- Herdr's `agent wait` / `agent send` is the missing mechanical delegation
  layer that a future pipeline runner (M7) needs. The substrate swap lays
  that plumbing now.

## Decisions (from design interview)

| Question | Decision |
|---|---|
| What survives from cortado? | Data model **and** orchestration logic; only UI/session substrate is replaced |
| Herdr's role | Replaces tmux entirely as session layer (not nested on top) |
| TUI | Slim rail (teams/agents/memory) in a pinned Herdr pane; layout management dropped |
| Pipelines | Untouched in M6; become first-class (`cortado pipeline run`) in M7 |
| tmux/Ghostty fallback | None — clean break, Herdr is a hard requirement |
| Transport to Herdr | Shell out to the `herdr` CLI (Option A); no socket client unless M7 proves the need |
| Session philosophy | Ephemeral processes, persistent memory; materialize on demand, reflect, wind down |

## Non-goals

- No pipeline engine (that is M7; this milestone only ensures the plumbing —
  `send`/`read`/`wait_status` — exists in the wrapper crate).
- No changes to the `~/cortado/teams` data layout, memory formats,
  `models.toml`, or keyring handling.
- No Herdr socket-API client; the CLI is the sole transport.
- No Linux verification gate for this milestone (Herdr ships on Homebrew and
  Linux; macOS is the verified platform, matching M1–M5 practice).

## Architecture

### Crate changes

| Crate | Fate |
|---|---|
| `cortado-core` | Untouched (teams, agents, memory, bridge, models, keyring, session names) |
| `cortado-git` | Untouched |
| `cortado-tmux` | **Deleted** |
| `cortado-term` | **Deleted** (Ghostty / env-terminal / print / workspace launchers all removed) |
| `cortado-herdr` | **New** — thin typed wrapper shelling out to the `herdr` binary |
| `cortado` (CLI + TUI) | Commands rewired to `cortado-herdr`; TUI slimmed to the rail |

### `cortado-herdr` public surface

Mirrors the shape of today's `Tmux` struct: a `Herdr` handle plus a
`HerdrError` enum. All functions are argv-construction + one `Command`
invocation; no state, no caching.

```rust
pub struct Herdr { /* path to binary, workspace label */ }

pub enum HerdrError { NotInstalled, ServerDown(String), Failed { stderr: String, .. }, Parse(String) }

impl Herdr {
    // M6 consumers
    pub fn ensure_server(&self) -> Result<()>;               // start headless if down
    pub fn ensure_workspace(&self) -> Result<WorkspaceId>;   // "Cortado" workspace, create if missing
    pub fn start(&self, name, cwd, env, argv, ws) -> Result<()>;  // herdr agent start
    pub fn list(&self) -> Result<Vec<AgentInfo>>;            // herdr agent list (name, status, pane)
    pub fn focus(&self, name) -> Result<()>;                 // herdr agent focus
    pub fn close(&self, name) -> Result<()>;                 // herdr pane close <pane of agent>
    // M7 plumbing, built now, unused by M6 commands
    pub fn send(&self, name, text) -> Result<()>;            // herdr agent send
    pub fn read(&self, name, lines) -> Result<String>;       // herdr agent read
    pub fn wait_status(&self, name, status, timeout) -> Result<()>; // herdr agent wait
}
```

`AgentInfo` carries the agent name, Herdr pane id, and status
(`idle | working | blocked | unknown`). Session names keep today's
`cortado_<team>_<agent>_<n>` encoding and become Herdr agent names, so
`SessionName::parse` and `next_free` are reused unchanged.

### Command mapping

| Today | Becomes |
|---|---|
| `tmux.spawn(name, worktree, env, cmd)` | `herdr agent start <name> --cwd <worktree> --workspace <ws> --split right --env K=V … -- <cmd>` |
| `cortado sessions` (tmux list) | `herdr agent list`, filtered to `cortado_*` names; now shows live status |
| `cortado open` reattach | `herdr agent focus <name>` |
| `cortado kill <target>` | `herdr pane close` for each matching agent pane |
| Ghostty window / workspace viewer panes | Gone — Herdr *is* the window |

Everything upstream of the spawn is unchanged: bridge regeneration before
every spawn, model env resolution (keyring / `CORTADO_KEY_*`), the
`CORTADO_TEAM/AGENT/MODEL/SESSION` env vars, and `logs/sessions.jsonl`
append-only history.

## Session lifecycle (ephemeral by default)

- **Dormant** (steady state): an agent is a folder. Zero processes.
- **`cortado open <agent>`:** regenerate bridge → resolve model env → if a
  live pane matches and `--new` was not passed, `focus` it; otherwise
  `start` a fresh pane with memory loaded. `--new` spawns a parallel
  session exactly as today.
- **Working:** the pane lives in the Herdr server; closing the Herdr client
  detaches, it does not kill. Equivalent guarantee to tmux: only closing
  the pane, killing the server, or reboot ends a session.
- **Wind-down:** when the runtime exits (user quits Claude Code, or a
  future M7 runner finishes a stage), the pane closes and the agent returns
  to dormant. The M2 Stop-hook reflection is the continuity mechanism and
  is now load-bearing: session-end memory write-back is a first-class
  guarantee, not a nice-to-have. M6 must verify the Stop hook fires
  correctly inside Herdr panes.
- **`cortado kill`** remains for force-closing live panes, but is no longer
  the routine way sessions end.

## Workspace & slim TUI

On first `cortado open` or `cortado ui`, cortado ensures the Herdr server is
running and a workspace labeled **"Cortado"** exists. The rail runs as that
workspace's pinned left pane (~34-column ratio); agents spawn as panes to
its right.

The rail keeps: the teams → agents tree, Sessions and Memory tabs, `Enter`
(open/focus), `n` (model picker), `N` (new-agent form), `x` (kill with
confirm), `q`/`Esc`, and the status line. Badges upgrade from live-count to
live **status** (idle / working / blocked) sourced from `herdr agent list`
on the same 2-second stateless tick.

The rail **drops**: viewer-pane management, width balancing, zoom
keybindings, the right-click pane menu, and scratch shells. Herdr owns
layout natively; users may move agent panes to other tabs/workspaces with
Herdr's own keys and cortado will not care — it finds agents by name, not
position. Every rail action continues to call the same library function its
CLI verb uses; no TUI-only code paths.

## Configuration & doctor

- `[tmux]` and `[terminal]` config sections retire. Unknown/obsolete keys
  produce a one-time warning, never an error.
- New optional `[herdr]` section: `binary = "herdr"` (path override),
  `workspace = "Cortado"` (label override). Defaults mean an empty config
  keeps working.
- `cortado doctor` drops tmux/Ghostty checks; adds: `herdr` on PATH,
  minimum version (pin at implementation time to the tested release, e.g.
  ≥ 0.7.4), and server reachable-or-startable. Git and runtime checks
  unchanged.
- `cortado init` no longer scaffolds tmux-related config.

## Migration

- **Data:** none. `~/cortado/teams`, memory, `models.toml`, keyring entries
  are untouched.
- **Live tmux sessions:** become invisible to cortado after upgrade.
  Release note: finish them out, then `tmux -L cortado kill-server`.
- **`.git/info/exclude` blocks** for worktree agents are unaffected (owned
  by `cortado-git`).
- **Docs:** README requirements table (tmux/Ghostty → Herdr), install
  steps, workspace section, and config reference all updated in this
  milestone.

## Error handling

- `HerdrError` mirrors today's `TmuxError` pattern: non-zero exits surface
  Herdr's stderr verbatim in CLI output and the rail status line. No
  retries, no fallbacks, no silent recovery.
- `NotInstalled` and `ServerDown` produce actionable messages pointing at
  `cortado doctor` / `brew install herdr`.
- `ensure_server` is the one place cortado starts a process it does not own
  (matching tmux's implicit server auto-start); failure to start is a hard,
  clearly-reported error.

## Testing

- **Unit:** argv construction and `herdr agent list` output parsing in
  `cortado-herdr` are pure functions, tested without Herdr installed.
- **Integration** (gated on `herdr` binary present, same pattern as the
  real-tmux tests): boot a headless server against an isolated config dir,
  start a dummy `sh` agent, assert list/focus/close round-trips, tear down.
- **TUI:** existing stateless render tests adapted to the slim rail and
  status badges.
- **E2E manual (macOS):** new team → new agent → open → verify Stop-hook
  reflection on exit → close Herdr client → reopen → dormant/live states
  correct in rail.

## Licensing note

Herdr is AGPL-3.0; cortado invokes it strictly as a separate process via
its public CLI — no linking, no code sharing. Cortado's MIT/Apache-2.0 dual
license is unaffected.

## Out of scope, queued next (M7 preview)

First-class pipelines: `pipeline.toml` per team and
`cortado pipeline run/status/resume`, sequencing stage agents via the
`send`/`read`/`wait_status` plumbing this milestone adds. The orchestrator
agent's QA-gate role and the `pipeline/<slug>/` artifact convention are
preserved; cortado mechanizes the baton-passing that is manual today —
one stage's process alive at a time. M7 gets its own design interview and
spec once M6 ships.
