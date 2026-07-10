# Damon — Design Spec

**Date:** 2026-07-07
**Status:** M1 + M2 SHIPPED (2026-07-08). This document is the approved design
plus the **As-built addendum** at the end, which records every divergence
between this spec and the shipped code. Read the addendum before planning M3 —
where it conflicts with the body, the addendum (i.e. the code) wins.

> **Renamed:** the project was renamed `damon` → **Cortado** in M8 (2026-07-10).
> Identifiers throughout this historical document reflect the pre-rename name.

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

---

## As-built addendum (M1 + M2 + M3, 2026-07-08)

Authoritative deltas between the approved design above and the shipped code.
Where this section conflicts with the body, this section wins.

### Layout & config (changed from body)

- **Config dir is `~/.config/damon` on EVERY OS** — the body's
  "`dirs::config_dir()`" parenthetical was dropped (it resolves to
  `~/Library/Application Support` on macOS). `default_config_dir(home)` is a
  pure function = `<home>/.config/damon`. Overrides: `$DAMON_CONFIG_DIR`,
  `$DAMON_ROOT` (env, mainly for tests).
- **No panics on unresolvable home** — `expand_tilde`, `Config::config_dir`,
  `Config::root` return `CoreError::NoHome` instead of `.expect()`.
- **Stray directories are reported** — non-slug-named dirs under `teams/` or
  any `agents/` surface via `Store::strays()` and print in `team ls` as
  `INVALID NAME`, never silently hidden.

### Models & keys (M2, replaces the body's models.toml sketch)

- Shipped registry keys: `claude`, `gpt` (codex), `gpt_openrouter` (codex via
  OpenRouter, `OPENAI_*` env), `kimi`, `minimax`, `glm` (claude runtime via
  OpenRouter, `ANTHROPIC_*` env), `opencode`.
- Env value resolution at spawn (whole-value placeholders only):
  `${keyring:<account>}` → (1) `DAMON_KEY_<ACCOUNT>` env (uppercase, `-`/`.`→`_`)
  then (2) OS keyring service `"damon"`, account `<account>`; `${VAR}` → damon's
  own environment; anything else literal. Empty `${keyring:}` account → error.
  `DAMON_NO_KEYRING` (non-empty) disables all OS-keychain access (key commands
  fail cleanly; keyring models fall to the missing-key error).
- `damon key set|rm <provider>`: rpassword hidden prompt on a TTY, reads stdin
  when piped; keyring v2 `Entry::new("damon", provider)` (note: returns
  `Result`).
- **Threat model (documented in README):** resolved key goes keychain → memory
  → `tmux -e` → session process environment for the session's lifetime;
  same-user-only visibility; never written to disk/logs by damon. tmux error
  strings redact `-e` values as `KEY=***` (`display_args` in damon-tmux).

### Runtimes & bridges (M2, replaces the body's Runtime-adapter sketch)

- No `Runtime` trait shipped — `RuntimeId` enum + exhaustive matches proved
  sufficient. `RuntimeId::binary()` honors `DAMON_BIN_<RT>`; open honors
  `DAMON_<RT>_ARGS` (test seams).
- `write_bridges(runtime, agent_name, memory_dir, worktree, damon_exe)`:
  - Claude → `CLAUDE.md` (absolute `@` imports; memory path must be
    whitespace-free — validated) **+ `.claude/settings.json`** with a Stop
    hook running `<damon_exe> hook reflect` (serde_json-built; skipped when
    `damon_exe` contains whitespace — hook is enhancement, not correctness).
  - Codex & OpenCode → `AGENTS.md` via shared `embedded_bridge(label, …)`
    (memory content EMBEDDED — no import mechanism; write-back protocol
    included textually).
  - Returned paths drive git-exclusion (`.git/info/exclude` in the common dir).
- `damon hook reflect` (hidden subcommand): stdin hook JSON;
  `stop_hook_active:true` → exit 0; else reflection instruction on stderr +
  exit 2 (blocks the stop exactly once; garbage stdin fails toward reflecting).
- `agent new` default_model follows runtime: claude→`claude`, codex→`gpt`,
  opencode→`opencode` (registry keys).

### Misc deltas

- `kill <agent>` continues past per-session failures, then reports
  `killed N, failed M: …`.
- `GitError::Io{path,source}` for fs failures (Spawn reserved for exec).
- `worktree_add` probes `rev-parse --verify refs/heads/<branch>` and
  deterministically attaches vs creates (no error-masking fallback).
