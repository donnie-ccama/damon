# damon

**An orchestrator for persistent coding agents.** Build a roster of named
agents — each with its own git worktree, runtime CLI, and self-curated
markdown memory — and work alongside them in real terminals. Come back
tomorrow and the same agent remembers what it learned today.

Damon is deliberately boring under the hood: **plain files are the database,
tmux is the session layer, [Ghostty](https://ghostty.org) is the window.**
There is no daemon, no SQLite, no cloud. Everything damon knows lives in
folders you can read, edit, back up, and rsync between machines.

```
$ damon open scout
session damon_newsletter_scout_1        ← tmux session, spawned in the agent's worktree
                                        ← a Ghostty window opens with Claude Code running
```

## Features

- **Persistent agent identities** — an agent is a folder: `agent.toml`
  (name, role, runtime), markdown memory, and its own git worktree. Delete
  the folder, the agent is gone. Copy it, the agent moves.
- **Teams** — agents are grouped into teams (`newsletter/scout`,
  `web/fixer`); bare names work when unambiguous (`damon open scout`).
- **Self-curated memory** — each agent keeps `AGENT.md` (identity),
  `USER.md` (what it knows about you), `MEMORY.md` (its notes + write-back
  protocol), and self-authored skills. Memory lives *outside* the worktree,
  so branch churn never touches it. A generated `CLAUDE.md` bridge imports
  it into every session.
- **Three repo sources** — start an agent with a fresh repo
  (`--repo-new`), a clone (`--repo-clone URL`), or attach it to an existing
  local project via git worktree (`--repo-worktree PATH`) so several agents
  can work the same codebase on isolated branches.
- **Sessions that survive** — every session is a tmux session on a
  dedicated socket. Close the window, log out, come back: `damon open`
  reattaches exactly where the agent left off. Only `damon kill` (or a
  reboot) ends a session.
- **Model registry** — `models.toml` maps friendly names to runtime +
  environment. Ships with Claude, GPT (Codex), Kimi, MiniMax, and GLM
  entries. Adding a model is editing TOML, not code. *(OpenRouter key
  storage and non-Claude runtimes activate in M2.)*
- **Stateless and honest** — `damon sessions` asks tmux, `damon team ls`
  reads the filesystem. Nothing is cached, so nothing can drift. Broken
  TOML and stray directories are reported, never silently hidden.
- **One layout everywhere** — identical paths on macOS and Linux:
  data in `~/damon`, config in `~/.config/damon`.

## Requirements

| Tool | Why | Required? |
|---|---|---|
| git | every agent gets a repo or worktree | yes |
| tmux ≥ 3.2 | session persistence | yes |
| [Ghostty](https://ghostty.org) | terminal windows | recommended (any `$TERMINAL` works) |
| [Claude Code](https://code.claude.com) | the M1 agent runtime | for `damon open` |
| Rust toolchain | building damon | build only |

`damon doctor` checks all of these and tells you exactly what to install.

## Install

### macOS

```bash
brew install tmux ghostty rustup && rustup-init -y   # skip what you already have
npm i -g @anthropic-ai/claude-code                   # the agent runtime

git clone https://github.com/donnie-ccama/damon.git
cd damon
cargo install --path crates/damon

damon doctor && damon init
```

### Omarchy / Arch Linux

```bash
sudo pacman -S --needed git tmux ghostty rustup && rustup default stable
npm i -g @anthropic-ai/claude-code

git clone https://github.com/donnie-ccama/damon.git
cd damon
cargo install --path crates/damon

damon doctor && damon init
```

Omarchy ships Ghostty as its default terminal, so the only additions are
usually tmux and the Rust toolchain. If you prefer another terminal, set
`launcher = "env-terminal"` in `~/.config/damon/config.toml` and damon will
use `$TERMINAL` instead.

## Quickstart

```bash
damon team new Newsletter
damon agent new newsletter/Scout --repo-new --role "Researches topics"
damon open scout
```

A Ghostty window opens with Claude Code running inside Scout's worktree,
with Scout's memory loaded. First open only: Claude Code asks to allow the
external memory imports — answer **allow** (memory intentionally lives
outside the worktree).

```bash
damon sessions            # what's alive right now
damon open scout          # reattach (survives closing the window)
damon open scout --new    # a second parallel session for the same agent
damon kill scout          # end all of scout's sessions
```

Attach an agent to a project you already have:

```bash
damon agent new web/Fixer --repo-worktree ~/Projects/my-site
# Fixer works ~/Projects/my-site on its own branch (agent/fixer),
# in its own worktree, without touching your checkout
```

## Command reference

```
damon init                            scaffold ~/damon + default config
damon doctor                          check git/tmux/ghostty/runtimes, with install hints
damon team new <name> | ls | rm <team> [--force]
damon agent new <team>/<Name> [--role STR] [--runtime claude|codex|opencode]
      (--repo-new | --repo-clone URL | --repo-worktree PATH) [--branch B]
damon agent ls [team] | rm <team>/<agent> --yes
damon open <agent> [--model M] [--new]
damon sessions
damon kill <session-name | agent>
```

## Configuration

`~/.config/damon/config.toml` (all keys optional — these are the defaults):

```toml
[general]
root = "~/damon"          # where teams/agents live
default_runtime = "claude"

[tmux]
socket = "damon"          # dedicated server; your personal tmux is untouched

[terminal]
launcher = "ghostty"      # ghostty | env-terminal | print
```

`~/.config/damon/models.toml` — the model registry. Add a model by adding a
table; `${keyring:...}` values activate with key storage in M2.

## Data layout

```
~/damon/teams/<team>/
  team.toml
  agents/<agent>/
    agent.toml            # identity: name, role, runtime, repo binding
    memory/               # AGENT.md, USER.md, MEMORY.md, skills/  (canonical)
    worktree/             # the agent's git worktree — sessions run here
    logs/sessions.jsonl   # append-only session history
```

Everything is plain text. `rsync -a ~/damon ~/.config/damon othermachine:` is
a complete migration.

## Status & roadmap

- **M1 (shipped)** — teams, agents, all three repo sources, memory
  scaffolding + CLAUDE.md bridge, tmux sessions, Ghostty launch, doctor.
  Claude Code is the active runtime. 65 tests, real-git/real-tmux
  integration coverage, verified end-to-end on macOS.
- **M2** — OpenRouter models with OS-keyring key storage, Codex & OpenCode
  runtimes, session-end reflection hook.
- **M3** — ratatui TUI: the team/agent rail, live session badges, memory
  browser.
- **M4** — polish: `damon memory --edit`, packaging (Homebrew / AUR).

Design docs live in [docs/superpowers/specs](docs/superpowers/specs) and
[docs/superpowers/plans](docs/superpowers/plans).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT license](LICENSE-MIT) at your option. Unless you explicitly state
otherwise, any contribution intentionally submitted for inclusion in damon
shall be dual licensed as above, without any additional terms or conditions.

The agent-memory concepts are adapted from
[NousResearch/hermes-agent](https://github.com/NousResearch/hermes-agent) (MIT)
by way of [damon-ade](https://github.com/per-simmons/damon-ade)'s design;
damon shares no code with either.
