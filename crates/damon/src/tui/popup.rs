//! Popups: at most one open at a time; keys route here while one is open.
use crate::tui::app::{Action, Model};
use ratatui::crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug, PartialEq)]
pub enum Popup {
    ModelPicker(ModelPicker),
    ConfirmKill { reference: String, count: usize },
    NewAgent(NewAgentForm),
}

#[derive(Debug, PartialEq)]
pub struct ModelPicker {
    pub reference: String,
    pub models: Vec<(String, String)>,
    pub selected: usize,
}

#[derive(Debug, PartialEq)]
pub struct NewAgentForm; // Task 12

pub fn update_popup(m: &mut Model, key: KeyEvent) -> Vec<Action> {
    // take/put-back: mutating the popup AND m.popup in one pass would be a
    // double mutable borrow of m.popup.
    let Some(mut popup) = m.popup.take() else {
        return Vec::new();
    };
    let mut actions = Vec::new();
    let mut keep = true;
    match &mut popup {
        Popup::ModelPicker(p) => match key.code {
            KeyCode::Esc => keep = false,
            KeyCode::Up | KeyCode::Char('k') => p.selected = p.selected.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                p.selected = (p.selected + 1).min(p.models.len().saturating_sub(1))
            }
            KeyCode::Enter => {
                actions.push(Action::Open {
                    reference: p.reference.clone(),
                    model: p.models.get(p.selected).map(|(k, _)| k.clone()),
                    fresh: true,
                });
                keep = false;
            }
            _ => {}
        },
        Popup::ConfirmKill { reference, .. } => match key.code {
            KeyCode::Char('y') | KeyCode::Enter => {
                actions.push(Action::Kill {
                    reference: reference.clone(),
                });
                keep = false;
            }
            KeyCode::Char('n') | KeyCode::Esc => keep = false,
            _ => {}
        },
        Popup::NewAgent(_) => {
            if key.code == KeyCode::Esc {
                keep = false; // Task 12 replaces this arm
            }
        }
    }
    if keep {
        m.popup = Some(popup);
    }
    actions
}
