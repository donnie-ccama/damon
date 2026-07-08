//! UI state and the pure update function. No filesystem, tmux, or git here:
//! side effects travel out as Actions for the event loop to execute.
use crate::commands::agent::RepoArg;
use crate::tui::popup::Popup;
use crate::tui::snapshot::{AgentRow, Snapshot};
use damon_core::entity::RuntimeId;
use damon_core::slug::Slug;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tab {
    #[default]
    Sessions,
    Memory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RailSel {
    Team(Slug),
    Agent(Slug, Slug),
}

pub enum Event {
    Key(KeyEvent),
    Tick,
}

#[derive(Debug, PartialEq)]
pub enum Action {
    Open {
        reference: String,
        model: Option<String>,
        fresh: bool,
    },
    Kill {
        reference: String,
    },
    CreateAgent {
        reference: String,
        runtime: RuntimeId,
        role: Option<String>,
        repo: RepoArg,
        branch: Option<String>,
    },
    Preview {
        title: String,
        path: PathBuf,
    },
    Quit,
}

#[derive(Debug)]
pub struct Preview {
    pub title: String,
    pub content: String,
    pub scroll: u16,
}

#[derive(Default)]
pub struct Model {
    pub sel: Option<RailSel>,
    pub tab: Tab,
    pub popup: Option<Popup>,
    pub status: Option<String>,
    pub mem_idx: usize,
    pub preview: Option<Preview>,
}

impl Model {
    pub fn selected_agent<'s>(&self, snap: &'s Snapshot) -> Option<&'s AgentRow> {
        match self.sel.as_ref()? {
            RailSel::Agent(t, a) => snap.agent(t, a),
            RailSel::Team(_) => None,
        }
    }
}

pub fn rail_rows(snap: &Snapshot) -> Vec<RailSel> {
    let mut rows = Vec::new();
    for t in &snap.teams {
        rows.push(RailSel::Team(t.slug.clone()));
        for a in &t.agents {
            rows.push(RailSel::Agent(t.slug.clone(), a.slug.clone()));
        }
    }
    rows
}

fn ensure_selection(m: &mut Model, snap: &Snapshot) {
    let rows = rail_rows(snap);
    if !m.sel.as_ref().is_some_and(|s| rows.contains(s)) {
        m.sel = rows.first().cloned();
    }
}

fn move_sel(m: &mut Model, snap: &Snapshot, delta: isize) {
    let rows = rail_rows(snap);
    let Some(cur) = m
        .sel
        .as_ref()
        .and_then(|s| rows.iter().position(|r| r == s))
    else {
        m.sel = rows.first().cloned();
        return;
    };
    let next = cur
        .saturating_add_signed(delta)
        .min(rows.len().saturating_sub(1));
    m.sel = rows.get(next).cloned();
    m.mem_idx = 0;
}

fn reference(m: &Model) -> Option<String> {
    match m.sel.as_ref()? {
        RailSel::Agent(t, a) => Some(format!("{t}/{a}")),
        RailSel::Team(_) => None,
    }
}

