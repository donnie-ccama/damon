# cortado

**An orchestrator for persistent coding agents.** Build a roster of named
agents — each with its own git worktree, runtime CLI, and self-curated
markdown memory — and work alongside them in real terminals. Come back
tomorrow and the same agent remembers what it learned today.

Cortado is deliberately boring under the hood: **plain files are the
database, [Herdr](https://herdr.dev) is the session layer and window.**
There is no daemon of cortado's own, no SQLite, no cloud. Everything cortado
knows lives in folders you can read, edit, back up, and rsync between
machines; agents themselves are ephemeral processes over that persistent
memory.

```
$ cortado open scout
session cortado_newsletter_scout_1        ← Herdr agent pane, spawned in the agent's worktree
                                        ← Claude Code boots inside it, memory imported
```

> **New to cortado?** Open the no-jargon guided tour —
> [docs/slides/cortado-guide.html](docs/slides/cortado-guide.html) — in any
> browser: what cortado is, how it works, and the handful of commands you'll
> actually use.

## Features

- **Persistent agent identities** — an agent is a folder: `agent.toml`
  (name, role, runtime), markdown memory, and its own git worktree. Delete
  the folder, the agent is gone. Copy it, the agent moves.
- **Teams** — agents are grouped into teams (`newsletter/scout`,
  `web/fixer`); bare names work when unambiguous (`cortado open scout`).
- **Self-curated memory** — each agent keeps `AGENT.md` (identity),
  `USER.md` (what it knows about you), `MEMORY.md` (its notes + write-back
  protocol), and self-authored skills. Memory lives *outside* the worktree,
  so branch churn never touches it. A generated `CLAUDE.md` bridge imports
  it into every session.
- **Three repo sources** — start an agent with a fresh repo
  (`--repo-new`), a clone (`--repo-clone URL`), or attach it to an existing
  local project via git worktree (`--repo-worktree PATH`) so several agents
  can work the same codebase on isolated branches.
- **Sessions that survive** — a live session is an agent pane in the Herdr
  server; closing the Herdr client detaches it, it does not kill it. The
  durable thing was never the session, though — it's the memory. `cortado
  open` materializes an agent pane on demand (reattach if one's live, spawn
  fresh with memory loaded if not); `cortado kill` (or a reboot) ends it.
- **Model registry** — `models.toml` maps friendly names to runtime +
  environment. Ships with Claude, GPT (Codex), OpenRouter GPT-5, Kimi,
  MiniMax, and GLM entries. Adding a model is editing TOML, not code.
- **Stateless and honest** — `cortado sessions` asks Herdr, `cortado team ls`
  reads the filesystem. Nothing is cached, so nothing can drift. Broken
  TOML and stray directories are reported, never silently hidden.
- **One layout everywhere** — identical paths on macOS and Linux:
  data in `~/cortado`, config in `~/.config/cortado`.

## Requirements

| Tool | Why | Required? |
|---|---|---|
| git | every agent gets a repo or worktree | yes |
| herdr ≥ 0.7.4 | session layer & window | yes |
| agent runtime: [Claude Code](https://code.claude.com) / [Codex](https://github.com/openai/codex) / [OpenCode](https://opencode.ai) | install the runtimes you use (Claude Code is the default) | for `cortado open` |
| Rust toolchain | building cortado | build only |

`cortado doctor` checks all of these and tells you exactly what to install.

## Install

### macOS

```bash
brew install herdr rustup && rustup-init -y   # skip what you already have
npm i -g @anthropic-ai/claude-code            # the agent runtime

git clone https://github.com/donnie-ccama/cortado.git
cd cortado
cargo install --path crates/cortado

cortado doctor && cortado init
```

### Omarchy / Arch Linux

```bash
sudo pacman -S --needed git rustup && rustup default stable
npm i -g @anthropic-ai/claude-code
# herdr isn't packaged for Arch yet — install it per https://herdr.dev

git clone https://github.com/donnie-ccama/cortado.git
cd cortado
cargo install --path crates/cortado

cortado doctor && cortado init
```

## Quickstart

```bash
cortado team new Newsletter
cortado agent new newsletter/Scout --repo-new --role "Researches topics"
cortado open scout
```

A Herdr pane opens with Claude Code running inside Scout's worktree, with
Scout's memory loaded. First open only: Claude Code asks to allow the
external memory imports — answer **allow** (memory intentionally lives
outside the worktree).

```bash
cortado sessions            # what's alive right now
cortado open scout          # reattach (survives closing the window)
cortado open scout --new    # a second parallel session for the same agent
cortado kill scout          # end all of scout's sessions
```

Attach an agent to a project you already have:

```bash
cortado agent new web/Fixer --repo-worktree ~/Projects/my-site
# Fixer works ~/Projects/my-site on its own branch (agent/fixer),
# in its own worktree, without touching your checkout
```

> **Note (worktree agents):** while a worktree agent exists for a repo, cortado
> keeps a `# cortado begin … # cortado end` block in that repo's shared
> `.git/info/exclude`, so *untracked* files named like bridge files
> (`CLAUDE.md`, `AGENTS.md`, `.claude/settings.json`) are hidden from
> `git status` in the repo's **other** worktrees too. Tracked files are
> unaffected. The block is removed when the last cortado worktree agent for
> that repo is deleted.

## Command reference

```
cortado init                            scaffold ~/cortado + default config
cortado doctor                          check git/herdr/runtimes, with install hints
cortado team new <name> | ls | rm <team> [--force]
cortado agent new <team>/<Name> [--role STR] [--runtime claude|codex|opencode]
      (--repo-new | --repo-clone URL | --repo-worktree PATH) [--branch B]
cortado agent ls [team] | rm <team>/<agent> --yes
cortado open <agent> [--model M] [--new]
cortado sessions
cortado kill <session-name | agent>
cortado memory <team>/<agent> [FILE]          # print memory (all surfaces, or one file)
cortado memory <team>/<agent> [FILE] --edit   # open in $VISUAL/$EDITOR (default MEMORY.md)
```

> Symlinks under an agent's `skills/` directory are ignored by `cortado memory`.

## TUI

```bash
cortado ui
```

A ratatui screen over the same teams/agents rail as `cortado team ls` /
`cortado agent ls`, plus live Herdr session state. `cortado ui` is
**stateless**: it re-derives everything from the filesystem and `herdr agent
list` on a 2s tick (and immediately after any action) — nothing is cached,
so nothing can drift, same principle as the CLI. Every key below calls the
same library function its CLI verb uses; there is no parallel TUI-only code
path. Quitting the TUI (`q`) never kills sessions — it's a view, not a
supervisor. Rail badges show live agent **status** — idle / working /
blocked — sourced straight from `herdr agent list`, not a session count.

| Key | Action |
|---|---|
| `↑/↓` / `j/k` | navigate the rail |
| `Tab` | toggle Sessions / Memory tab |
| `m` | jump to Memory tab |
| `n` | model-picker popup (entries from `models.toml`) → spawn new session (same as `cortado open --model M --new`) → open terminal |
| `Enter` | open/attach selected agent (same as `cortado open`: reattach highest-`n` live session, else spawn on default model); in Memory tab: preview selected file |
| `x` | kill with confirm popup; multiple sessions → kill all (same as `cortado kill team/agent`), partial failures reported in status line |
| `N` | new-agent form popup: team (preselected from rail), name, runtime, role, repo source (new / clone URL / worktree path), branch → same function as `cortado agent new` |
| `q` / `Esc` | `Esc` closes the top popup; `q` quits the TUI — sessions keep running |

**`j`/`k` mean different things depending on the active tab:** on the
Sessions tab they navigate the rail (same as `↑/↓`); on the Memory tab they
move the memory-preview cursor instead, and `↑/↓` navigate the rail. This
lets you scroll a memory file without leaving the rail's keyboard model.

Left pane is the rail (teams → agents, badged by live status — idle
yellow, working green, blocked red); right pane has Sessions (name, model,
status) and Memory (file list + scrollable preview) tabs; the status line
shows the last action's result or error, using the same error text the CLI
prints on failure.

## Herdr workspace

Cortado owns one Herdr workspace, labeled **Cortado**: the rail runs as its
pinned left pane, and every agent `cortado open`s spawns as a pane to the
rail's right. All layout — splitting, moving, resizing, zooming — is native
Herdr; cortado does not manage panes and will not fight you over them.
Closing a pane, a client, or the whole Herdr window never loses anything:
agents are ephemeral processes running over persistent markdown memory, and
the durable thing was always the memory, not the pane. `cortado open` just
materializes an agent on demand — reattach if a session's live, spawn fresh
with memory loaded if not.

### Optional Ghostty presentation profile

Cortado ships a matching **Cortado Night** theme and a restrained Ghostty
profile under `extras/ghostty/`. They are optional: the TUI remains readable
with the user's existing terminal theme.

```bash
mkdir -p ~/.config/ghostty/themes
cp "extras/ghostty/Cortado Night" ~/.config/ghostty/themes/
# Review extras/ghostty/cortado.conf, then copy the settings you want into
# ~/.config/ghostty/config.ghostty
```

The profile uses subtle transparency and blur, balanced window padding, and a
transparent macOS titlebar. Font family is deliberately not prescribed.

## Configuration

`~/.config/cortado/config.toml` (all keys optional — these are the defaults):

```toml
[general]
root = "~/cortado"          # where teams/agents live
default_runtime = "claude"

[herdr]
binary = "herdr"            # path or name of the herdr binary
workspace = "Cortado"       # label of the Herdr workspace cortado owns
```

Old `[tmux]`/`[terminal]` sections in an existing config file are retired:
they still parse (unknown keys are ignored) but produce a one-time nudge on
`cortado open` telling you to remove them — cortado no longer reads them.

`~/.config/cortado/models.toml` — the model registry. Add a model by adding a
table; `${keyring:...}` values resolve against OS-keyring-stored keys.

**Test/CI seam:** `CORTADO_HERDR_SESSION=<name>` targets a named, isolated
Herdr session instead of your default one — used by cortado's own
integration tests so they never touch a real session. Production code never
sets it. This sits alongside the existing escape hatches below
(`CORTADO_KEY_<ACCOUNT>`, `CORTADO_NO_KEYRING`, `CORTADO_BIN_<RUNTIME>`).

### Provider keys

Models that need an API key reference it as `${keyring:openrouter}` (or
whatever account name) in `models.toml`. Store the key once:

```bash
cortado key set openrouter     # prompts for the key (hidden input), saves to the OS keyring
cortado key rm openrouter      # remove it
```

Keys are stored in the OS keyring (Keychain on macOS, Secret Service on
Linux) under service `cortado`, account `<name>`, never written to disk by
cortado itself.

**Escape hatch:** set `CORTADO_KEY_<ACCOUNT>` (uppercased, `-`/`.` → `_`) to
bypass the keyring entirely — useful for CI or containers where no keyring
is available, e.g. `CORTADO_KEY_OPENROUTER=sk-...`.

**`CORTADO_NO_KEYRING`** — set (to any non-empty value) to disable keyring
access altogether; `cortado key set/rm` and any model needing a keyring key
will fail with a clear error instead of touching the OS keychain.

**Threat model:** a resolved key reaches the agent's session as an
environment variable (`herdr agent start --env KEY=VALUE`) and remains in
the session's process environment for the session's lifetime. Both that
environment and the momentary argv of the `herdr` CLI invocation are
readable only by your own user account — never by other users — and cortado
never writes the key to a file, log, or shell history. Any error text or
display string that might echo that argv redacts `--env` values. Treat any
process running in the session as able to read the key (that's what it's
for).

## Data layout

```
~/cortado/teams/<team>/
  team.toml
  agents/<agent>/
    agent.toml            # identity: name, role, runtime, repo binding
    memory/               # AGENT.md, USER.md, MEMORY.md, skills/  (canonical)
    worktree/             # the agent's git worktree — sessions run here
    logs/sessions.jsonl   # append-only session history
```

Everything is plain text. `rsync -a ~/cortado ~/.config/cortado othermachine:` is
a complete migration.

## Status & roadmap

- **M1 (shipped)** — teams, agents, all three repo sources, memory
  scaffolding + CLAUDE.md bridge, tmux sessions, Ghostty launch, doctor.
  Claude Code is the active runtime. 65 tests, real-git/real-tmux
  integration coverage, verified end-to-end on macOS.
- **M2 (shipped)** — `cortado key set/rm` OS-keyring key storage, OpenRouter
  models live in the registry, Codex & OpenCode runtimes, session-end
  reflection via a Claude Code Stop hook.
- **M3 (shipped)** — ratatui TUI (`cortado ui`): the team/agent rail with live
  session badges, Sessions/Memory tabs, model-picker/kill/new-agent popups.
  Stateless, 2s refresh, zero clippy warnings workspace-wide.
- **M4** — `cortado memory --edit`, doctor's string-driven tmux gate, `cortado
  memory` command, packaging (Homebrew / AUR).
- **M5 (shipped)** — single-window workspace: `cortado ui` rail + agent
  viewer panes in one tmux client (`launcher = "workspace"`, the new
  default), native tmux keys everywhere, agent sessions untouched.
- **M6 (in verification)** — Herdr substrate swap: tmux + Ghostty + the
  workspace session layer replaced by [Herdr](https://herdr.dev). `open` /
  `sessions` / `kill` / `doctor` / `ui` all drive Herdr through the new
  `cortado-herdr` crate; `cortado-tmux` and `cortado-term` are gone. Data
  model and memory model are unchanged. Code-complete and unit/integration
  green; pending a manual end-to-end pass on macOS with real Herdr + Claude
  Code before this line reads "shipped".

### Migrating from a tmux-era install

If you're upgrading from M5 or earlier: cortado no longer starts, sees, or
manages tmux sessions at all. Any agent sessions still running in your old
`cortado` tmux socket keep running, but `cortado sessions`/`cortado kill`
can't see them anymore — finish what's in flight the old way, then retire
the socket:

```bash
tmux -L cortado kill-server
```

Your data (`~/cortado`, `models.toml`, keyring entries) is untouched by the
upgrade; only the session layer changed.

Design docs live in [docs/superpowers/specs](docs/superpowers/specs) and
[docs/superpowers/plans](docs/superpowers/plans).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Unless you explicitly state
otherwise, any contribution intentionally submitted for inclusion in cortado
shall be dual licensed as above, without any additional terms or conditions.

The agent-memory concepts are adapted from
[NousResearch/hermes-agent](https://github.com/NousResearch/hermes-agent) (MIT)
by way of [cortado-ade](https://github.com/per-simmons/cortado-ade)'s design;
cortado shares no code with either.