- Reattach picks highest session `n` numerically (`max_by_key` on parsed n).
- Test conventions: real tmux on per-test scratch sockets with Drop-guard
  teardown; real git in tempdirs with `GIT_CONFIG_GLOBAL/SYSTEM=/dev/null`;
  keychain never touched by tests (`#[ignore]` for the one real round-trip).

### M3 — Phase 0 deltas (parked debt cleared)

- **`BridgeOutput` replaced the bare `Vec<PathBuf>`** return from
  `write_bridges` — a struct carrying the written paths plus an optional
  warning string, so a silently-skipped Stop hook (shell-metachar
  `damon_exe`) surfaces to `open` instead of vanishing.
- **`write_atomic`** (damon-core): temp-file-then-rename within the
  destination directory, used for both `CLAUDE.md` and
  `.claude/settings.json`. Each file is individually atomic; the *pair*
  remains non-transactional by design (bridges regenerate on every spawn, so
  a torn pair heals on next `open`), per the M3 spec.
- **`classify_entries`** (damon-core `store.rs`): centralizes the
  read-dir-entry classification used by `slug_dirs` — an unreadable entry
  becomes a stray (`<unreadable entry: {e}>`) instead of being flattened
  away by `filter_map`. Never silently hidden, matching the parent spec's
  validity rules.
- **`Slug::parse` rejects a trailing `-`.** `Slug::derive` never emitted one,
  so this closes a validation gap without a compatibility break; regression
  test added.

### M3 — TUI as shipped

- Module layout landed exactly as designed:
  `crates/damon/src/tui/{mod,app,view,event,snapshot,popup}.rs`.
- **`Option<Popup>` on the Model, not a popup stack.** At most one popup is
  ever open at a time in practice, so the spec's "popup stack" language
  simplified to `pub popup: Option<Popup>` on `app::Model`; `Esc` clears it.
- **`j`/`k` are tab-dependent**, not a blanket rail-navigation alias: on the
  Sessions tab `j`/`k` behave like `↑/↓` (rail navigation); on the Memory
  tab `j`/`k` move the memory-preview cursor while `↑/↓` still navigate the
  rail. This is a shipped UX rule, not a bug — it lets you scroll a memory
  preview without leaving the rail's key model. Documented in the README
  keys table.
- **`damon-tmux::Tmux::list_info`** uses `|`, not `\t`, as the
  `list-sessions -F` field separator (`#{session_name}|#{session_created}`):
  tmux 3.7b silently rewrites embedded tab bytes in `-F` output to `_`,
  independently reproduced with `tmux -F $'...\t...'` bypassing shell
  quoting — indistinguishable from underscores already used in session
  names (e.g. `damon_team_agent_1`). `|` passes through unmodified. One
  consequence, documented in code rather than worked around: a foreign
  (non-damon) tmux session whose name contains `|` is silently dropped from
  the parsed list; damon-generated names never contain `|`.
- **`damon-tmux::Tmux::env_var(session, var)`** added alongside `list_info`
  — reads one variable via `show-environment -t <session> <var>`, `None` on
  tmux's "unknown variable" exit. The Sessions tab reads the per-session
  model via `env_var(session, "DAMON_MODEL")` (set at spawn); unknown → the
  tab renders `"?"` rather than blocking on the lookup.
- **Command cores are print-free.** The CLI verbs (`team`, `agent`,
  `open`, `sessions`, `kill`) were split into a library-callable core that
  returns data/`Result` and a thin CLI wrapper that prints it, so the TUI
  calls the identical core functions the CLI verbs use — no parallel logic
  path, matching the spec's statelessness requirement.
- **`AgentRow` has no `team` field.** It was cut as genuinely dead once the
  event loop landed — every lookup path threads through `TeamRow.slug`
  rather than needing the team on the agent row itself.
- **Stray directories in the rail** render as non-selectable red lines:
  `{context}: INVALID NAME {name:?}` — Debug-quoting on the name is
  deliberate, kept specifically to disambiguate whitespace or other
  non-printing characters in a bad directory name.
- **Unreadable directory entries** surface as stray names of the form
  `<unreadable entry: {error}>`, produced by `classify_entries` (see Phase 0
  above) and rendered through the same stray-line path.
