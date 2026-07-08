# Damon — Design Spec

**Date:** 2026-07-07
**Status:** Approved (design review in conversation, 2026-07-07)

## What

Damon is a Rust orchestrator for persistent coding agents, in the spirit of
[damon-ade](https://github.com/per-simmons/damon-ade) but rebuilt around three
substitutions: **plain files instead of a database, tmux instead of an embedded
terminal host, and external Ghostty instead of an Electron shell.** It runs
identically on macOS and Omarchy (Arch) Linux.

Each agent is a durable identity — a name, a runtime CLI, its own git
worktree, and self-curated markdown memory. Damon manages the folders,
regenerates per-runtime bridge files, injects model/provider environment, and
spawns sessions as tmux sessions opened in Ghostty windows. Damon is
inspired by damon-ade's concepts but shares no code with it, so its
Elastic License does not bind this project.

### Decisions (settled in design review)

| Decision | Choice |
|---|---|
| v1 interface | CLI **and** ratatui TUI from day one |
| Data root | Visible `~/damon` (config in `~/.config/damon`) |
| Repo sources | All three: new, clone, worktree-of-existing |
| Runtimes | Full parity: Claude Code, Codex, OpenCode + OpenRouter models |
| State model | Stateless — filesystem + tmux are the only sources of truth |
| Session persistence | tmux on a dedicated socket (`tmux -L damon`) |
| Terminal | Ghostty via CLI launch; `$TERMINAL` fallback; no embedding |

### Non-goals (v1)

- No daemon, no SQLite, no sync service, no cloud components.
- No embedded terminal rendering (libghostty may slot behind the
  `TerminalLauncher` trait later).
- No control of Ghostty after launch (Ghostty has no remote-control API);
  damon tracks tmux sessions, not terminal windows.
- No avatars/photos; identity is textual.
- Survives window close and logout via tmux; does not survive reboot
  (sessions honestly reported gone afterward).

## Filesystem schema

```
~/damon/                              # data root (overridable in config.toml)
  teams/
    <team-slug>/
      team.toml
      agents/
        <agent-slug>/
          agent.toml
          memory/                     # canonical memory — never inside the worktree
            AGENT.md                  # identity & operating brief
            USER.md                   # profile of the user
            MEMORY.md                 # agent's notes + index of topic files
            skills/<skill-name>/SKILL.md
          worktree/                   # the agent's git worktree (session cwd)
          logs/
            sessions.jsonl            # append-only session history

~/.config/damon/
  config.toml                         # global settings
  models.toml                         # model registry (user-editable)
```

- **Slugs:** kebab-case, `^[a-z0-9][a-z0-9-]{0,31}$`, derived from display
  names (`"Newsletter Team"` → `newsletter-team`). Collisions rejected at
  create time.
- **Existence = validity:** an agent exists iff its folder and a parseable
  `agent.toml` exist. No registration step; entities can be created or
  repaired with a text editor. Damon commands validate and report, never
  silently rewrite, files they didn't just generate.
- Memory lives outside the worktree so branch/worktree churn can't touch it
  (same rationale as damon-ade / Hermes).

## File schemas

### `team.toml`

```toml
name = "Newsletter"                 # display name
created = "2026-07-07T18:00:00Z"    # ISO-8601 UTC
```

### `agent.toml`

```toml
[agent]
name = "Scout"                          # display name (required)
role = "Researches newsletter topics"   # optional; seeds AGENT.md at scaffold
runtime = "claude"                      # claude | codex | opencode
default_model = "claude"                # key into models.toml

[repo]
source = "worktree"                     # new | clone | worktree
# source = "new":     no other keys; damon git-inits worktree/
# source = "clone":   url  = "git@github.com:acme/site.git"
# source = "worktree": path = "~/Projects/site"   (existing local repo)
branch = "agent/scout"                  # branch damon creates/checks out
```

For `source = "worktree"`, damon runs `git worktree add` in the target repo,
placing the linked worktree at the agent's `worktree/` on branch `branch`.
Removing the agent (`damon agent rm`) runs `git worktree remove` and prompts
before deleting an unmerged branch.

### `config.toml`

```toml
[general]
root = "~/damon"                    # data root
default_runtime = "claude"

[tmux]
socket = "damon"                    # tmux -L damon; isolates from user tmux

[terminal]
launcher = "ghostty"                # ghostty | env-terminal | print
```

### `models.toml` — the model bar as data

```toml
[models.claude]
label = "Claude"
runtime = "claude"                  # no env: CLI uses its own login

[models.gpt]
label = "GPT-5.5"
runtime = "codex"

[models.kimi]
label = "Kimi K2.7"
runtime = "claude"                  # Claude Code pointed at OpenRouter
env = { ANTHROPIC_BASE_URL = "https://openrouter.ai/api/v1",
        ANTHROPIC_AUTH_TOKEN = "${keyring:openrouter}",
        ANTHROPIC_MODEL = "moonshotai/kimi-k2.7" }

# minimax, glm: same shape, different ANTHROPIC_MODEL
```

- `${keyring:<provider>}` placeholders resolve **at spawn time** from the OS
  keyring (macOS Keychain / Secret Service on Linux) via the `keyring` crate.
  Service `damon`, account `<provider>`. Set with `damon key set openrouter`
  (prompts, no echo). Plaintext never touches disk and is injected only into
  the spawned tmux session's environment.
- Adding a model = adding a TOML table. Damon ships the defaults above on
  `damon init`; the user owns the file afterward.

### `logs/sessions.jsonl`

One JSON object per line, append-only, per agent:

```json
{"ts":"2026-07-07T18:12:03Z","event":"spawn","session":"damon_newsletter_scout_1","model":"kimi","runtime":"claude"}
{"ts":"2026-07-07T19:02:41Z","event":"kill","session":"damon_newsletter_scout_1"}
```

History/reporting only; never read to determine liveness (tmux is liveness).

## Session layer: tmux

- Dedicated server: every command uses `tmux -L damon …`, isolating damon
  from any personal tmux configuration or sessions.
- **Naming:** `damon_<team-slug>_<agent-slug>_<n>` where `n` is the lowest
  free positive integer for that agent. tmux forbids `:` and `.` in names;
  slug charset already excludes both. Slug charset excludes `_`, so parsing
  names back apart on `_` is unambiguous.
- **Spawn:** `tmux -L damon new-session -d -s <name> -c <worktree>` with
  environment: resolved model `env` map, `DAMON_TEAM`, `DAMON_AGENT`,
  `DAMON_MODEL`, `DAMON_SESSION`; the session command is the runtime
  adapter's launch command. Bridge files are regenerated immediately before
  every spawn.
- **List/kill:** `list-sessions -F` parsed by prefix; `kill-session -t`.
- Windows closing, Ghostty quitting, or the TUI exiting never terminates a
  session; only `damon kill` / `tmux kill-session` does.

## Terminal layer: Ghostty

```rust
trait TerminalLauncher {
    /// Open a terminal window attached to the given tmux session.
    fn open(&self, session: &str, title: &str) -> Result<()>;
}
```

- `GhosttyLauncher` — macOS: `open -na Ghostty --args -e tmux -L damon attach -t <session>`;
  Linux: `ghostty -e tmux -L damon attach -t <session>` (detached child).
- `EnvTerminalLauncher` — uses `$TERMINAL -e …` (Omarchy sets `$TERMINAL`).
- `PrintLauncher` — prints the attach command (headless/SSH fallback).

Launch is fire-and-forget by design. A future libghostty embedding or a
Ghostty remote-control API (if one ships) would implement this same trait.

## Runtime adapters

```rust
trait Runtime {
    fn id(&self) -> &'static str;                       // "claude" | "codex" | "opencode"
    fn cli_binary(&self) -> &'static str;               // for doctor checks
    fn launch_command(&self, model: &Model) -> Vec<String>;
    fn write_bridge_files(&self, agent: &Agent) -> Result<()>;
}
```

Bridge files are generated **into the worktree** from canonical memory and
added to `.git/info/exclude` (keeps generated files out of status without
touching the repo's tracked `.gitignore`):

- **Claude Code:** `CLAUDE.md` importing the four memory surfaces
  (`@<memory>/AGENT.md` etc.), plus `.claude/settings.json` wiring a **Stop
  hook** that triggers the session-end reflection (review conversation →
  update memory/skills), mirroring damon-ade's enforced write-back.
- **Codex:** regenerated `AGENTS.md` embedding memory content (Codex has no
  import mechanism), including the write-back protocol as instructions.
- **OpenCode:** `opencode.json` + instructions file, same embedding approach.

Memory templates (seeded at `agent new`): AGENT.md from name+role, USER.md
skeleton, MEMORY.md with the write-back protocol (when to save, when to skip,
consolidate over append — adapted in concept from Hermes/damon-ade docs).

## CLI surface

```
damon init                                  # scaffold ~/damon + config + models.toml
damon doctor                                # check git, tmux, ghostty, runtime CLIs, keyring
damon team new <name> | ls | rm <team>
damon agent new <team>/<name> [--runtime R] [--role STR]
      (--repo-new | --repo-clone URL | --repo-worktree PATH) [--branch B]
damon agent ls [team] | rm <team>/<agent>
damon open <team>/<agent> [--model M] [--new] # spawn or reattach; opens terminal
damon sessions                               # live sessions (from tmux) across agents
damon kill <session-name | team/agent>       # kill one session or all of an agent's
damon memory <team>/<agent> [FILE]           # print memory file(s); --edit opens $EDITOR
damon key set <provider> | rm <provider>
damon ui                                     # launch the TUI
```

`damon open` with no live session spawns one on `default_model`; with live
sessions and no `--new`, reattaches the most recent. Agent references accept
unambiguous bare agent slugs (`damon open scout`).

## TUI (ratatui)

- **Left pane:** teams → agents tree, each agent badged with live-session
  count (green) — the ADE rail, textual.
- **Right pane, tabs:** *Sessions* (name, model, uptime) and *Memory*
  (file list + preview of AGENT/USER/MEMORY/skills).
- **Keys:** `n` new session (model-picker popup from models.toml), `Enter`
  open/attach selected in Ghostty, `x` kill (confirm), `m` memory tab,
  `N` new agent (form popup), `q` quit.
- Holds no state: every refresh (2s tick + after any action) re-derives from
  the filesystem and `tmux list-sessions`. TUI actions call the same library
  functions as the CLI verbs.

## Rust workspace

```
damon/  (this repo, cargo workspace)
  crates/damon-core    # domain types, TOML schemas, slugs, memory, bridge generation
  crates/damon-git     # repo sources: init / clone / git worktree (shells out to git)
  crates/damon-tmux    # tmux -L damon wrapper: spawn, list, kill, env injection
  crates/damon-term    # TerminalLauncher trait + Ghostty/EnvTerminal/Print impls
  crates/damon         # binary: clap CLI, ratatui TUI, keyring integration
```

Key dependencies: `clap`, `ratatui` + `crossterm`, `serde` + `toml`,
`keyring`, `thiserror` (libraries) / `anyhow` (binary), `serde_json`.
Git and tmux are driven via `std::process::Command` — no libgit2 in v1
(damon-ade shells out too, via simple-git).

## Error handling

- Libraries return typed errors (`thiserror`); the binary renders them with
  context (`anyhow`) and a next-step hint.
- `damon doctor` checks each external dependency and prints per-OS install
  hints (brew / pacman). Every command that needs a missing dependency fails
  with the same hint, not a raw exec error. (At design time, tmux and Ghostty
  are not yet installed on the primary macOS machine — doctor is the first
  thing built for a reason.)
- Unparseable TOML anywhere under `~/damon` → the entity is listed as
  *invalid* with the parse error and path; never skipped silently, never
  auto-rewritten.
- Spawn failures kill the half-created tmux session before reporting.

## Testing

- **Unit (damon-core):** TOML schema round-trips, slug derivation/validation,
  session-name encode/parse, bridge-file generation against golden files,
  `${keyring:…}` resolution with a mock resolver.
- **Integration (damon-tmux, damon-git):** drive a real tmux on a scratch
  socket (`-L damon-test-<pid>`); real git repos in tempdirs for all three
  repo sources. tmux + git are available in macOS and Linux CI runners.
- **TUI:** ratatui `TestBackend` snapshot tests over fixture filesystems.
- **Not CI-testable:** actual Ghostty launching — covered by the trait's
  `PrintLauncher` in tests and by `damon doctor` at runtime.

## Build order

1. **M0** — workspace scaffold; `damon-core` schemas + slugs; `init`, `doctor`.
2. **M1** — `damon-git` (three sources) + `damon-tmux`; `team`/`agent` CRUD;
   `open`/`sessions`/`kill` with Claude runtime only; Ghostty launcher.
3. **M2** — models.toml registry, keyring, OpenRouter models; Codex +
   OpenCode adapters; bridge generation for all three; reflection Stop hook.
4. **M3** — ratatui TUI.
5. **M4** — polish: `memory --edit`, doctor hints, packaging (brew formula /
   AUR later).

Each milestone lands green (fmt, clippy, tests) before the next begins.
