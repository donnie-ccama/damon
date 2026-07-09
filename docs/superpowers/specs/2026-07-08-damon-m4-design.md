# Damon M4 — polish and packaging

Design approved 2026-07-08. Scope is the full M4 ledger recorded in the
parent spec's as-built addendum (`2026-07-07-damon-orchestrator-design.md`,
"Next milestone"): the `damon memory` command, the doctor structured-check
refactor, the shared `info/exclude` resolution, Homebrew packaging, and the
five parked M3 debt items.

Decisions locked with the user:

- **Scope:** full addendum list — nothing deferred except AUR.
- **Packaging:** private Homebrew tap, source build (repo stays private).
- **info/exclude:** marked block + cleanup on last-agent removal (no
  per-worktree `extensions.worktreeConfig` mutation of user repos).

## 1. `damon memory <team>/<agent> [FILE]`

The one new feature. New `crates/damon/src/commands/memory.rs` following
the M3 print-free-core convention: a core function returning data, a thin
CLI wrapper that prints.

Behavior:

- **No FILE:** print every memory surface concatenated — `AGENT.md`,
  `USER.md`, `MEMORY.md`, then `skills/*` in filename order — each
  preceded by a header line of the form `── <path relative to memory
  dir> ──`. (Spec body: "print memory file(s)"; this is the plural case.)
- **FILE given:** print exactly that file, resolved relative to the
  agent's memory dir (`MEMORY.md`, `skills/foo.md`, …).
  - **Traversal guard:** the resolved path must remain under the memory
    dir; `..`, absolute paths, and symlink escapes are rejected with a
    clear error. Missing file → error naming the path tried.
- **`--edit`:** opens an editor on FILE (default `MEMORY.md` when FILE is
  omitted). Editor resolution: `$VISUAL`, then `$EDITOR`, then `vi`.
  Spawned inheriting the TTY (stdin/stdout/stderr); damon exits with the
  editor's exit status. `--edit` does not create missing files — editing
  a nonexistent FILE is an error (the seeded surfaces always exist; a
  typo should not create a stray file).
- Core signature (shape, not verbatim): resolve agent → memory dir →
  return `Vec<(PathBuf, String)>` for the print path, or the single
  validated `PathBuf` for the edit path. The TUI can reuse the same core
  later for an edit hook; not wired in M4.

Tests (tempdir fixtures): full-print ordering and headers, single-file
resolution, traversal rejection (`../x`, absolute path, symlink pointing
outside), missing-file error. Editor launch itself is not CI-tested
(same policy as Ghostty launching); the editor-resolution order is a
pure function and is tested.

## 2. Doctor: structured checks replace the string-driven gate

Today `commands/doctor.rs` gates on `tmux_line.starts_with("ok")` — a
display string driving control flow (the debt item parked since M1+M2).

Refactor:

- Introduce a small check model inside `commands/doctor.rs`:
  `CheckResult { name: &'static str, status: CheckStatus, hint: Option<String> }`
  with `enum CheckStatus { Ok(String), Missing, TooOld { found: (u32, u32), need: (u32, u32) } }`
  (the `Ok` payload carries the detail text, e.g. the version line).
- Checks (git, tmux, ghostty, runtime CLIs, keyring) each produce a
  `CheckResult`; a single render pass prints them; required-dependency
  gating reads `status`, never the rendered string.
- Output format stays what it is today — this is a refactor, not a UX
  change. Per-OS install hints (`hint(..)`) survive as the `hint` field.

Tests: unit tests on the status logic (gating decision from a set of
`CheckResult`s, `TooOld` formatting). No golden output tests — doctor has
none today and the render pass is trivial; before/after output compared
by hand once.

## 3. `info/exclude`: sentinel-delimited block + cleanup

`damon_git::exclude` currently appends idempotent lines to the source
repo's shared `<git-common-dir>/info/exclude`, which leaks damon patterns
into every worktree of that repo. Resolution (user-approved): marked
block, not per-worktree config.

Writer (`damon-git`):

- Damon's patterns live in exactly one block:

  ```
  # damon begin
  CLAUDE.md
  ...
  # damon end
  ```

