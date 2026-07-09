# Damon M5 — sweep the ledger, ship public

Design approved 2026-07-09. Scope is the full M5 candidate list recorded
in the parent spec's as-built addendum
(`2026-07-07-damon-orchestrator-design.md`, "Next milestone" + the M4
parked-debt list): TUI completeness (`memory --edit` in the TUI, bounded
preview scroll, N+1 `tmux show-environment` fix), correctness/
maintainability debt (cross-process lock on `info/exclude`, de-manualized
`KNOWN_PATTERNS`, `skills/` symlink-cycle guard), and distribution
(repo goes public, versioned Homebrew release, AUR package).

Decisions locked with the user:

- **Scope:** the full addendum sweep — nothing deferred.
- **Release model:** `donnie-ccama/damon` **goes public**; the versioned
  Homebrew formula uses a public tarball `url` + `sha256` (no token).
- **AUR:** author `PKGBUILD` + `.SRCINFO` + a runbook here (macOS, fully
  reviewed); the user runs the on-Arch `makepkg`/`namcap`/`git push`
  steps. macOS cannot run `makepkg`/`namcap`/`pacman`, so those are a
  documented handoff, not an automated step.

## Bucket A — TUI completeness

### 1. `memory --edit` in the TUI

Reuse the existing print-free memory core; do not duplicate editor logic.

- **Refactor `crates/damon/src/commands/memory.rs`:** extract
  `pub fn spawn_editor(path: &Path) -> anyhow::Result<std::process::ExitStatus>`
  — resolves `$VISUAL` → `$EDITOR` → `vi` (via the existing
  `editor_from`), splits on whitespace into program + args, spawns
  inheriting the TTY, and **returns the status without calling
  `std::process::exit`**. The CLI `edit_file` becomes a thin wrapper:
  call `spawn_editor`, and on non-success `std::process::exit(status
  .code().unwrap_or(1))` exactly as today. CLI behavior is unchanged;
  the TUI now has a process-safe entry point.
- **`Action::Edit { path: PathBuf }`** added to `tui/app.rs`.
- **Key `e` in the Memory tab** (`tui/app.rs` `update`): edits the
  selected memory file. It works whether or not the preview pane is
  open — when preview is open, `e` edits the file being previewed; in
  the memory list, it edits `agent.memory.get(m.mem_idx)`. Emits
  `Action::Edit { path }`; no-op when no memory file is selected.
- **Suspend/resume in the event loop** (`tui/mod.rs`): the editor needs
  the real terminal, and only `event_loop` owns `terminal`, so
  `Action::Edit` is intercepted there, not in `execute()`. A helper

  ```rust
  fn suspend<T>(
      terminal: &mut ratatui::DefaultTerminal,
      f: impl FnOnce() -> T,
  ) -> std::io::Result<T>
  ```

  runs `disable_raw_mode()` + `LeaveAlternateScreen`, calls `f()`, then
  `EnterAlternateScreen` + `enable_raw_mode()` + `terminal.clear()`
  (forces a full redraw of the restored screen). On `Action::Edit`, the
  loop calls `suspend(&mut terminal, || memory::spawn_editor(&path))`,
  sets `model.status` to `edited <file>` on success or `error: …` on
  failure (including a non-zero editor exit), and forces `refresh =
  true`. `execute()` keeps handling every other action unchanged and
  keeps its `-> bool` (quit) signature.
- crossterm entry points come via ratatui's re-export
  (`ratatui::crossterm::{terminal::{…}, execute}`) — no separate
  crossterm dependency, matching the M3 convention.

Tests: a `Model`/update test that pressing `e` on a selected memory file
emits `Action::Edit` with that file's path, and that `e` with no
selection emits nothing. The suspend/resume path itself is not
`TestBackend`-testable; it is covered by a manual smoke test with
`EDITOR=true` (success) and `EDITOR=false` (error status), documented in
the task.

### 2. Bounded preview scroll