- **New-agent form validation messages**, verbatim: `"agent name is
  required"`, `"clone URL is required for source = clone"`, `"repo path is
  required for source = worktree"`. Role and branch are optional in the
  form; left empty they fall back to the same defaults `damon agent new`
  uses on the CLI.
- **Zero clippy warnings workspace-wide** as of Task 14 (event-loop wiring).
  Transient `dead_code` allowances during Tasks 9–13, while the TUI was
  being assembled module-by-module, were accepted as normal mid-milestone
  noise and are gone by the milestone gate.

### M4 — as shipped (2026-07-08)

Five areas, per `docs/superpowers/specs/2026-07-08-damon-m4-design.md`:
parked-debt fixes, doctor's structured checks, `info/exclude` block +
cleanup, `damon memory`, Homebrew packaging.

- **`damon-git` `KNOWN_PATTERNS` filter closure.** The plan's sketch
  compiled as written via auto-deref: `KNOWN_PATTERNS: [&str; 3] =
  ["CLAUDE.md", "AGENTS.md", ".claude/settings.json"]`,
  `.filter(|l| !KNOWN_PATTERNS.contains(&l.trim()))` with `l: &&str` —
  `contains(&l.trim())` compares `&&str` against `&str` through
  deref-coercion, no explicit `*l` needed. Used identically in
  `upsert_block`'s before/after-line filters and in `exclude_remove`'s
  line-drop loop (`crates/damon-git/src/lib.rs`).
- **`exclude_remove` hardened beyond the plan's sample.** Only
  `io::ErrorKind::NotFound` on the initial read is a no-op
  (`return Ok(())`); every other read error (permission denied, non-UTF8
  content) propagates as `GitError::Io { path, source }` instead of being
  swallowed. Test: `exclude_remove_propagates_non_missing_read_errors`
  (`crates/damon-git/tests/repo_sources.rs`) writes raw non-UTF8 bytes
  (`[0xFF, 0xFE, 0xFD]`) to the exclude file and asserts `Err`.
- **`exclude` hardened to match, post-final-review.** Both `exclude()`
  and `exclude_remove()` now propagate non-`NotFound` read errors instead
  of swallowing them; `exclude()` previously used
  `unwrap_or_default()` on the initial read, which risked clobbering a
  user's existing exclude file (non-UTF8 content, permission denied)
  with a fresh block on write. Test:
  `exclude_propagates_non_missing_read_errors_instead_of_clobbering`
  (`crates/damon-git/tests/repo_sources.rs`).
- **`agent rm` exclude cleanup fails closed beyond the plan's sample.**
  `cleanup_exclude` (`crates/damon/src/commands/agent.rs`) decides
  whether another agent still uses the repo via
  `store.all_agents().unwrap_or_default().iter().any(|a| match
  a.agent.as_ref() { Err(_) => true, Ok(f) => /* path match */ })` — a
  surviving agent whose `agent.toml` fails to parse counts as *still
  using the repo*, so the exclude block is left in place (stale block
  beats broken exclusions). Test:
  `agent_rm_skips_exclude_cleanup_when_survivor_toml_is_corrupt`
  (`crates/damon/tests/cli_agent.rs`) corrupts a survivor's `agent.toml`
  and asserts the block (`# damon begin`) survives `agent rm`.
  **Consequence, deliberate:** any corrupt `agent.toml` anywhere in the
  store suppresses cleanup for every repo being checked in that call — do
  not optimize this back to a narrower per-repo check.
- **`damon memory` gained a guard not in the plan.** If the agent's
  memory dir itself is missing, all modes (print-all, single-file,
  `--edit`) bail before doing anything else, with:
  `"no memory directory for {reference} at {} — the agent is broken;
  recreate it"` (`crates/damon/src/commands/memory.rs`, `run`). This
  closes an inconsistency the plan didn't anticipate: no-FILE would have
  printed empty output / exited 0 while FILE-given errored. Test:
  `memory_errors_when_memory_dir_is_missing`
  (`crates/damon/tests/cli_memory.rs`) deletes an agent's `memory/` dir
  entirely and asserts failure with `"no memory directory"` on stderr.
- **Editor rule shipped as planned.** `editor_from(visual, editor)`
  checks `$VISUAL`, then `$EDITOR`, then falls back to `"vi"`; each value
  is trimmed and an empty-after-trim value counts as unset
  (`find(|v| !v.is_empty())` post-`.trim()`). The resolved string is
  split on whitespace into program + args
  (`editor.split_whitespace()`) and spawned inheriting the TTY; on a
  non-zero exit damon calls `std::process::exit(status.code().unwrap_or(1))`,
  propagating the editor's own exit code (or `1` if the code is
  unavailable, e.g. killed by signal).
