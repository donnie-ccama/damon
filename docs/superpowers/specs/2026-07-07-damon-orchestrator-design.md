# Damon — Design Spec

**Date:** 2026-07-07
**Status:** M1 + M2 SHIPPED (2026-07-08). This document is the approved design
plus the **As-built addendum** at the end, which records every divergence
between this spec and the shipped code. Read the addendum before planning M3 —
where it conflicts with the body, the addendum (i.e. the code) wins.

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

### Parked debt (triaged, non-blocking)

All five M3 items (tmp-file cleanup on failed atomic writes, popup
`TestBackend` coverage, the `ensure_selection` mem_idx-reset papercut,
the live-session-loop duplication, and the silent no-op on `N` with an
empty rail) shipped in M4 Tasks 1–5 and are cleared. Newly parked during
M4, all non-blocking, are M5 candidates:

- No cross-process lock on the `info`/`exclude` read-modify-write: two
  concurrent `damon open`s against the same repo can lose one update
  (the temp-file+rename pattern only guarantees single-writer atomicity,
  not mutual exclusion across processes). Self-heals on the next spawn,
  since bridges and the exclude block regenerate every `open` — revisit
  only if parallel opens become a routine workflow.
- `KNOWN_PATTERNS` in `damon-git` is a manually-synced list of bridge
  filenames; a future runtime that introduces a new bridge filename must
  add it there for legacy-line migration to pick it up.
- The `skills/` recursive walk in `commands/memory.rs` follows
  symlinked directories with no cycle guard.
- `cleanup_exclude` duplicates the expand-tilde → `common_dir` →
  `canonicalize` chain twice (once per comparison side) — a helper-
  extraction candidate, not a correctness issue.
- Carried from the M3 final review, still unaddressed: N+1
  `tmux show-environment` calls per TUI refresh at scale; unbounded
  preview scroll in the Memory tab.

### Next milestone

**M5 candidates:** a versioned Homebrew release (cut tag `v0.1.0`, add a
`url`/`sha256` formula stanza, or go public); wiring `damon memory --edit`
into the TUI via the same print-free core; `info`/`exclude` read-modify-
write locking (above); AUR packaging.
