# Damon M8 — rename to **Cortado** (design)

Rename the whole project from `damon` to `cortado` (display: **Cortado**):
5 crates, ~40 source files, 14 env-var seams, all string literals, docs,
packaging, and the external release surface (GitHub repo, Homebrew tap,
AUR, a new tag). A **clean hard rename** — no migration code; the user
re-inits locally afterward.

This spec is the source of truth for M8. It is the last dated
`damon`-named artifact; it is intentionally not renamed (see Historical
records).

## Decisions (settled in scoping)

- **Clean hard rename.** No migration shims, no `migrate` command. After
  M8 the user runs `cortado init` (regenerates config), `cortado key set`
  (re-stores keys under the new keyring service), lets old `damon_*` tmux
  sessions die, and clears any orphaned `# damon begin/end` blocks in
  existing worktrees by hand.
- **Full rebrand incl. re-publish.** In-repo rename PLUS GitHub repo
  rename, a new `v0.2.0` tag, Homebrew tap rename + formula re-publish,
  and AUR file updates (live AUR push stays an on-Arch user handoff).
- **Controller-scripted execution** (mechanical global rename), verified
  by the full test suite + clippy + a zero-stragglers grep, then an opus
  whole-branch review. Not subagent-driven — there is no design work,
  only a faithful, complete substitution.
- **Version `v0.2.0`** (minor bump from `v0.1.0` for a rebrand).

## Naming convention

Lowercase `cortado` for every identifier/path; capitalized **Cortado**
for human-facing display text.

| Aspect | `damon` | `cortado` |
|---|---|---|
| Crates | `damon`, `damon-core`, `damon-git`, `damon-term`, `damon-tmux` | `cortado`, `cortado-core`, `cortado-git`, `cortado-term`, `cortado-tmux` |
| Binary / CLI | `damon` | `cortado` |
| Rust module paths | `damon_core::`, `damon_tmux::`, `damon_git::`, `damon_term::` | `cortado_core::`, `cortado_tmux::`, `cortado_git::`, `cortado_term::` |
| Env-var seams (14) | `DAMON_*` (`DAMON_ROOT`, `DAMON_CONFIG_DIR`, `DAMON_NO_KEYRING`, `DAMON_MODEL`, `DAMON_BIN_CLAUDE`, `DAMON_BIN_OPENCODE`, `DAMON_CLAUDE_ARGS`, `DAMON_OPENCODE_ARGS`, `DAMON_KEY_*`, `DAMON_AGENT`, `DAMON_TEAM`, `DAMON_SESSION`, `DAMON_BIN_*`) | `CORTADO_*` |
| tmux socket | `"damon"` | `"cortado"` |
| Session prefix | `damon_{team}_{agent}_{n}` | `cortado_{team}_{agent}_{n}` |
| tmux user option | `@damon_model` | `@cortado_model` |
| Config dir | `…/damon` (`~/.config/damon` and macOS `Application Support/damon`) | `…/cortado` |
| Sidecars | `.{file}.damon-tmp`, `.damon-exclude.lock` | `.{file}.cortado-tmp`, `.cortado-exclude.lock` |
| Exclude markers | `# damon begin` / `# damon end` | `# cortado begin` / `# cortado end` |
| Keyring service | `damon` | `cortado` |
| Bridge Stop-hook cmd | `damon hook reflect` | `cortado hook reflect` |
| Display (README, TUI titles e.g. "damon — error") | damon | **Cortado** |

The substitution is exactly three case-aware forms applied everywhere in
scope: `damon`→`cortado`, `Damon`→`Cortado`, `DAMON`→`CORTADO`. Because
"damon" names nothing but this application, the replace is unambiguous.

## In-repo scope

1. **Crate directories:** `git mv crates/damon-core crates/cortado-core`
   (and the other four, incl. `crates/damon` → `crates/cortado`).
2. **Cargo manifests (6):** rename each `[package] name`, every path
   dependency (`damon-core = { path = "../damon-core" }` →
   `cortado-core = { path = "../cortado-core" }`), the workspace
   `members` list, and any `[[bin]]`/default-run name. Regenerate
   `Cargo.lock`.
3. **Rust source (~40 files):** all `use damon_core::` / `damon_tmux::`
   etc. module paths; all `DAMON_*` env-var names; all `"damon"` /
   `"damon_"` / `@damon_model` / `.damon-tmp` / `.damon-exclude.lock` /
   `# damon` string literals; the `damon hook reflect` bridge command;
   test names and fixtures that embed the name (e.g.
   `attach_command_targets_damon_socket`, `DAMON_KEY_DAMONTEST`,
   `damon_newsletter_scout_3`).
4. **README.md** and any user-facing doc/heading → **Cortado**.
5. **packaging/aur:** `PKGBUILD` (pkgname/pkgbase/url), `.SRCINFO`,
   `PUBLISHING.md`.

