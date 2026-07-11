//! Popups: at most one open at a time; keys route here while one is open.
use crate::commands::agent::RepoArg;
use crate::tui::app::{Action, Model};
use cortado_core::entity::RuntimeId;
use cortado_core::slug::Slug;
use ratatui::crossterm::event::{KeyCode, KeyEvent};

#[derive(Debug, PartialEq)]
pub enum Popup {
    Help,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepoChoice {
    New,
    Clone,
    Worktree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormFocus {
    Name,
    Role,
    Runtime,
    Source,
    Target,
    Branch,
}

#[derive(Debug, PartialEq)]
pub struct NewAgentForm {
    pub team: Slug,
    pub name: String,
    pub role: String,
    pub runtime: RuntimeId,
    pub source: RepoChoice,
    pub target: String,
    pub branch: String,
    pub focus: FormFocus,
}

impl NewAgentForm {
    pub fn new(team: Slug) -> Self {
        NewAgentForm {
            team,
            name: String::new(),
            role: String::new(),
            runtime: RuntimeId::Claude,
            source: RepoChoice::New,
            target: String::new(),
            branch: String::new(),
            focus: FormFocus::Name,
        }
    }
}

const FOCUS_ORDER: [FormFocus; 6] = [
    FormFocus::Name,
    FormFocus::Role,
    FormFocus::Runtime,
    FormFocus::Source,
    FormFocus::Target,
    FormFocus::Branch,
];

fn cycle_focus(f: FormFocus, delta: isize) -> FormFocus {
    let i = FOCUS_ORDER.iter().position(|x| *x == f).unwrap_or(0) as isize;
    let n = FOCUS_ORDER.len() as isize;
    FOCUS_ORDER[((i + delta).rem_euclid(n)) as usize]
}

fn cycle_runtime(r: RuntimeId, delta: isize) -> RuntimeId {
    const ORDER: [RuntimeId; 3] = [RuntimeId::Claude, RuntimeId::Codex, RuntimeId::Opencode];
    let i = ORDER.iter().position(|x| *x == r).unwrap_or(0) as isize;
    ORDER[((i + delta).rem_euclid(3)) as usize]
}

fn cycle_source(s: RepoChoice, delta: isize) -> RepoChoice {
    const ORDER: [RepoChoice; 3] = [RepoChoice::New, RepoChoice::Clone, RepoChoice::Worktree];
    let i = ORDER.iter().position(|x| *x == s).unwrap_or(0) as isize;
    ORDER[((i + delta).rem_euclid(3)) as usize]
}

pub enum FormOutcome {
    Pending,
    Cancel,
    Invalid(String),
    Submit(Action),
}

fn form_key(form: &mut NewAgentForm, key: KeyEvent) -> FormOutcome {
    match key.code {
        KeyCode::Esc => return FormOutcome::Cancel,
        KeyCode::Tab => form.focus = cycle_focus(form.focus, 1),
        KeyCode::BackTab => form.focus = cycle_focus(form.focus, -1),
        KeyCode::Left => match form.focus {
            FormFocus::Runtime => form.runtime = cycle_runtime(form.runtime, -1),
            FormFocus::Source => form.source = cycle_source(form.source, -1),
            _ => {}
        },
        KeyCode::Right => match form.focus {
            FormFocus::Runtime => form.runtime = cycle_runtime(form.runtime, 1),
            FormFocus::Source => form.source = cycle_source(form.source, 1),
            _ => {}
        },
        KeyCode::Backspace => {
            if let Some(f) = text_field(form) {
                f.pop();
            }
        }
        KeyCode::Char(c) => {
            if let Some(f) = text_field(form) {
                f.push(c);
            }
        }
        KeyCode::Enter => return submit(form),
        _ => {}
    }
    FormOutcome::Pending
}

fn text_field(form: &mut NewAgentForm) -> Option<&mut String> {
    match form.focus {
        FormFocus::Name => Some(&mut form.name),
        FormFocus::Role => Some(&mut form.role),
        FormFocus::Target => Some(&mut form.target),
        FormFocus::Branch => Some(&mut form.branch),
        FormFocus::Runtime | FormFocus::Source => None,
    }
}

fn submit(form: &NewAgentForm) -> FormOutcome {
    let name = form.name.trim();
    if name.is_empty() {
        return FormOutcome::Invalid("agent name is required".into());
    }
    let target = form.target.trim();
    let repo = match form.source {
        RepoChoice::New => RepoArg::New,
        RepoChoice::Clone if target.is_empty() => {
            return FormOutcome::Invalid("clone URL is required for source = clone".into())
        }
        RepoChoice::Clone => RepoArg::Clone(target.to_string()),
        RepoChoice::Worktree if target.is_empty() => {
            return FormOutcome::Invalid("repo path is required for source = worktree".into())
        }
        RepoChoice::Worktree => RepoArg::Worktree(target.to_string()),
    };
    let non_empty = |s: &str| {
        let t = s.trim();
        (!t.is_empty()).then(|| t.to_string())
    };
    FormOutcome::Submit(Action::CreateAgent {
        reference: format!("{}/{}", form.team, name),
        runtime: form.runtime,
        role: non_empty(&form.role),
        repo,
        branch: non_empty(&form.branch),
    })
}

pub fn update_popup(m: &mut Model, key: KeyEvent) -> Vec<Action> {
    // take/put-back: mutating the popup AND m.popup in one pass would be a
    // double mutable borrow of m.popup.
    let Some(mut popup) = m.popup.take() else {
        return Vec::new();
    };
    let mut actions = Vec::new();
    let mut keep = true;
    match &mut popup {
        Popup::Help => match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => keep = false,
            _ => {}
        },
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
        Popup::NewAgent(form) => match form_key(form, key) {
            FormOutcome::Pending => {}
            FormOutcome::Cancel => keep = false,
            FormOutcome::Invalid(msg) => m.status = Some(msg),
            FormOutcome::Submit(action) => {
                actions.push(action);
                keep = false;
            }
        },
    }
    if keep {
        m.popup = Some(popup);
    }
    actions
}
