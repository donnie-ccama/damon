# Damon M6 — TUI preview & suspend hardening (design)

Clears the M5 parked debt around the TUI preview pane and the
suspend/resume editor boundary. Four small, cohesive changes, all in
`crates/damon/src/tui/`. No CLI, config, or packaging changes.

This spec is the source of truth for M6. Where it conflicts with the
M5 as-built addendum's "Next milestone" sketch, this spec wins.

## Scope

In:

1. Viewport-aware preview scroll clamp (wrap-accurate).
2. Live-refresh preview content after an in-place `e` edit.
3. Harden `suspend()` so no error path leaves a mixed terminal state.
4. `TestBackend`-reachable tests for the pure pieces of the above.

Out:

- AUR live publish — a user-run, on-Arch handoff (`makepkg`/`namcap`/
  `git push` to `aur.archlinux.org`). macOS cannot run those tools.
  Tracked separately; not part of this milestone's code.
- Mocking the real suspend/resume terminal boundary behind a trait.
  Rejected as over-engineering for a ~12-line function; that path stays
  a documented manual pty smoke test.

Files touched: `tui/app.rs`, `tui/view.rs`, `tui/mod.rs`, and the
workspace `Cargo.toml` (one feature flag).

## 1. Viewport-aware preview scroll clamp

### Problem

`update_preview` (`crates/damon/src/tui/app.rs`) clamps the scroll
offset to `content.lines().count().saturating_sub(1)`. This has two
faults:

- It ignores the pane height, so the user can scroll until the last
  logical line sits at the *top* of the pane, leaving the rest of the
  pane empty below.
- It counts *logical* lines, but the pane renders with
  `Wrap { trim: false }`, so a long line occupies several visual rows.
  The scroll offset passed to `Paragraph::scroll` is in *wrapped* rows,
  so a logical-line clamp can never reach the true bottom of wrapped
  content.

### Approach

The pane's dimensions are known only at render time (`render_memory`
in `tui/view.rs`, which takes `&Model`). We compute the exact maximum
scroll there — wrap-accurate — and stash it on the `Preview` for the
next `update_preview` call to clamp against.

`Preview` gains a `max_scroll: std::cell::Cell<u16>` field. `Cell`
gives interior mutability so `render`'s `&Model` signature is
unchanged. `Preview` derives only `Debug` today, and `Cell<u16>: Debug`,
so no derive breaks.

Wrap-accurate row count uses `Paragraph::line_count(width)`, gated
behind ratatui's `unstable-rendered-line-info` feature (see below).

**Critical detail — how `line_count` treats the block.** In ratatui
0.29, `Paragraph::line_count(width)` wraps the text at exactly the
`width` you pass (it does **not** subtract horizontal border columns),
then *adds* the block's top/bottom border rows to the result. To get a
clean count of content rows we therefore call it on a **blockless**
paragraph at the **inner** width and do the border arithmetic
ourselves:

```rust
// in render_memory, before rendering the bordered preview paragraph
let inner_w = area.width.saturating_sub(2);   // left + right border
let inner_h = area.height.saturating_sub(2);  // top + bottom border
let total_rows = Paragraph::new(p.content.clone())
    .wrap(Wrap { trim: false })
    .line_count(inner_w) as u16;
p.max_scroll.set(total_rows.saturating_sub(inner_h));
```

The bordered paragraph is still constructed and rendered exactly as
today; the blockless one exists only to measure. Constructing a second
`Paragraph` per frame is cheap.

`update_preview` then clamps against the stored ceiling:

```rust
let max = p.max_scroll.get();
// Up/PageUp: saturating_sub as today (floor at 0)
// Down:      p.scroll = p.scroll.saturating_add(1).min(max)
// PageDown:  p.scroll = p.scroll.saturating_add(10).min(max)
```

The old `let max = p.content.lines().count()...` line is removed.

**One-frame lag.** `update_preview` reads the `max_scroll` written by
the *previous* render. The event loop draws before it reads input, so
by the time any scroll key is processed the preview has rendered at
least once and `max_scroll` is populated. On the very first frame the
Cell holds its initial `0`; a `min(0)` simply pins scroll at the top,
which is correct for a freshly opened preview. Accepted.

### Construction sites

`Preview` is built in two places; both gain `max_scroll: Cell::new(0)`:

- `Action::Preview` handler in `tui/mod.rs`.
- The `memory_tab_lists_files_and_preview_renders_content` test in
  `tui/view.rs`.

### The ratatui unstable feature

Workspace `Cargo.toml`:

```toml
ratatui = { version = "0.29", features = ["unstable-rendered-line-info"] }
```

`unstable-rendered-line-info` is a plain cargo feature (`= []`) — it
compiles on stable Rust; "unstable" here means ratatui reserves the
right to change the API/behavior across versions, not that it needs a
nightly toolchain. ratatui is already pinned to `0.29` and the lock
file is committed, so an accidental upgrade can't silently change
behavior. Risk is accepted per the M6 scoping decision; item 4 adds a
guard test (below) so a deliberate ratatui bump that changes wrapping
semantics fails loudly.