- **Doctor: structured checks, output verified byte-identical to
  legacy.** `CheckStatus { Ok(String), Missing, TooOld { found: (u32,
  u32), need: (u32, u32) } }` replaces the string-driven gate; gating
  (`failed_required`) reads `CheckStatus` via `CheckResult::passed()`,
  never the rendered display line — enforced by test
  `gate_reads_status_not_rendered_text`
  (`crates/damon/src/commands/doctor.rs`). `REQUIRED: [&str; 2] =
  ["git", "tmux"]`. The live-compared render output matched the
  pre-refactor output byte-for-byte, including the em-dash character
  (U+2014) in hint text. **Divergence:** the M4 design doc
  (`docs/superpowers/specs/2026-07-08-damon-m4-design.md`) listed a
  keyring check among doctor's checks, but none shipped — none existed
  pre-M4 either, and the byte-identical-output constraint won; a keyring
  check remains unimplemented.
- **Homebrew: explicit-URL tap fallback documented but not needed.**
  `docs/PACKAGING.md` records the `brew tap donnie-ccama/damon
  https://github.com/donnie-ccama/homebrew-damon` fallback for when tap
  auto-resolution fails against a private repo, but in practice
  `brew install --HEAD donnie-ccama/damon/damon` auto-tapped and resolved
  on the first try via the user's stored git credentials — no explicit
  tap step required. Source build completed in ~12s; `brew test damon`
  passed. Tap repo `donnie-ccama/homebrew-damon` at commit `9bd45bd`.

### M5 — as shipped (2026-07-09)

Per `docs/superpowers/specs/2026-07-09-damon-m5-design.md`: sweep the M4
parked-debt list (six items, all cleared) and ship distribution (repo
public, versioned Homebrew, AUR artifacts).

- **Preview scroll bounded, but to content length, not viewport.**
  `update_preview` in `crates/damon/src/tui/app.rs` computes `let max =
  p.content.lines().count().saturating_sub(1) as u16` and clamps `Down`/
  `j`/`PageDown` to `.min(max)`; `Up`/`PageUp` already saturated at 0.
  This is a pure-`Model` fix — no viewport height is threaded into
  `update_preview` — so on content taller than the pane, `max` can still
  scroll the last line to the top of the frame; it only stops the
  previously-unbounded runaway past the content. Documented here as the
  intentional scope, not a bug. The existing
  `preview_scrolls_and_escapes` fixture used 1-line content (`"a"`),
  which the clamp would pin at `scroll = 0` forever; it was widened to
  2-line (`"a\nb"`) so the `j`-then-`scroll == 1` assertion still holds
  under the new clamp. A new `preview_scroll_is_bounded_by_content` test
  covers the clamp itself.
- **`skills/` walk stopped following symlinks.** `collect_files` in
  `crates/damon/src/commands/memory.rs` switched from `path.is_dir()` /
  `path.is_file()` (which follow symlinks) to `entry.file_type()?`
  (which does not): a symlink — to a directory or a file — is neither
  `is_dir()` nor `is_file()` under `file_type`, so it's skipped outright.
  A self-referential symlink (`skills/loop` pointing at its own parent)
  can no longer recurse into a cycle; it's just absent from the
  collected files. The `damon memory` help/README text now documents
  that symlinks under `skills/` are ignored.
- **`KNOWN_PATTERNS` drift converted from a manual-sync liability to a
  test failure.** `crates/damon-git/src/lib.rs` exposes `pub fn
  known_patterns() -> &'static [&'static str]` over the existing
  `KNOWN_PATTERNS` array. A new cross-crate integration test,
  `crates/damon/tests/bridge_exclude_sync.rs`
  (`known_patterns_cover_every_bridge_filename`), runs
  `damon_core::bridge::write_bridges` for all three `RuntimeId`s
  (`Claude`, `Codex`, `Opencode`) into a temp worktree, collects every
  returned path relative to the worktree into a `BTreeSet`, and asserts
  that set equals `damon_git::known_patterns()` as a set. Today the union
  is `{CLAUDE.md, .claude/settings.json, AGENTS.md}`, matching
  `KNOWN_PATTERNS` exactly. A future runtime that emits a new bridge
  filename now fails this test at build time instead of silently
  breaking legacy-line migration.