- Writing is an idempotent block rewrite: if the markers exist, replace
  the block contents in place; otherwise append the block. Lines outside
  the markers are never modified, reordered, or removed — with one
  exception: known damon patterns found *outside* the markers (legacy
  lines from pre-M4 installs) are stripped when the block is written.
  Safe because the patterns are damon-specific bridge filenames. This is
  the whole migration story; no version stamp, no separate migration
  step.
- One block per repo. The patterns are identical for every agent (bridge
  filenames don't vary), so the block needs no per-agent bookkeeping.
- Uses the existing atomic temp-file+rename pattern (`write_atomic` from
  damon-core, or a damon-git equivalent) — `info/exclude` is a user-repo
  file; a torn write there is not acceptable.

Cleanup (`agent rm` in `commands/agent.rs`):

- After removing a worktree-source agent, scan the store for any other
  agent whose source resolves to the same repo (compare canonicalized
  git common dirs, not raw config strings). If none remain, rewrite that
  repo's `info/exclude` with the damon block removed. Non-damon lines
  are untouched. Failures here warn and continue — cleanup must never
  block agent removal.

README: document the residual limitation — while a damon worktree agent
exists for a repo, untracked files matching the bridge names
(`CLAUDE.md`, `AGENTS.md`, `.claude/…`) are hidden from `git status` in
that repo's *other* worktrees.

Tests (real git repos in tempdirs, existing damon-git convention):
fresh-file block append; second write is byte-identical (idempotence);
user lines above/below the block preserved; legacy unmarked damon lines
migrated into the block; cleanup removes only the block; cleanup skipped
while another agent still references the repo.

## 4. Homebrew private tap

- New repo **`donnie-ccama/homebrew-damon`**, containing
  `Formula/damon.rb`:
  - `head "https://github.com/donnie-ccama/damon.git"` — git transport,
    authenticated by the user's existing git credentials (works for a
    private repo without token plumbing).
  - `depends_on "rust" => :build`; install via
    `system "cargo", "install", *std_cargo_args(path: "crates/damon")`.
  - `test do` block runs `damon --version` (verified working today:
    clap's `version` attribute is set; prints `damon 0.1.0`).
- Install command: `brew install --HEAD donnie-ccama/damon/damon`.
  HEAD-only is deliberate: the repo is private and untagged; a versioned
  `url`/`sha256` stanza is added when a `v0.1.0` tag is cut (recorded as
  the release procedure, not done in M4).
- `docs/PACKAGING.md` in the damon repo records: tap layout, the install
  command, the future tagged-release procedure, and the note that AUR is
  deferred.
- Verification: an actual `brew install --HEAD` + `brew test damon` run
  on this machine is the acceptance test. No CI for the tap.

## 5. Parked M3 debt (five fixes)

1. **`write_atomic` temp cleanup** (damon-core): when the rename step
   fails, best-effort `remove_file` the temp file; the original rename
   error is still the one reported. Test: rename-failure path leaves no
   temp file behind (simulate by making the destination a directory).
2. **Popup `TestBackend` coverage** (tui): rendering tests for the
   ModelPicker and NewAgent popups, plus an assertion that the selected
   row carries the `REVERSED` style. Same fixture style as the existing
   rail/tab view tests.
3. **`ensure_selection` resets `mem_idx`** (tui/app): when the rail
   selection changes, the memory-preview cursor returns to 0. Test at
   the Model/update level.
4. **Unify the live-session loop** (tui/snapshot): extract the
   session-row derivation shared by `load_world` (production tmux path)
   and the test-fixture loop into one function with two callers. Pure
   refactor; existing tests keep passing.
5. **`N` on an empty rail** (tui): sets a status-line hint —
   ``no teams — run `damon team new` first`` — instead of a silent
   no-op. Test: Model/update emits the hint when teams are empty.

## Build order

1. Debt fixes (§5) — small, independent, clears the ledger first.
2. Doctor refactor (§2).
3. `info/exclude` block + cleanup (§3).
4. `damon memory` (§1).
5. Homebrew tap (§4) — last, so the formula snapshots a green tree.

Each step lands green (`cargo fmt --check`, `clippy` with zero warnings
workspace-wide, `cargo test`) before the next begins, per the parent
spec's milestone rule.

## Out of scope

- AUR packaging (deferred, unchanged).
- Tagged/versioned Homebrew releases (procedure documented, first tag
  cut post-M4).
- TUI memory *editing* (the CLI `--edit` core is TUI-reusable, but no
  TUI wiring in M4).