pub fn update(m: &mut Model, snap: &Snapshot, ev: Event) -> Vec<Action> {
    ensure_selection(m, snap);
    let Event::Key(key) = ev else {
        return Vec::new();
    };
    if key.kind != KeyEventKind::Press {
        return Vec::new();
    }
    if m.popup.is_some() {
        return crate::tui::popup::update_popup(m, key);
    }
    if m.preview.is_some() {
        return update_preview(m, key);
    }
    match key.code {
        KeyCode::Char('q') => return vec![Action::Quit],
        KeyCode::Up | KeyCode::Char('k') if m.tab == Tab::Sessions => move_sel(m, snap, -1),
        KeyCode::Down | KeyCode::Char('j') if m.tab == Tab::Sessions => move_sel(m, snap, 1),
        KeyCode::Up => move_sel(m, snap, -1),
        KeyCode::Down => move_sel(m, snap, 1),
        KeyCode::Char('k') => m.mem_idx = m.mem_idx.saturating_sub(1),
        KeyCode::Char('j') => {
            let max = m
                .selected_agent(snap)
                .map_or(0, |a| a.memory.len().saturating_sub(1));
            m.mem_idx = (m.mem_idx + 1).min(max);
        }
        KeyCode::Tab => {
            m.tab = match m.tab {
                Tab::Sessions => Tab::Memory,
                Tab::Memory => Tab::Sessions,
            }
        }
        KeyCode::Char('m') => m.tab = Tab::Memory,
        KeyCode::Enter => return on_enter(m, snap),
        KeyCode::Char('n') => {
            if let (Some(reference), false) = (reference(m), snap.models.is_empty()) {
                m.popup = Some(Popup::ModelPicker(crate::tui::popup::ModelPicker {
                    reference,
                    models: snap.models.clone(),
                    selected: 0,
                }));
            }
        }
        KeyCode::Char('x') => {
            if let (Some(reference), Some(agent)) = (reference(m), m.selected_agent(snap)) {
                if agent.sessions.is_empty() {
                    m.status = Some(format!("no live sessions for {reference}"));
                } else {
                    m.popup = Some(Popup::ConfirmKill {
                        reference,
                        count: agent.sessions.len(),
                    });
                }
            }
        }
        KeyCode::Char('N') => {
            let team = match m.sel.as_ref() {
                Some(RailSel::Team(t)) | Some(RailSel::Agent(t, _)) => Some(t.clone()),
                None => None,
            };
            if let Some(team) = team {
                m.popup = Some(Popup::NewAgent(crate::tui::popup::NewAgentForm::new(team)));
            }
        }
        _ => {}
    }
    Vec::new()
}

fn on_enter(m: &mut Model, snap: &Snapshot) -> Vec<Action> {
    match m.tab {
        Tab::Sessions => match reference(m) {
            Some(reference) => vec![Action::Open {
                reference,
                model: None,
                fresh: false,
            }],
            None => Vec::new(),
        },
        Tab::Memory => {
            let Some(agent) = m.selected_agent(snap) else {
                return Vec::new();
            };
            let Some(file) = agent.memory.get(m.mem_idx) else {
                return Vec::new();
            };
            vec![Action::Preview {
                title: file.label.clone(),
                path: file.path.clone(),
            }]
        }
    }
}