- **`info`/`exclude` cross-process lock via `fs4`.** Added `fs4 = "0.13"`
  ("0.13.1" resolved) to the workspace `[workspace.dependencies]` and to
  `damon-git`, imported as `use fs4::fs_std::FileExt;` — the resolved
  0.13 API needed no fallback to a bare `fs4::FileExt` path. Locking the
  exclude file itself is unsafe, because `write_file`'s temp+rename
  swaps its inode out from under any lock held on the old fd; instead a
  private `fn with_exclude_lock<T>(common: &Path, f: impl FnOnce() ->
  Result<T, GitError>) -> Result<T, GitError>` locks a stable sidecar,
  `<common_dir>/info/.damon-exclude.lock` (created if absent, never
  renamed), via `lock_exclusive()`, runs `f`, and releases via an
  explicit `FileExt::unlock` plus drop. `common_dir` is resolved once
  per call and threaded into the helper. Both `exclude()` and
  `exclude_remove()` moved their full read-modify-write inside this
  helper; the M4 NotFound-only read-error hardening is preserved
  unchanged inside the locked section. A deterministic test spawns four
  threads hammering a shared counter guarded by the same lock file and
  asserts max observed concurrency is exactly 1 — proving the flock
  contends even across separate `File` opens in-process, not just across
  OS processes. The now-dead `exclude_path` helper (superseded by the
  sidecar path) was deleted. Folded in the M4-parked
  `cleanup_exclude` dedup: `crates/damon/src/commands/agent.rs` gained
  `fn canonical_common_dir(path: &str) -> Option<PathBuf>`, used on both
  sides of the survivor comparison; the fail-closed-on-corrupt-TOML
  behavior (`Err(_) => true`) is unchanged.
- **N+1 `tmux show-environment` eliminated.** The model now travels on
  the tmux session itself as a user option, `@damon_model`, set once at
  spawn: `crates/damon/src/commands/open.rs` calls `tmux.set_option(&name,
  "@damon_model", key)` right after `tmux.spawn(...)` succeeds, with a
  non-fatal warning on failure (the session is already live either way).
  `crates/damon-tmux/src/lib.rs`'s `list_info` format string became
  `#{session_name}|#{session_created}|#{@damon_model}`, parsed by a new
  `parse_info_line` — an empty third field parses to `model: None`, and
  a line missing the second (`created`) field is dropped entirely.
  `SessionInfo` gained `pub model: Option<String>`. `Tmux::env_var` —
  which existed only to read `DAMON_MODEL` per-session — was **removed**.
  `live_sessions` (`crates/damon/src/tui/snapshot.rs`) now maps each
  `SessionInfo` straight to `LiveSession { name, created_unix, model }`
  with no per-session tmux call: one `list_info` invocation per refresh
  regardless of session count. The `-e DAMON_MODEL=<key>` spawn env is
  kept as-is — the running process and its hooks still read it; only the
  TUI's listing path changed. Backward compatibility is deliberate and
  fallback-free: tmux sessions spawned by a pre-M5 damon carry no
  `@damon_model` and render model `?` until respawned — a fallback would
  reintroduce the N+1 this task removes.
- **`damon memory --edit` reachable from the TUI.**
  `crates/damon/src/commands/memory.rs` gained `pub fn spawn_editor(path:
  &Path) -> anyhow::Result<std::process::ExitStatus>`, extracted from the
  CLI's `edit_file` with the `std::process::exit` call removed — it just
  resolves `$VISUAL`/`$EDITOR`/`vi`, splits into program + args, and
  spawns inheriting the TTY. `edit_file` is now a thin wrapper:
  `spawn_editor` plus the same `std::process::exit(status.code()
  .unwrap_or(1))` on non-success, so CLI behavior is byte-identical.
  `tui/app.rs` gained `Action::Edit { path: PathBuf }`, and `Preview`
  gained a `path: PathBuf` field (all four construction sites of
  `Preview` were updated to carry it). Key `e` in the Memory tab emits
  `Action::Edit` for the selected memory file, whether or not the
  preview pane is open (previewing edits the previewed file; the list
  view edits `agent.memory.get(m.mem_idx)`); it no-ops when nothing is
  selected. Only `tui/mod.rs`'s event loop owns `terminal`, so
  `Action::Edit` is intercepted there rather than in the general
  dispatcher — the local `fn execute` was renamed to `fn execute_action`
  and kept an exhaustive match with a no-op `Action::Edit` arm (handled
  earlier in the loop). A new `fn suspend<T>(terminal: &mut
  ratatui::DefaultTerminal, f: impl FnOnce() -> T) -> std::io::Result<T>`
  does `disable_raw_mode()` + `LeaveAlternateScreen`, runs `f`, then
  `EnterAlternateScreen` + `enable_raw_mode()` + `terminal.clear()` (full
  redraw of the restored screen). The event loop calls `suspend(&mut
  terminal, || memory::spawn_editor(&path))` and sets `model.status` to
  `edited <file>` on success or an error string on failure (including a
  non-zero editor exit), forcing a refresh either way. `crossterm` is
  used only via ratatui's re-export (`ratatui::crossterm::{...}`) — no
  separate `crossterm` dependency, per the M3 convention. Smoke-tested
  manually via a pty harness (no TTY in the sandbox environment):
  `EDITOR=true` produced `edited <path>` with a clean redraw; `EDITOR=false`
  produced `editor exited 1`, also with a clean redraw.