### Historical records (deliberately NOT renamed)

- `docs/superpowers/plans/*` and `docs/superpowers/specs/*` — dated
  build-log records of what shipped **as damon** at the time. Rewriting
  them would falsify history. Instead, append a one-line note to the
  orchestrator design doc
  (`docs/superpowers/specs/2026-07-07-damon-orchestrator-design.md`):
  "Project renamed damon → Cortado in M8 (2026-07-09); identifiers below
  reflect the pre-rename name."
- `.superpowers/sdd/` ledger, git commit history, `target/`, and the
  binary data files (`agentdb.rvf`, `ruvector.db`) — untouched.

## Execution approach (staged, controller-driven)

Each stage ends by confirming the tree still builds/tests, so a mistake
surfaces immediately rather than compounding:

1. **Branch** `m8-rename-cortado` off `main`.
2. **Crate dirs:** `git mv` all five. `cargo build` will fail (paths) —
   expected; proceed to stage 3 before re-testing.
3. **Cargo manifests:** rename package names, path deps, workspace
   members. `cargo build` — expect it to compile the manifest graph but
   fail on `use damon_core::` paths in source.
4. **Identifiers + literals:** the three case-aware replaces across all
   Rust source + README + packaging (excluding the Historical-records
   paths, `.git`, `target`, `Cargo.lock`). `cargo build` → clean.
5. **Regenerate `Cargo.lock`** (`cargo build` does this) and commit.
6. **Verify:** `cargo test` (all crates green), `cargo clippy
   --all-targets` (clean), and the zero-stragglers grep (below).
7. **Historical note** appended to the orchestrator design doc.
8. Commit (one or a few logical commits on the branch).

## Verification

- `cargo test` — full workspace, 0 failed, under the new names. Every
  test that referenced a `damon_*` prefix / `DAMON_*` env / `"damon"`
  literal is renamed and must still pass.
- `cargo clippy --all-targets` — no warnings.
- **Zero-stragglers grep:** `grep -rIi damon` over tracked files,
  excluding `docs/superpowers/plans`, `docs/superpowers/specs`,
  `.superpowers/`, `.git`, `target` — must return **nothing** in code,
  packaging, or README. This is the completeness proof.
- `cargo run -p cortado -- --help` prints `cortado` usage; the binary
  artifact is named `cortado`.
- Opus whole-branch review of the complete diff before merge.

## External / operational sequence (after the in-repo branch is reviewed green)

Executed by the controller unless marked handoff, in order:

1. **Merge** `m8-rename-cortado` → `main` (fast-forward).
2. **Rename the GitHub repo** `donnie-ccama/damon` → `cortado`
   (`gh repo rename cortado`); update local `origin` URL. GitHub
   auto-redirects old URLs, so existing `--HEAD` installs keep resolving.
3. **Push** `main` to origin (`cortado`).
4. **Tag `v0.2.0`** at the merge commit; push the tag.
5. **Homebrew tap:** rename `donnie-ccama/homebrew-damon` →
   `homebrew-cortado`; `git mv Formula/damon.rb Formula/cortado.rb`;
   update the formula `class Damon` → `class Cortado`, `url` (new repo +
   `v0.2.0` tarball), and `sha256` (new tarball); `brew audit --strict
   --online` + `brew install donnie-ccama/cortado/cortado` + `brew test`.
6. **AUR:** update in-repo `packaging/aur/PKGBUILD` (pkgname/pkgbase →
   `cortado`, url, `pkgver=0.2.0`) and regenerate `.SRCINFO`. The live
   `makepkg`/`namcap`/`git push ssh://aur@aur.archlinux.org/cortado.git`
   is an **on-Arch user handoff** (macOS cannot run it).

## Stop point

The controller performs everything through the external sequence above
(code renamed + merged + repo renamed + pushed + tagged + Homebrew
re-published + AUR files updated), then **STOPS before the final QA
test**. The user runs the final QA in a **clean chat**: fresh
`brew install donnie-ccama/cortado/cortado`, `cortado init`, `cortado
ui`, and a full smoke of the rebranded app. The AUR live publish is the
user's on-Arch handoff, done alongside or after that QA.

## Risks

- **Incomplete rename** (a stray `damon` in a code path) — caught by the
  zero-stragglers grep gate; a milestone cannot pass with any straggler
  outside the historical-records exclusions.
- **Repo rename breaking the tap/AUR URLs** — mitigated by GitHub's
  automatic old-URL redirect and by updating the formula/PKGBUILD `url`
  to the new name + `v0.2.0` tarball explicitly.
- **Orphaned local state** (old config/keyring/tmux/markers) — accepted
  per the clean-hard-rename decision; the user re-inits.
