//! Tokyo Night ("Night") palette + semantic style helpers. Accent-only:
//! nothing here paints a pane background except the selection bar.
use ratatui::style::{Color, Modifier, Style};

const BG_HIGHLIGHT: Color = Color::Rgb(0x29, 0x2e, 0x42);
const MUTED: Color = Color::Rgb(0x56, 0x5f, 0x89);
/// MUTED at ~3x perceptual luminance: the bottom key menu must stay readable.
const HINT: Color = Color::Rgb(0x8e, 0x9d, 0xe2);
const BLUE: Color = Color::Rgb(0x7a, 0xa2, 0xf7);
const CYAN: Color = Color::Rgb(0x7d, 0xcf, 0xff);
const GREEN: Color = Color::Rgb(0x9e, 0xce, 0x6a);
const MAGENTA: Color = Color::Rgb(0xbb, 0x9a, 0xf7);
const RED: Color = Color::Rgb(0xf7, 0x76, 0x8e);
const ORANGE: Color = Color::Rgb(0xff, 0x9e, 0x64);

/// Exposed for the view's style-assertion tests.
pub(crate) const SELECTION_BG: Color = BG_HIGHLIGHT;
pub(crate) const BORDER_FG: Color = BLUE;

pub fn border() -> Style {
    Style::default().fg(BORDER_FG)
}
pub fn title() -> Style {
    Style::default().fg(BLUE).add_modifier(Modifier::BOLD)
}
pub fn selection() -> Style {
    Style::default()
        .bg(SELECTION_BG)
        .fg(MAGENTA)
        .add_modifier(Modifier::BOLD)
}
pub fn tab_active() -> Style {
    Style::default()
        .fg(CYAN)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
}
pub fn tab_inactive() -> Style {
    Style::default().fg(MUTED)
}
pub fn team() -> Style {
    Style::default().fg(BLUE).add_modifier(Modifier::BOLD)
}
pub fn header() -> Style {
    Style::default().fg(BLUE).add_modifier(Modifier::BOLD)
}
pub fn badge() -> Style {
    Style::default().fg(GREEN)
}
pub fn model_col() -> Style {
    Style::default().fg(CYAN)
}
pub fn uptime_col() -> Style {
    Style::default().fg(MUTED)
}
pub fn invalid() -> Style {
    Style::default().fg(RED)
}
pub fn hint() -> Style {
    Style::default().fg(HINT)
}
pub fn status_msg() -> Style {
    Style::default().fg(ORANGE)
}
pub fn status_error() -> Style {
    Style::default().fg(RED)
}
pub fn error_block() -> Style {
    Style::default().fg(RED)
}
