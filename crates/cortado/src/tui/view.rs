//! Pure rendering: (Model, Snapshot, now) -> one frame. No IO, no mutation.
use crate::tui::app::{Model, RailSel, Tab};
use crate::tui::popup::{FormFocus, NewAgentForm, Popup, RepoChoice};
use crate::tui::snapshot::{AgentRow, Snapshot};
use crate::tui::theme;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Tabs, Wrap,
};
use ratatui::Frame;

pub fn render(f: &mut Frame, m: &Model, snap: &Snapshot, now_unix: i64) {
    let [main, status] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(f.area());
    let [rail, right] =
        Layout::horizontal([Constraint::Length(30), Constraint::Min(0)]).areas(main);
    render_rail(f, rail, m, snap);
    render_right(f, right, m, snap, now_unix);
    render_status(f, status, m);
    if let Some(p) = &m.popup {
        render_popup(f, p);
    }
}

pub fn render_error(f: &mut Frame, msg: &str) {
    let text = format!("cannot read cortado state:\n\n{msg}\n\nretrying every 2s — q quits");
    f.render_widget(
        Paragraph::new(text).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::error_block())
                .title(Line::styled("cortado — error", theme::error_block())),
        ),
        f.area(),
    );
}

fn render_rail(f: &mut Frame, area: Rect, m: &Model, snap: &Snapshot) {
    let mut items: Vec<ListItem> = Vec::new();
    for t in &snap.teams {
        let sel = m.sel == Some(RailSel::Team(t.slug.clone()));
        let line = match &t.display {
            Ok(name) => Line::styled(name.clone(), theme::team()),
            Err(_) => Line::styled(format!("{} INVALID", t.slug), theme::invalid()),
        };
        items.push(selected(ListItem::new(line), sel));
        for a in &t.agents {
            let sel = m.sel == Some(RailSel::Agent(t.slug.clone(), a.slug.clone()));
            let line = match &a.display {
                Ok(name) => {
                    let mut spans = vec![Span::raw(format!("  {name}"))];
                    if !a.sessions.is_empty() {
                        spans.push(Span::styled(
                            format!(" ●{}", a.sessions.len()),
                            theme::badge(),
                        ));
                    }
                    Line::from(spans)
                }
                Err(_) => Line::styled(format!("  {} INVALID", a.slug), theme::invalid()),
            };
            items.push(selected(ListItem::new(line), sel));
        }
    }
    for s in &snap.strays {
        items.push(ListItem::new(Line::styled(
            format!("{}: INVALID NAME {:?}", s.context, s.name),
            theme::invalid(),
        )));
    }
    f.render_widget(
        List::new(items).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border())
                .title(Line::styled("agents", theme::title())),
        ),
        area,
    );
}

fn selected(item: ListItem<'_>, on: bool) -> ListItem<'_> {
    if on {
        item.style(theme::selection())
    } else {
        item
    }
}

fn render_right(f: &mut Frame, area: Rect, m: &Model, snap: &Snapshot, now_unix: i64) {
    let [bar, content] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
    let idx = match m.tab {
        Tab::Sessions => 0,
        Tab::Memory => 1,
    };
    f.render_widget(
        Tabs::new(vec!["Sessions", "Memory"])
            .select(idx)
            .style(theme::tab_inactive())
            .highlight_style(theme::tab_active()),
        bar,
    );
    let agent = m.selected_agent(snap);
    match m.tab {
        Tab::Sessions => render_sessions(f, content, agent, now_unix),
        Tab::Memory => render_memory(f, content, m, agent),
    }
}

fn render_sessions(f: &mut Frame, area: Rect, agent: Option<&AgentRow>, now_unix: i64) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(Line::styled("sessions", theme::title()));
    let Some(agent) = agent else {
        f.render_widget(Paragraph::new("select an agent").block(block), area);
        return;
    };
    if agent.sessions.is_empty() {
        f.render_widget(
            Paragraph::new("no live sessions — n spawns one").block(block),
            area,
        );
        return;
    }
    let rows: Vec<Row> = agent
        .sessions
        .iter()
        .map(|s| {
            Row::new(vec![
                Cell::from(s.name.clone()),
                Cell::from(Span::styled(s.model.clone(), theme::model_col())),
                Cell::from(Span::styled(
                    fmt_uptime(now_unix - s.created_unix),
                    theme::uptime_col(),
                )),
            ])
        })
        .collect();
    f.render_widget(
        Table::new(
            rows,
            [
                Constraint::Min(28),
                Constraint::Length(14),
                Constraint::Length(8),
            ],
        )
        .header(Row::new(vec!["session", "model", "uptime"]).style(theme::header()))
        .block(block),
        area,
    );
}

