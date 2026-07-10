# Damon M7 — Tokyo Night theme (design)

Give the ratatui TUI a cohesive **Tokyo Night** ("Night" variant),
**accent-only** color scheme: the terminal's own background is left
alone (transparency-friendly), and the palette lands on borders,
titles, selection, tabs, badges, and status accents. One new module
holds the palette; the render layer is refactored to pull from it.

This spec is the source of truth for M7.

## Scope

In:

- New `crates/damon/src/tui/theme.rs`: Tokyo Night palette as named
  `Color::Rgb` constants + semantic style helpers.
- Refactor `crates/damon/src/tui/view.rs` (and its popup/form/error
  rendering) to use the helpers instead of inline style literals.
- `mod theme;` in `crates/damon/src/tui/mod.rs`.
- 1–2 `TestBackend` style-assertion tests pinning that the theme is
  applied.

Out (YAGNI / non-goals):

- Runtime theme switching or a `theme` config key. A single hardcoded
  theme, but structured (semantic helpers) so a second could be added
  later without touching call sites.
- `NO_COLOR` handling. The current code already uses color
  unconditionally; this milestone does not change that.
- Truecolor is assumed (Ghostty, the target terminal, supports it;
  ratatui downsamples `Rgb` on 256-color terminals automatically).
- Painting pane/body backgrounds ("immersive" mode) — explicitly
  rejected in scoping in favor of accent-only.

Files touched: new `tui/theme.rs`, `tui/view.rs`, `tui/mod.rs`.

## The module: `tui/theme.rs`

Palette constants (only colors actually used are defined, to avoid
dead-code warnings on a crate-private module):

```rust
use ratatui::style::{Color, Modifier, Style};

const BG_HIGHLIGHT: Color = Color::Rgb(0x29, 0x2e, 0x42); // selection bar bg
const MUTED: Color       = Color::Rgb(0x56, 0x5f, 0x89); // comment/muted
const BLUE: Color        = Color::Rgb(0x7a, 0xa2, 0xf7);
const CYAN: Color        = Color::Rgb(0x7d, 0xcf, 0xff);
const GREEN: Color       = Color::Rgb(0x9e, 0xce, 0x6a);
const MAGENTA: Color     = Color::Rgb(0xbb, 0x9a, 0xf7);
const RED: Color         = Color::Rgb(0xf7, 0x76, 0x8e);
const ORANGE: Color      = Color::Rgb(0xff, 0x9e, 0x64);
```

Semantic helpers (each returns a `Style`; the call sites read as
intent). Expose `SELECTION_BG` (and any palette constant a test needs)
as `pub(crate)` so the style-assertion test can name the expected
selection background; keep the rest private:

```rust
pub(crate) const SELECTION_BG: Color = BG_HIGHLIGHT;
pub(crate) const BORDER_FG: Color = BLUE;

pub fn border() -> Style        { Style::default().fg(BLUE) }
pub fn title() -> Style         { Style::default().fg(BLUE).add_modifier(Modifier::BOLD) }
pub fn selection() -> Style     { Style::default().bg(BG_HIGHLIGHT).fg(MAGENTA).add_modifier(Modifier::BOLD) }
pub fn tab_active() -> Style    { Style::default().fg(CYAN).add_modifier(Modifier::BOLD | Modifier::UNDERLINED) }
pub fn tab_inactive() -> Style  { Style::default().fg(MUTED) }
pub fn team() -> Style          { Style::default().fg(BLUE).add_modifier(Modifier::BOLD) }
pub fn header() -> Style        { Style::default().fg(BLUE).add_modifier(Modifier::BOLD) }
pub fn badge() -> Style         { Style::default().fg(GREEN) }
pub fn model_col() -> Style     { Style::default().fg(CYAN) }
pub fn uptime_col() -> Style    { Style::default().fg(MUTED) }
pub fn invalid() -> Style       { Style::default().fg(RED) }
pub fn hint() -> Style          { Style::default().fg(MUTED) }
pub fn status_msg() -> Style    { Style::default().fg(ORANGE) }
pub fn status_error() -> Style  { Style::default().fg(RED) }
pub fn error_block() -> Style   { Style::default().fg(RED) }
```

## Role mapping (what the refactor changes in `view.rs`)