In `tui/app.rs` `update_preview`, clamp downward scroll so it cannot run
past the content:

```rust
let max = p.content.lines().count().saturating_sub(1) as u16;
KeyCode::Down | KeyCode::Char('j') => p.scroll = (p.scroll + 1).min(max),
KeyCode::PageDown => p.scroll = p.scroll.saturating_add(10).min(max),
```

Upward scroll already saturates at 0. This is a pure-`Model` fix — no
viewport height is threaded in, so the last line can still scroll to the
top of the pane, but scroll is bounded by the content length (the
reported unbounded-scroll bug). Test at the `Model`/update level:
repeated `j`/PageDown on short content stops at `max`.

### 3. Kill the N+1 `tmux show-environment`

Today `live_sessions` calls `list_info` once, then `env_var` once per
session to read `DAMON_MODEL` — N+1 tmux invocations every 2s refresh.
Replace the per-session read with a tmux **user option** surfaced in the
single `list_info` format string.

- **`crates/damon-tmux/src/lib.rs`:**
  - `SessionInfo` gains `pub model: Option<String>`.
  - `list_info` format becomes
    `#{session_name}|#{session_created}|#{@damon_model}`; parse three
    `|`-separated fields, empty third field → `None`. (User options
    render empty when unset; `|` stays the separator per the tmux-3.7b
    tab-mangling note.)
  - New `pub fn set_option(&self, session: &str, name: &str, value:
    &str) -> Result<(), TmuxError>` running `set-option -t <session>
    <name> <value>`.
  - `env_var` is **removed** — it existed only for the model read and is
    now dead.
- **`crates/damon/src/commands/open.rs`:** after `tmux.spawn(...)`
  succeeds, call `tmux.set_option(&name, "@damon_model", key)`. Keep the
  existing `-e DAMON_MODEL=<key>` in the spawn env (the running process
  and hooks still see it); the user option is purely for fast listing.
  A `set_option` failure is non-fatal — warn and continue (the session
  is already live; it will just render model `?`).
- **`crates/damon/src/tui/snapshot.rs`:** `live_sessions` maps each
  `SessionInfo` straight to `LiveSession { name, created_unix, model }`
  — no per-session tmux call. One tmux invocation per refresh regardless
  of session count.

Backward compatibility: sessions spawned by a pre-M5 damon have no
`@damon_model`, so they render model `?` until respawned — self-healing,
documented, no fallback path (a fallback would reintroduce the N+1).

Tests: the existing real-tmux `builds_from_a_real_tmux_server` test is
updated to set `@damon_model` via `set_option` (or spawn through the
`open` path) and assert the model is read back through `list_info`; a
`list_info` parse unit test covers the empty-third-field → `None` case.

## Bucket B — correctness / maintainability debt

### 4. Cross-process lock on `info/exclude`

Two concurrent `damon open`s against the same repo can lose one update:
temp-file+rename guarantees single-writer atomicity, not mutual
exclusion. Fix with an advisory `flock`.

- **New dependency:** `fs4` (maintained successor to `fs2`) added to the
  workspace `[workspace.dependencies]` and to `crates/damon-git`. This
  is a deliberate exception to the "no new crates" milestone tradition —
  the M4 review flagged this lock as the top parked item, and `flock` is
  the only robust cross-process fix (auto-released on fd close, so a
  crash leaves no stale lock).
- **Lock on a stable file, never the exclude file itself.** Locking
  `info/exclude` is unsafe because `write_file`'s temp+rename swaps the
  inode out from under the lock, so a second process opening the path
  gets the new inode and is not excluded. Instead, lock
  `<common_dir>/info/.damon-exclude.lock` (created if absent, never
  renamed).
- A private helper wraps the critical section:

  ```rust
  fn with_exclude_lock<T>(
      common: &Path,
      f: impl FnOnce() -> Result<T, GitError>,
  ) -> Result<T, GitError>
  ```

  opens/creates the lock file, `FileExt::lock_exclusive()`, runs `f`,
  and unlocks on `File` drop. Both `exclude` and `exclude_remove` move
  their read-modify-write inside this helper. `common_dir` is resolved
  once and passed to the helper (so the lock file lives beside the
  exclude file).