fn render_memory(f: &mut Frame, area: Rect, m: &Model, agent: Option<&AgentRow>) {
    if let Some(p) = &m.preview {
        // Wrap-accurate max scroll: total wrapped rows minus the visible
        // inner height. line_count wraps at exactly the width passed (it does
        // NOT subtract block borders) and would add border rows, so measure a
        // blockless paragraph at the inner width and do the border math here.
        let inner_w = area.width.saturating_sub(2).max(1);
        let inner_h = area.height.saturating_sub(2);
        let total_rows = u16::try_from(
            Paragraph::new(p.content.clone())
                .wrap(Wrap { trim: false })
                .line_count(inner_w),
        )
        .unwrap_or(u16::MAX);
        p.max_scroll.set(total_rows.saturating_sub(inner_h));

        f.render_widget(
            Paragraph::new(p.content.clone())
                .scroll((p.scroll, 0))
                .wrap(Wrap { trim: false })
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(theme::border())
                        .title(Line::styled(
                            format!("{} — j/k scroll, Esc back", p.title),
                            theme::title(),
                        )),
                ),
            area,
        );
        return;
    }
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(Line::styled("memory", theme::title()));
    let Some(agent) = agent else {
        f.render_widget(Paragraph::new("select an agent").block(block), area);
        return;
    };
    let items: Vec<ListItem> = agent
        .memory
        .iter()
        .enumerate()
        .map(|(i, file)| selected(ListItem::new(file.label.clone()), i == m.mem_idx))
        .collect();
    f.render_widget(List::new(items).block(block), area);
}

fn render_status(f: &mut Frame, area: Rect, m: &Model) {
    let (text, style) = match &m.status {
        Some(s) if s.starts_with("error") => (s.clone(), theme::status_error()),
        Some(s) => (s.clone(), theme::status_msg()),
        None => (
            "n new session · Enter open · e edit · x kill · N new agent · Tab/m tabs · q quit"
                .to_string(),
            theme::hint(),
        ),
    };
    f.render_widget(Paragraph::new(text).style(style), area);
}

fn render_popup(f: &mut Frame, popup: &Popup) {
    let area = centered(f.area(), 60, 12);
    f.render_widget(Clear, area);
    match popup {
        Popup::ModelPicker(p) => {
            let items: Vec<ListItem> = p
                .models
                .iter()
                .enumerate()
                .map(|(i, (key, label))| {
                    selected(ListItem::new(format!("{label} ({key})")), i == p.selected)
                })
                .collect();
            f.render_widget(
                List::new(items).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(theme::border())
                        .title(Line::styled(
                            "new session — pick model (Enter/Esc)",
                            theme::title(),
                        )),
                ),
                area,
            );
        }
        Popup::ConfirmKill { reference, count } => {
            f.render_widget(
                Paragraph::new(format!("kill {count} session(s) of {reference}?  y / n")).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(theme::border())
                        .title(Line::styled("confirm kill", theme::title())),
                ),
                area,
            );
        }
        Popup::NewAgent(form) => render_form(f, area, form),
    }
}

fn render_form(f: &mut Frame, area: Rect, form: &NewAgentForm) {
    let field = |label: &str, value: &str, focused: bool| {
        let marker = if focused { "▶ " } else { "  " };
        Line::from(format!("{marker}{label:<9}{value}"))
    };
    let source = match form.source {
        RepoChoice::New => "new",
        RepoChoice::Clone => "clone",
        RepoChoice::Worktree => "worktree",
    };
    let target_label = match form.source {
        RepoChoice::New => "(unused)",
        RepoChoice::Clone => "url",
        RepoChoice::Worktree => "path",
    };
    let lines = vec![
        Line::from(format!("  team     {}", form.team)),
        field("name", &form.name, form.focus == FormFocus::Name),
        field("role", &form.role, form.focus == FormFocus::Role),
        field(
            "runtime",
            form.runtime.as_str(),
            form.focus == FormFocus::Runtime,
        ),
        field("source", source, form.focus == FormFocus::Source),
        field(target_label, &form.target, form.focus == FormFocus::Target),
        field("branch", &form.branch, form.focus == FormFocus::Branch),
        Line::from("  Tab next · ←/→ cycle · Enter create · Esc cancel"),
    ];
    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border())
                .title(Line::styled("new agent", theme::title())),
        ),
        area,
    );
}

fn centered(outer: Rect, w: u16, h: u16) -> Rect {
    let w = w.min(outer.width);
    let h = h.min(outer.height);
    Rect {
        x: outer.x + (outer.width - w) / 2,
        y: outer.y + (outer.height - h) / 2,
        width: w,
        height: h,
    }
}