- **Distribution: repo public, versioned Homebrew, AUR authored.**
  `donnie-ccama/damon` is now **public** (`gh repo edit --visibility
  public`, verified). Tag `v0.1.0` points at `148566f`, the finished-code
  `HEAD` for M5. Tap `donnie-ccama/homebrew-damon` at `6d812bc` gained a
  versioned formula stanza (`url` pinned to the `v0.1.0` tarball, `sha256
  afb34ba8d6d167b717b91053ac82b944f8ef6cbc5844184837950aa04968d495`,
  computed via `shasum -a 256` on macOS) alongside the existing `head`
  stanza; `brew audit --strict --online` is clean, a versioned install
  (no `--HEAD`) builds keg `0.1.0`, and `brew test` passes. `docs/
  PACKAGING.md` was rewritten for the public+versioned story (the
  private-tap-fallback text from M4 no longer applies). AUR artifacts
  are committed at `4db3520`: `packaging/aur/PKGBUILD`, a hand-verified
  `.SRCINFO` (tab-indented, matching what `makepkg --printsrcinfo` would
  emit), and `packaging/aur/PUBLISHING.md`. Per the design's explicit
  scope line, the actual `makepkg`/`namcap`/`git push` to
  `aur.archlinux.org` requires an Arch machine and **was not performed
  this session** — it's recorded as a pending user handoff, not a gap in
  the code.

### Parked debt (triaged, non-blocking)

All six M4 items (the `info`/`exclude` cross-process lock, the
`KNOWN_PATTERNS` manual-sync liability, the `skills/` symlink-cycle
guard, the `cleanup_exclude` dedup, the N+1 `tmux show-environment`
calls, and unbounded preview scroll) shipped in M5 and are cleared.
Newly parked during M5, all non-blocking:

- No unit test for the preview-mode `e` edit path — `Action::Edit`
  emission from the Memory-tab list view is covered by a `Model`/update
  test, but editing while the preview pane is open is verified only by
  the manual pty smoke test (`EDITOR=true`/`EDITOR=false`), since the
  suspend/resume path isn't `TestBackend`-testable.
- `suspend()` (`crates/damon/src/tui/mod.rs`) returns early on an IO
  error from `execute!(stdout(), LeaveAlternateScreen)` with raw mode
  already disabled — a residual inconsistent-terminal-state risk on that
  specific failure, accepted per the brief as out of scope for this
  milestone.
- Preview content goes stale after an in-place edit: editing the
  previewed file via `e` does not refresh `Preview.content`, so the pane
  shows pre-edit text until closed and reopened.
- AUR live publish (`makepkg`/`namcap`/`git push` to
  `aur.archlinux.org`) is authored and ready but not executed — it's a
  documented on-Arch handoff to the user (macOS cannot run those tools).

### Next milestone

**M6 candidates:** the on-Arch AUR publish (above); viewport-aware
preview scrolling (thread pane height into `update_preview` so the
content-length clamp also accounts for what's visible, closing the
divergence noted in the M5 entry above); live-refreshing preview content
after an in-place `e` edit; hardening `suspend()`'s early-return path so
a `LeaveAlternateScreen` failure can't leave the terminal in a mixed
raw/alternate-screen state; a `TestBackend`-reachable test for the
preview-mode `e` path if a way to fake the suspend/resume boundary
emerges.