- **Fold in the M4 leftover:** `cleanup_exclude` in
  `crates/damon/src/commands/agent.rs` duplicates the expand-tilde →
  `common_dir` → `canonicalize` chain twice. Extract a small local
  `fn canonical_common_dir(path: &str) -> Option<PathBuf>` and use it
  on both comparison sides. Cosmetic; no behavior change.

Tests: acquire the lock via `with_exclude_lock` (or directly on the lock
path), then from a second `File` on the same lock path assert
`try_lock_exclusive()` returns `WouldBlock` — deterministic OS-level
mutual-exclusion proof without spawning processes. The existing exclude
block tests continue to pass unchanged (the lock is transparent to
single-writer behavior).

### 5. De-manualize `KNOWN_PATTERNS`

`KNOWN_PATTERNS` in `damon-git` is a hand-synced list of bridge
filenames; a new runtime bridge file must be added there for legacy-line
migration to work. Keep the list where it is (no new crate dependency
edge) but guard it with a cross-crate test.

- **`crates/damon-git/src/lib.rs`:** expose `pub fn known_patterns() ->
  &'static [&'static str]` returning `&KNOWN_PATTERNS`.
- **`crates/damon` integration test** (new
  `crates/damon/tests/bridge_exclude_sync.rs`, which sees both crates):
  for every `RuntimeId` (`Claude`, `Codex`, `Opencode`), run
  `damon_core::bridge::write_bridges` into a temp worktree with a clean
  (whitespace-free) `damon_exe`, collect each returned path relative to
  the worktree, union them, and assert the set equals
  `damon_git::known_patterns()` as a set. A future runtime that emits a
  new bridge filename fails this test until `KNOWN_PATTERNS` is updated —
  converting the silent liability into a hard failure.

No signature changes to `exclude`/`exclude_remove`; no
`damon-git → damon-core` dependency added.

### 6. `skills/` symlink-cycle guard

`collect_files` in `crates/damon/src/commands/memory.rs` uses
`path.is_dir()` / `path.is_file()`, both of which follow symlinks — a
symlink loop under `skills/` would recurse until the OS errors on path
length. Switch to `entry.file_type()?`, which does **not** follow
symlinks:

```rust
let ft = entry.file_type().map_err(|e| anyhow::anyhow!("{}: {e}", dir.display()))?;
if ft.is_dir() {
    collect_files(&path, base, out)?;
} else if ft.is_file() {
    // read + push
}
```

A symlink (to a dir or file) is neither `is_dir()` nor `is_file()` under
`file_type`, so it is skipped entirely — no recursion into symlinked
directories, no cycles. Memory is agent-authored real files; document
that symlinks under `skills/` are ignored by `damon memory`.

Test: create a `skills/loop` symlink pointing at its own parent and
assert `files()` returns the real files without hanging or erroring.

## Bucket C — distribution (goes public)

### 7. Go public

One isolated, gated task:

```bash
gh repo edit donnie-ccama/damon --visibility public
```

Authorized by the release-model decision above. Verify with `gh repo
view donnie-ccama/damon --json visibility`. No code change.

### 8. Versioned Homebrew release

- Tag the finished M5 `HEAD` (after all code tasks land) and push:
  `git tag v0.1.0 && git push origin v0.1.0`. Workspace version is
  already `0.1.0`; this is the first release tag.
- **Tap `donnie-ccama/homebrew-damon`, `Formula/damon.rb`:** add a
  stable stanza and keep `head` for `--HEAD`:

  ```ruby
  url "https://github.com/donnie-ccama/damon/archive/refs/tags/v0.1.0.tar.gz"
  sha256 "<computed>"
  head "https://github.com/donnie-ccama/damon.git", branch: "main"
  ```

  `sha256` computed on macOS via `curl -sL <url> | shasum -a 256`.
  `depends_on "rust" => :build`, `std_cargo_args(path: "crates/damon")`,
  and the `test do` block are unchanged.