| Element | Today | M7 |
|---|---|---|
| Block borders (rail, sessions, memory, preview, popups) | default | `theme::border()` — blue `#7aa2f7` |
| Block titles | default | `theme::title()` — blue bold |
| Selection (rail, memory list, model picker) | `Modifier::REVERSED` | `theme::selection()` — bg `#292e42` + magenta fg, bold |
| Active tab | bold+underline | `theme::tab_active()` — cyan bold+underline |
| Inactive tabs | default | `theme::tab_inactive()` — muted (set as `Tabs::style`) |
| Team name (rail) | bold | `theme::team()` — blue bold |
| Agent name (rail) | default | unchanged (default fg) |
| Session badge `●N` | `Color::Green` | `theme::badge()` — palette green `#9ece6a` |
| Model column (sessions table) | default | `theme::model_col()` — cyan |
| Uptime column | default | `theme::uptime_col()` — muted |
| Table header row | bold | `theme::header()` — blue bold |
| INVALID entries / strays | `Color::Red` | `theme::invalid()` — palette red `#f7768e` |
| Status line — idle hint | default | `theme::hint()` — muted |
| Status line — active message | default | orange (`status_msg()`) — see error special-case below |
| Error screen border + title | default | `theme::error_block()` — red |

### Selection preserves semantic color

`selection()` sets the row's **base** style (bg + magenta fg + bold).
Each row's own styled spans — the green badge, a red INVALID — keep
their fg because in ratatui a span's style patches over the list/base
style for that span's cells. So a selected live agent still shows a
green badge on the magenta bar, and a selected INVALID stays red; the
highlight never clobbers a semantic color. The `selected()` helper in
`view.rs` changes from applying `Modifier::REVERSED` to applying
`theme::selection()`.

### Status line: error special-case

`render_status` has only the status string. All error statuses in the
codebase are built with an `"error: …"` prefix (`execute_action`'s
`Open`/`Kill`/`CreateAgent`/`Preview` arms and the `Action::Edit`
arm all `format!("error: …")`). So:

- No active status (idle) → render the hint text with `theme::hint()`.
- Active status starting with `"error"` → `theme::status_error()` (red).
- Any other active status → `theme::status_msg()` (orange).

`text.starts_with("error")` is the discriminator. A partial-success
message like `"edited X (preview: <err>)"` starts with `"edited"`, so
it renders orange — correct, it's not a hard error. This is a
localized content check in `render_status`; no status-set site changes.

### Table cell coloring

The sessions table currently builds `Row::new(vec![name, model,
uptime])` from plain strings. To color the model and uptime columns,
wrap those two in styled cells, e.g.
`Cell::from(Span::styled(s.model.clone(), theme::model_col()))` and
`Cell::from(Span::styled(fmt_uptime(...), theme::uptime_col()))`; the
name cell stays a plain string. The header row uses `theme::header()`.

## Testing

The existing `TestBackend` tests assert on rendered **text**
(`buffer_text` + `.contains`), so they are unaffected by color changes
and must still pass.

Add focused **style**-assertion tests in `view.rs`'s test module that
read the rendered buffer's cell styles (a `TestBackend` cell exposes
`.fg` / `.bg` / `.style()`):

1. **Selection bar applied:** render a `Model` with a rail selection;
   scan the buffer for a cell whose `bg == theme::SELECTION_BG`
   (`#292e42`); assert at least one exists. This pins that selection is
   a themed bar, not `REVERSED`.
2. **Borders themed:** scan for a cell whose `fg == theme::BORDER_FG`
   (blue); assert at least one exists (borders render on every frame).

A small buffer-scan helper (`fn any_cell_matches(backend, pred) ->
bool`) keeps these robust to exact layout. Reference the palette via
the `pub(crate)` constants the theme module exposes so the tests and
the module can't drift.

`cargo test -p damon` green and `cargo clippy -p damon --all-targets`
with no warnings (note: no `[lib]` target, so `--lib` is not usable —
use `cargo test -p damon`).

## Risks

- **Content-sniffing the status prefix** couples `render_status` to the
  `"error: "` convention. Mitigated: it's the single existing
  convention across all status-set sites, and the fallback (orange) is
  harmless if a future non-error message ever began with "error".
- **Truecolor assumption.** On a non-truecolor terminal ratatui
  downsamples `Rgb`; colors approximate rather than break. Ghostty
  (the documented target) is truecolor.
