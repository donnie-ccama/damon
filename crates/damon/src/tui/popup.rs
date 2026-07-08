use crate::tui::app::{Action, Model};
use ratatui::crossterm::event::KeyEvent;

#[derive(Debug, PartialEq)]
pub enum Popup {}

pub fn update_popup(_m: &mut Model, _key: KeyEvent) -> Vec<Action> {
    Vec::new() // Task 11
}