- Verify live on macOS: `brew audit --strict --online damon`,
  `brew uninstall damon` then `brew install donnie-ccama/damon/damon`
  (versioned path, no `--HEAD`), `damon --version` → `damon 0.1.0`,
  `brew test damon`.
- Rewrite `docs/PACKAGING.md`: public install story
  (`brew install donnie-ccama/damon/damon`), the versioned-vs-HEAD
  distinction, and the release-cut procedure (tag → sha256 → formula →
  audit).

### 9. AUR package

Authored and committed here; published by the user on Arch.

- **`packaging/aur/PKGBUILD`** in the damon repo: `pkgname=damon`,
  `pkgver=0.1.0`, `arch=('x86_64' 'aarch64')`, `license=('MIT' 'Apache')`,
  `makedepends=('cargo')`,
  `source=("$pkgname-$pkgver.tar.gz::https://github.com/donnie-ccama/damon/archive/refs/tags/v$pkgver.tar.gz")`,
  matching `sha256sums`. `build()` runs
  `cargo build --release --frozen --package damon` (or `--locked`);
  `package()` installs `target/release/damon` to `usr/bin`, plus
  `LICENSE-MIT` and `LICENSE-APACHE` to `usr/share/licenses/$pkgname/`.
- **`packaging/aur/.SRCINFO`** generated to match (byte-exact to what
  `makepkg --printsrcinfo` would produce; the AUR requires it and it is
  hand-verifiable from the PKGBUILD fields).
- **`packaging/aur/PUBLISHING.md`** — the exact on-Arch runbook the user
  executes: `makepkg -si` (build+install test), `namcap PKGBUILD` and
  `namcap *.pkg.tar.zst` (lint), then first-publish
  `git clone ssh://aur@aur.archlinux.org/damon.git`, copy `PKGBUILD` +
  `.SRCINFO`, `git add`, commit, `git push`. Prerequisites listed: an
  AUR account with a registered SSH key, and an Arch environment.
- `sha256sums` is computed on macOS (`shasum -a 256`), so the committed
  artifacts are complete; only `makepkg`/`namcap`/`git push` await the
  Arch box. `.SRCINFO` regeneration on Arch (`makepkg --printsrcinfo`)
  is part of the runbook in case any field needs to change.

## Build order

Code first (cheap → meaty), then packaging (so the tag captures finished
M5), then docs:

1. Bounded preview scroll (§2).
2. `skills/` symlink guard (§6).
3. `KNOWN_PATTERNS` sync test + `known_patterns()` (§5).
4. `info/exclude` lock + `cleanup_exclude` dedup (§4).
5. N+1 tmux model via `@damon_model` (§3).
6. `memory --edit` in the TUI (§1).
7. Go public (§7).
8. Tag `v0.1.0` + versioned Homebrew formula + `PACKAGING.md` (§8).
9. AUR `PKGBUILD` + `.SRCINFO` + `PUBLISHING.md` (§9).
10. M5 as-built addendum + milestone gate.

Each code task lands green (`cargo fmt --check && cargo clippy
--workspace --all-targets && cargo test --workspace`, zero warnings) and
commits per task, per the parent spec's milestone rule.

## Out of scope

- Driving the AUR publish from this session — macOS cannot run
  `makepkg`/`namcap`; the on-Arch steps are a documented user handoff.
- Bumping the crate version beyond `0.1.0` — this is the first tagged
  release.
- A model fallback for pre-M5 tmux sessions lacking `@damon_model` — they
  render `?` until respawn by design (a fallback reintroduces the N+1).
- Viewport-aware preview scrolling — the clamp is to content length, not
  content-minus-viewport; bounding the runaway is the goal.