fn update_preview(m: &mut Model, key: KeyEvent) -> Vec<Action> {
    let Some(p) = m.preview.as_mut() else {
        return Vec::new();
    };
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => m.preview = None,
        KeyCode::Up | KeyCode::Char('k') => p.scroll = p.scroll.saturating_sub(1),
        KeyCode::Down | KeyCode::Char('j') => p.scroll = p.scroll.saturating_add(1),
        KeyCode::PageUp => p.scroll = p.scroll.saturating_sub(10),
        KeyCode::PageDown => p.scroll = p.scroll.saturating_add(10),
        _ => {}
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::snapshot::{AgentRow, MemFile, Snapshot, TeamRow};
    use damon_core::slug::Slug;
    use ratatui::crossterm::event::{KeyCode, KeyEvent};

    fn s(x: &str) -> Slug {
        Slug::parse(x).unwrap()
    }

    fn key(c: KeyCode) -> Event {
        Event::Key(KeyEvent::from(c))
    }

    pub(crate) fn snap_fixture() -> Snapshot {
        Snapshot {
            teams: vec![TeamRow {
                slug: s("newsletter"),
                display: Ok("Newsletter".into()),
                agents: vec![
                    AgentRow {
                        slug: s("scout"),
                        display: Ok("Scout".into()),
                        sessions: vec![],
                        memory: vec![MemFile {
                            label: "AGENT.md".into(),
                            path: "/mem/AGENT.md".into(),
                        }],
                    },
                    AgentRow {
                        slug: s("writer"),
                        display: Ok("Writer".into()),
                        sessions: vec![],
                        memory: vec![],
                    },
                ],
            }],
            strays: vec![],
            models: vec![("claude".into(), "Claude".into())],
        }
    }

    #[test]
    fn selection_starts_on_first_row_and_moves() {
        let snap = snap_fixture();
        let mut m = Model::default();
        update(&mut m, &snap, Event::Tick);
        assert_eq!(m.sel, Some(RailSel::Team(s("newsletter"))));
        update(&mut m, &snap, key(KeyCode::Char('j')));
        assert_eq!(m.sel, Some(RailSel::Agent(s("newsletter"), s("scout"))));
        update(&mut m, &snap, key(KeyCode::Down));
        update(&mut m, &snap, key(KeyCode::Down)); // clamped at last row
        assert_eq!(m.sel, Some(RailSel::Agent(s("newsletter"), s("writer"))));
        update(&mut m, &snap, key(KeyCode::Char('k')));
        assert_eq!(m.sel, Some(RailSel::Agent(s("newsletter"), s("scout"))));
    }

    #[test]
    fn vanished_selection_snaps_back_to_first_row() {
        let snap = snap_fixture();
        let mut m = Model {
            sel: Some(RailSel::Agent(s("gone"), s("gone"))),
            ..Default::default()
        };
        update(&mut m, &snap, Event::Tick);
        assert_eq!(m.sel, Some(RailSel::Team(s("newsletter"))));
    }

    #[test]
    fn tab_and_m_switch_tabs_and_q_quits() {
        let snap = snap_fixture();
        let mut m = Model::default();
        update(&mut m, &snap, key(KeyCode::Tab));
        assert_eq!(m.tab, Tab::Memory);
        update(&mut m, &snap, key(KeyCode::Tab));
        assert_eq!(m.tab, Tab::Sessions);
        update(&mut m, &snap, key(KeyCode::Char('m')));
        assert_eq!(m.tab, Tab::Memory);
        assert_eq!(
            update(&mut m, &snap, key(KeyCode::Char('q'))),
            vec![Action::Quit]
        );
    }

    #[test]
    fn enter_on_agent_opens_reattach() {
        let snap = snap_fixture();
        let mut m = Model::default();
        update(&mut m, &snap, key(KeyCode::Char('j'))); // scout
        let actions = update(&mut m, &snap, key(KeyCode::Enter));
        assert_eq!(
            actions,
            vec![Action::Open {
                reference: "newsletter/scout".into(),
                model: None,
                fresh: false
            }]
        );
    }

    #[test]
    fn enter_on_team_does_nothing() {
        let snap = snap_fixture();
        let mut m = Model::default();
        update(&mut m, &snap, Event::Tick); // selects team row
        assert_eq!(update(&mut m, &snap, key(KeyCode::Enter)), vec![]);
    }

    #[test]
    fn memory_tab_enter_requests_preview() {
        let snap = snap_fixture();
        let mut m = Model::default();
        update(&mut m, &snap, key(KeyCode::Char('j'))); // scout
        update(&mut m, &snap, key(KeyCode::Char('m')));
        let actions = update(&mut m, &snap, key(KeyCode::Enter));
        assert_eq!(
            actions,
            vec![Action::Preview {
                title: "AGENT.md".into(),
                path: "/mem/AGENT.md".into()
            }]
        );
    }

    #[test]
    fn preview_scrolls_and_escapes() {
        let snap = snap_fixture();
        let mut m = Model {
            preview: Some(Preview {
                title: "t".into(),
                content: "c".into(),
                scroll: 0,
            }),
            ..Default::default()
        };
        update(&mut m, &snap, key(KeyCode::Char('j')));
        assert_eq!(m.preview.as_ref().unwrap().scroll, 1);
        update(&mut m, &snap, key(KeyCode::Esc));
        assert!(m.preview.is_none());
    }

    #[test]
    fn n_opens_model_picker_and_enter_spawns_fresh() {
        let snap = snap_fixture();
        let mut m = Model::default();
        update(&mut m, &snap, key(KeyCode::Char('j'))); // scout
        update(&mut m, &snap, key(KeyCode::Char('n')));
        assert!(matches!(
            m.popup,
            Some(crate::tui::popup::Popup::ModelPicker(_))
        ));
        let actions = update(&mut m, &snap, key(KeyCode::Enter));
        assert_eq!(
            actions,
            vec![Action::Open {
                reference: "newsletter/scout".into(),
                model: Some("claude".into()),
                fresh: true
            }]
        );
        assert!(m.popup.is_none());
    }

    #[test]
    fn n_on_team_row_does_nothing() {
        let snap = snap_fixture();
        let mut m = Model::default();
        update(&mut m, &snap, Event::Tick); // team row
        update(&mut m, &snap, key(KeyCode::Char('n')));
        assert!(m.popup.is_none());
    }

    #[test]
    fn x_confirms_before_killing_and_esc_cancels() {
        let mut snap = snap_fixture();
        snap.teams[0].agents[0]
            .sessions
            .push(crate::tui::snapshot::SessionRow {
                name: "damon_newsletter_scout_1".into(),
                n: 1,
                created_unix: 0,
                model: "claude".into(),
            });
        let mut m = Model::default();
        update(&mut m, &snap, key(KeyCode::Char('j'))); // scout
        update(&mut m, &snap, key(KeyCode::Char('x')));
        assert!(matches!(
            m.popup,
            Some(crate::tui::popup::Popup::ConfirmKill { .. })
        ));
        assert_eq!(update(&mut m, &snap, key(KeyCode::Esc)), vec![]);
        assert!(m.popup.is_none());
        update(&mut m, &snap, key(KeyCode::Char('x')));
        let actions = update(&mut m, &snap, key(KeyCode::Char('y')));
        assert_eq!(
            actions,
            vec![Action::Kill {
                reference: "newsletter/scout".into()
            }]
        );
        assert!(m.popup.is_none());
    }

    #[test]
    fn x_with_no_sessions_sets_status_not_popup() {
        let snap = snap_fixture();
        let mut m = Model::default();
        update(&mut m, &snap, key(KeyCode::Char('j')));
        update(&mut m, &snap, key(KeyCode::Char('x')));
        assert!(m.popup.is_none());
        assert!(m.status.as_deref().unwrap().contains("no live sessions"));
    }

    #[test]
    fn new_agent_form_fills_and_submits() {
        let snap = snap_fixture();
        let mut m = Model::default();
        update(&mut m, &snap, Event::Tick); // team row selected
        update(&mut m, &snap, key(KeyCode::Char('N')));
        assert!(matches!(
            m.popup,
            Some(crate::tui::popup::Popup::NewAgent(_))
        ));
        for c in "Editor".chars() {
            update(&mut m, &snap, key(KeyCode::Char(c)));
        }
        update(&mut m, &snap, key(KeyCode::Tab)); // -> Role (left empty)
        update(&mut m, &snap, key(KeyCode::Tab)); // -> Runtime
        update(&mut m, &snap, key(KeyCode::Right)); // claude -> codex
        update(&mut m, &snap, key(KeyCode::Tab)); // -> Source (stays New)
        let actions = update(&mut m, &snap, key(KeyCode::Enter));
        assert_eq!(
            actions,
            vec![Action::CreateAgent {
                reference: "newsletter/Editor".into(),
                runtime: damon_core::entity::RuntimeId::Codex,
                role: None,
                repo: crate::commands::agent::RepoArg::New,
                branch: None,
            }]
        );
        assert!(m.popup.is_none());
    }

    #[test]
    fn new_agent_form_requires_name_and_clone_url() {
        let snap = snap_fixture();
        let mut m = Model::default();
        update(&mut m, &snap, Event::Tick);
        update(&mut m, &snap, key(KeyCode::Char('N')));
        assert_eq!(update(&mut m, &snap, key(KeyCode::Enter)), vec![]); // no name
        assert!(m.popup.is_some());
        assert!(m.status.as_deref().unwrap().contains("name"));
        for c in "Ed".chars() {
            update(&mut m, &snap, key(KeyCode::Char(c)));
        }
        update(&mut m, &snap, key(KeyCode::Tab)); // Role
        update(&mut m, &snap, key(KeyCode::Tab)); // Runtime
        update(&mut m, &snap, key(KeyCode::Tab)); // Source
        update(&mut m, &snap, key(KeyCode::Right)); // New -> Clone; target empty
        assert_eq!(update(&mut m, &snap, key(KeyCode::Enter)), vec![]);
        assert!(m.status.as_deref().unwrap().contains("clone URL"));
    }

    #[test]
    fn n_uppercase_works_from_agent_row_too() {
        let snap = snap_fixture();
        let mut m = Model::default();
        update(&mut m, &snap, key(KeyCode::Char('j'))); // scout (agent row)
        update(&mut m, &snap, key(KeyCode::Char('N')));
        match &m.popup {
            Some(crate::tui::popup::Popup::NewAgent(f)) => {
                assert_eq!(f.team.as_str(), "newsletter")
            }
            other => panic!("expected NewAgent popup, got {other:?}"),
        }
    }
}
