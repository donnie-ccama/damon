# damon

Orchestrator for persistent coding agents: each agent is a folder (identity
TOML + markdown memory + its own git worktree), each session is a tmux
session on the `damon` socket, opened in Ghostty. Stateless — the filesystem
and tmux are the only sources of truth.

## Quickstart

    cargo install --path crates/damon
    damon doctor                 # tells you what to brew/pacman install
    damon init
    damon team new Newsletter
    damon agent new newsletter/Scout --repo-new --role "Researches topics"
    damon open scout             # spawns Claude Code in tmux, opens Ghostty

Sessions survive window close: `damon open scout` reattaches. `damon
sessions` lists, `damon kill scout` stops.

Design spec: docs/superpowers/specs/2026-07-07-damon-orchestrator-design.md
Status: M1 (Claude runtime). M2 adds OpenRouter models + Codex/OpenCode; M3 the TUI.