pub fn fmt_uptime(secs: i64) -> String {
    let s = secs.max(0);
    let (h, m) = (s / 3600, (s % 3600) / 60);
    if h > 0 {
        format!("{h}h{m:02}m")
    } else {
        format!("{m}m{:02}s", s % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::app::{Model, RailSel, Tab};
    use crate::tui::snapshot::{AgentRow, MemFile, SessionRow, Snapshot, TeamRow};
    use cortado_core::slug::Slug;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn s(x: &str) -> Slug {
        Slug::parse(x).unwrap()
    }

    fn snap() -> Snapshot {
        Snapshot {
            teams: vec![TeamRow {
                slug: s("newsletter"),
                display: Ok("Newsletter".into()),
                agents: vec![AgentRow {
                    slug: s("scout"),
                    display: Ok("Scout".into()),
                    sessions: vec![SessionRow {
                        name: "cortado_newsletter_scout_1".into(),
                        n: 1,
                        created_unix: 1000,
                        model: "kimi".into(),
                    }],
                    memory: vec![MemFile {
                        label: "AGENT.md".into(),
                        path: "/mem/AGENT.md".into(),
                    }],
                }],
            }],
            strays: vec![],
            models: vec![("claude".into(), "Claude".into())],
        }
    }

    fn rendered(m: &Model, snap: &Snapshot) -> String {
        let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
        terminal.draw(|f| render(f, m, snap, 1000 + 3723)).unwrap();
        buffer_text(terminal.backend())
    }

    fn buffer_text(backend: &TestBackend) -> String {
        let buf = backend.buffer();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                out.push_str(buf[(x, y)].symbol());
            }
            out.push('\n');
        }
        out
    }

    #[test]
    fn rail_shows_team_agent_and_badge() {
        let m = Model {
            sel: Some(RailSel::Agent(s("newsletter"), s("scout"))),
            ..Default::default()
        };
        let text = rendered(&m, &snap());
        assert!(text.contains("Newsletter"));
        assert!(text.contains("Scout"));
        assert!(text.contains("●1"));
    }

    #[test]
    fn sessions_tab_shows_name_model_uptime() {
        let m = Model {
            sel: Some(RailSel::Agent(s("newsletter"), s("scout"))),
            ..Default::default()
        };
        let text = rendered(&m, &snap());
        assert!(text.contains("cortado_newsletter_scout_1"));
        assert!(text.contains("kimi"));
        assert!(text.contains("1h02m"));
    }

    #[test]
    fn memory_tab_lists_files_and_preview_renders_content() {
        let mut m = Model {
            sel: Some(RailSel::Agent(s("newsletter"), s("scout"))),
            tab: Tab::Memory,
            ..Default::default()
        };
        let text = rendered(&m, &snap());
        assert!(text.contains("AGENT.md"));
        m.preview = Some(crate::tui::app::Preview {
            title: "AGENT.md".into(),
            content: "hello memory".into(),
            scroll: 0,
            path: "/mem/AGENT.md".into(),
            max_scroll: std::cell::Cell::new(0),
        });
        let text = rendered(&m, &snap());
        assert!(text.contains("hello memory"));
    }

    #[test]
    fn status_line_shows_message_or_hints() {
        let mut m = Model::default();
        let text = rendered(&m, &snap());
        assert!(text.contains("q quit"));
        m.status = Some("session cortado_newsletter_scout_1".into());
        let text = rendered(&m, &snap());
        assert!(text.contains("session cortado_newsletter_scout_1"));
    }

    #[test]
    fn popups_render_over_the_frame() {
        let m = Model {
            popup: Some(crate::tui::popup::Popup::ConfirmKill {
                reference: "newsletter/scout".into(),
                count: 2,
            }),
            ..Default::default()
        };
        let text = rendered(&m, &snap());
        assert!(text.contains("kill 2 session(s) of newsletter/scout?"));
    }

    #[test]
    fn invalid_agents_render_invalid() {
        let mut sn = snap();
        sn.teams[0].agents[0].display = Err("bad toml".into());
        let text = rendered(&Model::default(), &sn);
        assert!(text.contains("INVALID"));
    }

    #[test]
    fn error_state_renders_message() {
        let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
        terminal.draw(|f| render_error(f, "boom")).unwrap();
        let text = buffer_text(terminal.backend());
        assert!(text.contains("boom"));
        assert!(text.contains("retrying"));
    }

    #[test]
    fn uptime_formats() {
        assert_eq!(fmt_uptime(3723), "1h02m");
        assert_eq!(fmt_uptime(59), "0m59s");
        assert_eq!(fmt_uptime(-5), "0m00s");
    }

    fn rendered_terminal(m: &Model, snap: &Snapshot) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
        terminal.draw(|f| render(f, m, snap, 1000 + 3723)).unwrap();
        terminal
    }

    /// Every buffer cell rendered with the theme's selection background,
    /// concatenated. Post-theme replacement for the old REVERSED-modifier
    /// check: selection is now marked by `theme::SELECTION_BG`, not a
    /// modifier.
    fn selected_bg_text(backend: &TestBackend) -> String {
        let buf = backend.buffer();
        let mut out = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                let cell = &buf[(x, y)];
                if cell.bg == crate::tui::theme::SELECTION_BG {
                    out.push_str(cell.symbol());
                }
            }
        }
        out
    }

    #[test]
    fn rail_selection_uses_theme_background() {
        let m = Model {
            sel: Some(RailSel::Agent(s("newsletter"), s("scout"))),
            ..Default::default()
        };
        let terminal = rendered_terminal(&m, &snap());
        let rev = selected_bg_text(terminal.backend());
        assert!(rev.contains("Scout"), "selected-bg cells: {rev:?}");
        assert!(!rev.contains("Newsletter"), "selected-bg cells: {rev:?}");
    }

    #[test]
    fn model_picker_popup_renders_and_marks_selection() {
        let m = Model {
            popup: Some(crate::tui::popup::Popup::ModelPicker(
                crate::tui::popup::ModelPicker {
                    reference: "newsletter/scout".into(),
                    models: vec![
                        ("claude".into(), "Claude".into()),
                        ("kimi".into(), "Kimi K2".into()),
                    ],
                    selected: 1,
                },
            )),
            ..Default::default()
        };
        let terminal = rendered_terminal(&m, &snap());
        let text = buffer_text(terminal.backend());
        assert!(text.contains("pick model"));
        assert!(text.contains("Claude (claude)"));
        assert!(text.contains("Kimi K2 (kimi)"));
        let rev = selected_bg_text(terminal.backend());
        assert!(rev.contains("Kimi K2 (kimi)"), "selected-bg cells: {rev:?}");
        assert!(
            !rev.contains("Claude (claude)"),
            "selected-bg cells: {rev:?}"
        );
    }

    #[test]
    fn ratatui_line_count_wraps_as_expected() {
        // Pins the line_count semantics the preview scroll clamp relies on;
        // a ratatui upgrade that changes wrapping fails here, not silently.
        let text = "Hello World";
        let count = |w| {
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .line_count(w)
        };
        assert_eq!(count(20), 1);
        assert_eq!(count(10), 2);
    }

    fn any_cell_matches(
        backend: &TestBackend,
        pred: impl Fn(&ratatui::buffer::Cell) -> bool,
    ) -> bool {
        let buf = backend.buffer();
        (0..buf.area.height).any(|y| (0..buf.area.width).any(|x| pred(&buf[(x, y)])))
    }

    #[test]
    fn selection_bar_uses_theme_background() {
        let m = Model {
            sel: Some(RailSel::Agent(s("newsletter"), s("scout"))),
            ..Default::default()
        };
        let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
        terminal
            .draw(|f| render(f, &m, &snap(), 1000 + 3723))
            .unwrap();
        assert!(any_cell_matches(terminal.backend(), |c| {
            c.bg == crate::tui::theme::SELECTION_BG
        }));
    }

    #[test]
    fn selected_team_keeps_blue_fg() {
        // A selected TEAM row keeps its blue team fg — semantic color wins
        // over the magenta selection fg, the same way a selected INVALID row
        // stays red. The selection bg still marks the row.
        let m = Model {
            sel: Some(RailSel::Team(s("newsletter"))),
            ..Default::default()
        };
        let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
        terminal
            .draw(|f| render(f, &m, &snap(), 1000 + 3723))
            .unwrap();
        assert!(any_cell_matches(terminal.backend(), |c| {
            c.fg == crate::tui::theme::BORDER_FG && c.bg == crate::tui::theme::SELECTION_BG
        }));
    }

    #[test]
    fn borders_use_theme_color() {
        let m = Model {
            sel: Some(RailSel::Agent(s("newsletter"), s("scout"))),
            ..Default::default()
        };
        let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
        terminal
            .draw(|f| render(f, &m, &snap(), 1000 + 3723))
            .unwrap();
        assert!(any_cell_matches(terminal.backend(), |c| {
            c.fg == crate::tui::theme::BORDER_FG
        }));
    }

    #[test]
    fn new_agent_form_popup_renders_fields_and_focus() {
        let mut form = crate::tui::popup::NewAgentForm::new(s("newsletter"));
        form.name = "Editor".into();
        let m = Model {
            popup: Some(crate::tui::popup::Popup::NewAgent(form)),
            ..Default::default()
        };
        let text = rendered(&m, &snap());
        assert!(text.contains("new agent"));
        assert!(text.contains("team     newsletter"));
        assert!(text.contains("▶ name     Editor")); // Name focused on a fresh form
        assert!(text.contains("runtime  claude"));
        assert!(text.contains("source   new"));
        assert!(text.contains("Tab next"));
    }
}
