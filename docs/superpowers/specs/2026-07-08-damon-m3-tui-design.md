# Damon M3 — Parked Debt + ratatui TUI — Design Spec

**Date:** 2026-07-08
**Status:** APPROVED (design review 2026-07-08)
**Parent spec:** `2026-07-07-damon-orchestrator-design.md` (body TUI section +
as-built addendum). Where this document is silent, the parent spec and its
addendum govern.

## Scope

M3 has two phases, in order:

1. **Phase 0 — parked debt:** the five items the M1+M2 as-built addendum
   triaged to M3. Lands green (fmt, clippy, tests) before TUI work begins.
2. **Phase 1 — ratatui TUI:** the parent spec's TUI section, realized as a
   synchronous Elm-style module inside the `damon` binary crate.

Out of scope (parked to M4 per parent spec): `memory --edit`, doctor's
string-driven tmux gate, shared `info/exclude` multi-worktree concern,
`damon memory` CLI command, packaging.

## Phase 0 — parked debt

In order of risk:

1. **Atomic bridge writes.** `CLAUDE.md` and `.claude/settings.json` are each
   written via temp-file-then-rename in the destination directory. The *pair*
   remains non-transactional by design — bridges regenerate before every
   spawn, so a torn pair heals on next `open`. Each individual file must
   never be observable half-written.
2. **No silent hook disable.** When `write_bridges` skips the Stop hook
   because `damon_exe` contains whitespace (or other shell metacharacters),
   it returns a warning alongside the written paths; `open` prints it. The
   skip behavior itself is unchanged — this is an honesty fix, not a
   shell-quoting adventure.
3. **`slug_dirs` partial-failure edge.** Per-entry `read_dir` errors are no
   longer flattened away; they surface through the strays/invalid reporting
   path (never silently hidden, matching the parent spec's validity rules).
4. **`Slug::parse` trailing dash.** Validation tightened to reject a trailing
   `-`. `Slug::derive` never emits one, so no compatibility break; add a
   regression test.
5. **Codex whitespace-path bridge test.** Test-only: `embedded_bridge` output
   for a memory path containing whitespace (content is embedded, so this
   must already work — the test pins it).

## Phase 1 — TUI

### Architecture (decided in design review)

Synchronous Elm-style module in the binary crate. No new crates, no async
runtime. New workspace dependencies: `ratatui`, `crossterm`.

```
crates/damon/src/tui/
  mod.rs        # `damon ui` entry; terminal setup/teardown; main loop
  app.rs        # Model (UI state) + update(Model, Event) -> Action dispatch
  view.rs       # pure rendering: (Model, Snapshot) -> frames
  event.rs      # crossterm event poll with 2s tick timeout
  snapshot.rs   # Snapshot::build — world state from Store + session list
  popup.rs      # popup enum stack: model picker, kill confirm, new-agent form
```

Terminal handling: raw mode + alternate screen on entry; restored on exit
**and via a panic hook**, so a TUI crash never leaves the shell mangled.
`damon ui` is the explicit entry point (no default-to-TUI).

### Statelessness — the Snapshot

`Snapshot` is rebuilt from scratch on every 2s tick and immediately after any
action:

- `Store` load: teams, agents, invalid entities, strays. Invalid entities
  render as `INVALID` lines exactly as `team ls` prints them.
- `tmux -L damon list-sessions`: parsed session names → per-agent live
  session lists; uptime derived from `#{session_created}`.

`Snapshot::build` takes the session list **as a parameter** (the caller
queries tmux), so view and update tests never need tmux.

The Model holds only UI state:

- rail selection, keyed by `(team-slug, agent-slug)` — never by index, so a
  refresh cannot jump the cursor;
- active right-pane tab (Sessions | Memory) and memory-preview scroll state;
- popup stack;
- status-line message (info or error text).

Nothing else persists between frames. Damon files are never written by the
TUI except through the same library calls the CLI verbs use.

### Layout

- **Left pane (rail):** teams → agents tree; each agent badged with its live
  session count (green when > 0).
- **Right pane, tabs:** *Sessions* — name, model, uptime for the selected
  agent; *Memory* — file list + scrollable preview.
- **Status line:** last action result or error; transient.

### Keys and actions

Every action calls the same library function as its CLI verb — no parallel
code paths. Actions run inline on the event thread (all are fast
shell-outs); the snapshot rebuilds immediately after.

| Key | Action |
|---|---|
| `↑/↓` / `j/k` | navigate the rail |
| `Tab` | toggle Sessions / Memory tab |
| `m` | jump to Memory tab |
| `n` | model-picker popup (entries from models.toml) → spawn new session (same path as `damon open --model M --new`) → open terminal |
| `Enter` | open/attach selected agent (same as `damon open`: reattach highest-n live session, else spawn on default model); in Memory tab: preview selected file |
| `x` | kill with confirm popup; multiple sessions → kill all (same as `damon kill team/agent`), partial failures reported in status line |
| `N` | new-agent form popup: team (preselected from rail), name, runtime, role, repo source (new / clone URL / worktree path), branch → same function as `damon agent new` |
| `q` / `Esc` | `Esc` closes the top popup; `q` quits the TUI — sessions keep running |

Action errors render in the status line using the CLI's error text; the TUI
never exits on a failed action.

### Memory tab

Read-only in M3. File list: `AGENT.md`, `USER.md`, `MEMORY.md`,
`skills/*/SKILL.md` for the selected agent. `Enter` opens a scrollable
preview (`↑/↓`, `PgUp/PgDn`); `Esc` returns to the list.

### Error handling

- Snapshot build failures (unreadable root, tmux exec failure) render as a
  full-pane error state with the doctor-style hint; the TUI stays up and
  retries on the next tick.
- Keyring/model resolution errors on spawn surface in the status line with
  the same message the CLI prints.

## Testing

- **Update-function unit tests:** synthetic key events against fixture
  Model + Snapshot; assert state transitions and dispatched actions. Actions
  sit behind a thin dispatch seam so tests assert *which* library call would
  fire without executing it.
- **View snapshot tests:** ratatui `TestBackend` over fixture filesystems
  (tempdir `DAMON_ROOT`) with injected session lists.
- **One integration test:** real scratch-socket tmux (existing per-test
  socket + Drop-guard convention) → real Snapshot → assert badges and
  uptime render.
- Ghostty remains untestable in CI; `PrintLauncher` covers the trait in
  tests, as in M1/M2.

## Milestone exit

- Lands green per existing convention: `cargo fmt --check`, `cargo clippy`,
  all tests passing.
- Parent spec's as-built addendum updated with M3 deltas.
- README gains a TUI section with the keys table.
