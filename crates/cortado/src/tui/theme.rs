//! Cortado's "espresso at night" palette and semantic styles.
//!
//! The terminal owns the canvas; we paint only selection and popup surfaces so
//! the UI remains compatible with user Ghostty themes and transparency.
use ratatui::style::{Color, Modifier, Style};

const INK: Color = Color::Rgb(0x1a, 0x1b, 0x26);
const SELECTION: Color = Color::Rgb(0x32, 0x2b, 0x38);
const TEXT: Color = Color::Rgb(0xe7, 0xdf, 0xd5);
const MUTED: Color = Color::Rgb(0x78, 0x79, 0x8c);
const BORDER: Color = Color::Rgb(0x45, 0x46, 0x58);
const CORTADO: Color = Color::Rgb(0xd7, 0x9a, 0x68);
const LAVENDER: Color = Color::Rgb(0xa9, 0xa1, 0xd6);
const SAGE: Color = Color::Rgb(0x9f, 0xc0, 0x87);
const ROSE: Color = Color::Rgb(0xd9, 0x7b, 0x87);
const GOLD: Color = Color::Rgb(0xe3, 0xb3, 0x6b);

#[cfg(test)]
pub(crate) const SELECTION_BG: Color = SELECTION;
#[cfg(test)]
pub(crate) const BORDER_FG: Color = BORDER;
#[cfg(test)]
pub(crate) const TEAM_FG: Color = LAVENDER;

pub fn border() -> Style {
    Style::default().fg(BORDER)
}
pub fn border_focused() -> Style {
    Style::default().fg(CORTADO)
}
pub fn title() -> Style {
    Style::default().fg(TEXT).add_modifier(Modifier::BOLD)
}
pub fn brand() -> Style {
    Style::default().fg(CORTADO).add_modifier(Modifier::BOLD)
}
pub fn selection() -> Style {
    Style::default()
        .bg(SELECTION)
        .fg(TEXT)
        .add_modifier(Modifier::BOLD)
}
pub fn selection_marker() -> Style {
    Style::default().fg(CORTADO).add_modifier(Modifier::BOLD)
}
pub fn tab_active() -> Style {
    Style::default()
        .fg(CORTADO)
        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
}
pub fn tab_inactive() -> Style {
    Style::default().fg(MUTED)
}
pub fn team() -> Style {
    Style::default().fg(LAVENDER).add_modifier(Modifier::BOLD)
}
pub fn header() -> Style {
    Style::default().fg(MUTED).add_modifier(Modifier::BOLD)
}
pub fn primary() -> Style {
    Style::default().fg(TEXT)
}
pub fn muted() -> Style {
    Style::default().fg(MUTED)
}
pub fn badge() -> Style {
    Style::default().fg(SAGE).add_modifier(Modifier::BOLD)
}
pub fn model_col() -> Style {
    Style::default().fg(LAVENDER)
}
pub fn invalid() -> Style {
    Style::default().fg(ROSE).add_modifier(Modifier::BOLD)
}
pub fn hint() -> Style {
    Style::default().fg(LAVENDER)
}
pub fn key() -> Style {
    Style::default().fg(CORTADO).add_modifier(Modifier::BOLD)
}
pub fn status_msg() -> Style {
    Style::default().fg(SAGE)
}
pub fn status_warn() -> Style {
    Style::default().fg(GOLD)
}
pub fn status_error() -> Style {
    Style::default().fg(ROSE)
}
pub fn error_block() -> Style {
    Style::default().fg(ROSE)
}
pub fn popup() -> Style {
    Style::default().bg(INK)
}