## 2. Live-refresh preview after in-place `e` edit

### Problem

The `Action::Edit { path }` arm in `tui/mod.rs`'s event loop suspends,
runs the editor, sets a status, and reloads the world snapshot
(`refresh = true`). It never re-reads `m.preview.content`, so when the
edited file is the one currently previewed the pane shows pre-edit text
until it is closed and reopened.

### Approach

After a *successful* editor exit, if a preview is open and its `path`
equals the edited path, re-read the file into `preview.content`. Extract
the logic into a pure, unit-testable helper (free function in `app.rs`)
so it can be exercised against a temp file without the suspend boundary:

```rust
/// Re-read `path` into the open preview if it is the one being shown.
/// On read failure, keep the (now stale) pane open and return an error
/// string for the status line — mirrors the Action::Preview handler.
fn refresh_preview(m: &mut Model, path: &Path) -> Result<(), String>
```

Behavior on re-read failure: **keep the stale pane open** and surface
the error in `m.status` (consistent with the existing `Action::Preview`
error handling). Do not auto-close the pane. (Confirmed in scoping.)

On success, clamp `preview.scroll` against the current `max_scroll`
Cell value. That value may be one frame stale relative to the new
content, but the next render recomputes it and the subsequent key
re-clamps, so scroll can never be left visibly out of range for more
than the moment before the next draw.

The event loop's `Action::Edit` arm calls `refresh_preview` after a
successful edit and folds any returned error into the status it already
sets. The `refresh = true` world reload is unchanged.

## 3. Harden `suspend()` early-return

### Problem

`suspend()` in `tui/mod.rs`:

```rust
disable_raw_mode()?;
execute!(stdout(), LeaveAlternateScreen)?;   // <- early return here
let out = f();
execute!(stdout(), EnterAlternateScreen)?;
enable_raw_mode()?;
terminal.clear()?;
```

If `LeaveAlternateScreen` fails, raw mode is already disabled and the
`?` returns `Err` with the terminal in a mixed state (raw off, possibly
still on the alternate screen). The caller in the event loop **catches
this error into a status string and keeps looping**, so the next
`terminal.draw` renders into that broken terminal.

### Approach

Guarantee that every error path leaves the terminal in the
TUI-expected state (raw mode on + alternate screen on), because the
loop continues drawing after `suspend` returns:

```rust
fn suspend<T>(terminal, f) -> std::io::Result<T> {
    // Leave TUI mode.
    disable_raw_mode()?;
    if let Err(e) = execute!(stdout(), LeaveAlternateScreen) {
        let _ = enable_raw_mode(); // restore TUI-expected state before bail
        return Err(e);
    }

    let out = f();

    // Restore TUI mode — attempt both steps, report the first failure.
    let enter = execute!(stdout(), EnterAlternateScreen);
    let raw = enable_raw_mode();
    enter.and(raw)?;
    terminal.clear()?;
    Ok(out)
}
```

The `disable_raw_mode()?` on the very first line can still early-return,
but at that point nothing has changed yet — the terminal is still in
its normal TUI state, which is consistent. Every *later* failure now
either restores raw mode (the Leave path) or best-effort restores both
screen and raw mode (the return path) before propagating.

## 4. `TestBackend`-reachable tests

The real suspend/resume boundary touches actual stdout and is not
`TestBackend`-mockable; abstracting it behind a trait is out of scope
(see Scope). We test the pure pieces instead:

- **`update_preview` emits `Edit` on `e`:** with a preview open,
  pressing `e` returns `vec![Action::Edit { path }]` with the previewed
  path. Pure `Model`→`Vec<Action>`; no suspend.
- **Scroll clamp respects `max_scroll`:** set `preview.max_scroll` to a
  known value, drive `Down`/`PageDown` past it, assert `scroll` never
  exceeds it; drive `Up` past the top, assert it floors at 0.
- **`refresh_preview` re-reads updated content:** write a temp file,
  open a `Preview` on it, rewrite the file, call `refresh_preview`,
  assert `preview.content` reflects the new bytes. Then point at a
  missing/unreadable path and assert the pane stays open and an `Err`
  string is returned.
- **ratatui wrap guard:** a focused unit test asserting
  `Paragraph::new(<known text>).wrap(Wrap { trim:false }).line_count(w)`
  returns the expected wrapped-row count at a chosen width. This pins
  the `line_count` semantics we depend on, so a future ratatui upgrade
  that changes wrapping fails here rather than silently mis-clamping.

The suspend boundary itself remains the documented manual pty smoke
test (`EDITOR=true` / `EDITOR=false`) from M5.

## Testing summary

`cargo test` (workspace) plus `cargo clippy`. New tests live alongside
the existing `tui/app.rs` and `tui/view.rs` test modules. The manual
pty smoke test for the editor suspend path is re-run once by hand.

## Risks

- **ratatui `line_count` is unstable API.** Mitigated by the pinned
  `0.29` + committed lock file and the wrap guard test in item 4.
- **One-frame lag on `max_scroll`.** Analyzed above; bounded to a single
  pre-draw moment and never produces a visibly wrong frame.
