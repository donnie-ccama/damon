//! Pure rendering: (Model, Snapshot) -> one frame. No IO, no mutation.
use crate::tui::app::{Model, RailSel, Tab};
use crate::tui::popup::{FormFocus, NewAgentForm, Popup, RepoChoice};
use crate::tui::snapshot::{AgentRow, Snapshot};
use crate::tui::theme;
use cortado_herdr::AgentStatus;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, List, ListItem, Paragraph, Row, Table, Tabs, Wrap,
};
use ratatui::Frame;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;
use ratatui_image::{Resize, StatefulImage};

const LOGO_PNG: &[u8] = include_bytes!("../../assets/cortado-logo.png");
const LOGO_SCALE_PERCENT: u16 = 50;
const LOGO_BACKGROUND: image::Rgba<u8> = image::Rgba([0x29, 0x2c, 0x33, 0xff]);
const TMUX_CLIENT_FORMAT: &str = concat!(
    "#{client_pid}|#{client_created}|#{client_tty}|",
    "#{client_width}|#{client_height}|",
    "#{client_cell_width}|#{client_cell_height}|#{client_termname}"
);

pub struct LogoImage {
    protocol: StatefulProtocol,
}

/// Owns the terminal-specific image state. The tmux workspace starts detached,
/// so the logo is initialized only after a real client supplies its cell
/// metrics. A changed client signature rebuilds it at the new size.
pub struct LogoState {
    image: Option<LogoImage>,
    tmux_client_signature: Option<String>,
    direct_terminal_probed: bool,
}

impl LogoState {
    pub fn new() -> Self {
        let mut state = Self {
            image: None,
            tmux_client_signature: None,
            direct_terminal_probed: false,
        };
        state.refresh();
        state
    }

    /// Retry after detached startup and rebuild after a client reconnects.
    /// Call only between event reads; terminal capability queries consume
    /// stdin briefly.
    pub fn refresh(&mut self) {
        if std::env::var_os("TMUX_PANE").is_some() {
            let Some(client) = current_tmux_client() else {
                // The workspace starts detached, before Ghostty opens. Do not
                // query stdio until a real client is present to answer.
                self.image = None;
                self.tmux_client_signature = None;
                return;
            };
            if self.tmux_client_signature.as_deref() == Some(client.signature.as_str()) {
                return;
            }
            self.image = load_logo(Some(&client));
            self.tmux_client_signature = Some(client.signature);
        } else if !self.direct_terminal_probed {
            self.image = load_logo(None);
            self.direct_terminal_probed = true;
        }
    }

    pub fn image_mut(&mut self) -> Option<&mut LogoImage> {
        self.image.as_mut()
    }

    pub fn invalidate(&mut self) {
        self.image = None;
        self.tmux_client_signature = None;
        self.direct_terminal_probed = false;
    }
}

struct TmuxClient {
    signature: String,
    font_size: Option<(u16, u16)>,
    is_ghostty: bool,
}

fn current_tmux_client() -> Option<TmuxClient> {
    let pane = std::env::var_os("TMUX_PANE")?;
    let output = std::process::Command::new("tmux")
        .args([
            "list-clients",
            "-t",
            pane.to_str()?,
            "-F",
            TMUX_CLIENT_FORMAT,
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_tmux_clients(&String::from_utf8(output.stdout).ok()?)
}

fn parse_tmux_clients(text: &str) -> Option<TmuxClient> {
    let mut lines: Vec<&str> = text.lines().filter(|line| !line.is_empty()).collect();
    lines.sort_unstable();
    let signature = lines.join("\n");

    // Prefer the Ghostty client Cortado opened. If multiple clients view the
    // session, their complete sorted list remains in the signature so any
    // attach, detach, resize, or font-metric change triggers retransmission.
    let parsed: Vec<(Option<(u16, u16)>, bool)> = lines
        .iter()
        .filter_map(|line| {
            let fields: Vec<&str> = line.split('|').collect();
            let term = fields.get(7)?.to_ascii_lowercase();
            let width = fields.get(5)?.parse::<u16>().ok();
            let height = fields.get(6)?.parse::<u16>().ok();
            let font_size = match (width, height) {
                (Some(width), Some(height)) if width > 0 && height > 0 => Some((width, height)),
                _ => None,
            };
            Some((font_size, term.contains("ghostty")))
        })
        .collect();
    let (font_size, is_ghostty) = parsed
        .iter()
        .find(|(_, is_ghostty)| *is_ghostty)
        .or_else(|| parsed.first())?;

    Some(TmuxClient {
        signature,
        font_size: *font_size,
        is_ghostty: *is_ghostty,
    })
}

fn terminal_font_size() -> Option<(u16, u16)> {
    let size = ratatui::crossterm::terminal::window_size().ok()?;
    if size.columns == 0 || size.rows == 0 || size.width == 0 || size.height == 0 {
        return None;
    }
    Some((
        size.width.div_ceil(size.columns),
        size.height.div_ceil(size.rows),
    ))
}

fn env_is_ghostty() -> bool {
    std::env::var_os("GHOSTTY_RESOURCES_DIR").is_some()
        || std::env::var("TERM").is_ok_and(|term| term.to_ascii_lowercase().contains("ghostty"))
        || std::env::var("TERM_PROGRAM")
            .is_ok_and(|term| term.to_ascii_lowercase().contains("ghostty"))
}

/// Initialize a raster protocol for the terminal currently attached to the
/// workspace. Inside tmux, render with true-color half blocks: ratatui-image
/// 8.x sends Kitty bitmap chunks in one oversized passthrough sequence, which
/// tmux drops while leaving invisible placement markers behind. Half blocks
/// preserve the same artwork and palette without terminal-specific payloads.
fn load_logo(client: Option<&TmuxClient>) -> Option<LogoImage> {
    let is_ghostty = client.map_or_else(env_is_ghostty, |client| client.is_ghostty);
    let font_size = client
        .and_then(|client| client.font_size)
        .or_else(terminal_font_size);
    let mut picker = if let Some(font_size) = font_size {
        Picker::from_fontsize(font_size)
    } else if is_ghostty {
        Picker::from_fontsize((10, 20))
    } else {
        Picker::from_query_stdio().ok()?
    };
    if client.is_some() {
        picker.set_protocol_type(ProtocolType::Halfblocks);
    } else if picker.protocol_type() != ProtocolType::Kitty && is_ghostty {
        picker.set_protocol_type(ProtocolType::Kitty);
    }
    // ratatui-image pads aspect-ratio rounding before converting to terminal
    // half blocks. Make those pixels match the roster canvas instead of the
    // library's transparent-black default, which rendered as a black bar.
    picker.set_background_color(LOGO_BACKGROUND);
    let image = image::load_from_memory_with_format(LOGO_PNG, image::ImageFormat::Png).ok()?;
    Some(LogoImage {
        protocol: picker.new_resize_protocol(image),
    })
}

#[cfg(test)]
pub fn render(f: &mut Frame, m: &Model, snap: &Snapshot) {
    render_with_logo(f, m, snap, None);
}

pub fn render_with_logo(f: &mut Frame, m: &Model, snap: &Snapshot, logo: Option<&mut LogoImage>) {
    let [main, status] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(f.area());
    let rail_width = if f.area().width >= 100 { 34 } else { 30 };
    let [rail, right] =
        Layout::horizontal([Constraint::Length(rail_width), Constraint::Min(0)]).areas(main);
    render_rail(f, rail, m, snap, logo);
    render_right(f, right, m, snap);
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

fn render_rail(
    f: &mut Frame,
    area: Rect,
    m: &Model,
    snap: &Snapshot,
    logo: Option<&mut LogoImage>,
) {
    let mut items: Vec<ListItem> = Vec::new();
    items.push(ListItem::new(Line::styled(
        " C O R T A D O",
        theme::brand(),
    )));
    items.push(ListItem::new(""));
    for t in &snap.teams {
        let sel = m.sel == Some(RailSel::Team(t.slug.clone()));
        let line = match &t.display {
            Ok(name) => Line::styled(format!(" {}", name.to_uppercase()), theme::team()),
            Err(_) => Line::styled(format!("{} INVALID", t.slug), theme::invalid()),
        };
        items.push(selected(ListItem::new(line), sel));
        for a in &t.agents {
            let sel = m.sel == Some(RailSel::Agent(t.slug.clone(), a.slug.clone()));
            let line = match &a.display {
                Ok(name) => {
                    let marker = if sel {
                        "›"
                    } else if a.sessions.is_empty() {
                        "◇"
                    } else {
                        "◆"
                    };
                    let mut spans = vec![
                        Span::styled(
                            format!(" {marker} "),
                            if sel {
                                theme::selection_marker()
                            } else if a.sessions.is_empty() {
                                theme::muted()
                            } else {
                                theme::badge()
                            },
                        ),
                        Span::raw(name.clone()),
                    ];
                    if !a.sessions.is_empty() {
                        spans.push(Span::styled(
                            format!("  {} live", a.sessions.len()),
                            Style::default()
                                .fg(badge_style(&a.sessions))
                                .add_modifier(Modifier::BOLD),
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
    let agent_count: usize = snap.teams.iter().map(|t| t.agents.len()).sum();
    let live_count: usize = snap
        .teams
        .iter()
        .flat_map(|t| &t.agents)
        .map(|a| a.sessions.len())
        .sum();
    items.push(ListItem::new(""));
    items.push(ListItem::new(Line::from(vec![
        Span::styled(format!(" {live_count} live"), theme::badge()),
        Span::styled(format!("  ·  {agent_count} agents"), theme::muted()),
    ])));
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border_focused())
        .title(Line::styled(" roster ", theme::title()));
    let inner = block.inner(area);
    f.render_widget(block, area);

    // Keep the mark anchored to the bottom without sacrificing roster rows.
    // Shrink it on shorter terminals, hiding it only when fewer than ten rows
    // remain after the complete roster.
    let logo_height = logo
        .is_some()
        .then(|| available_logo_height(inner.height, items.len()))
        .flatten();
    if let Some(logo_height) = logo_height {
        let [roster, logo_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(logo_height)]).areas(inner);
        f.render_widget(List::new(items), roster);
        if let Some(logo) = logo {
            // Fit at full size first, then render at 50% of those dimensions
            // and anchor the smaller rectangle to the pane's bottom-right.
            let padded = Rect::new(
                logo_area.x + 1,
                logo_area.y,
                logo_area.width.saturating_sub(2),
                logo_area.height,
            );
            let full_size = logo.protocol.size_for(Resize::Fit(None), padded);
            let scaled_bounds = scale_logo_rect(full_size);
            let scaled_size = logo.protocol.size_for(Resize::Fit(None), scaled_bounds);
            let bottom_right = bottom_right_rect(padded, scaled_size);
            f.render_stateful_widget(
                StatefulImage::default().resize(Resize::Fit(None)),
                bottom_right,
                &mut logo.protocol,
            );
        }
    } else {
        f.render_widget(List::new(items), inner);
    }
}

fn scale_logo_rect(rect: Rect) -> Rect {
    let scale = |value: u16| {
        let scaled = (u32::from(value) * u32::from(LOGO_SCALE_PERCENT) + 50) / 100;
        u16::try_from(scaled).unwrap_or(u16::MAX).max(1)
    };
    Rect::new(0, 0, scale(rect.width), scale(rect.height))
}

fn bottom_right_rect(container: Rect, size: Rect) -> Rect {
    let width = size.width.min(container.width);
    let height = size.height.min(container.height);
    Rect::new(
        container.x + container.width.saturating_sub(width),
        container.y + container.height.saturating_sub(height),
        width,
        height,
    )
}

fn available_logo_height(inner_height: u16, item_count: usize) -> Option<u16> {
    const MIN_LOGO_HEIGHT: u16 = 10;
    const MAX_LOGO_HEIGHT: u16 = 20;
    let item_count = u16::try_from(item_count).unwrap_or(u16::MAX);
    let height = inner_height.saturating_sub(item_count).min(MAX_LOGO_HEIGHT);
    (height >= MIN_LOGO_HEIGHT).then_some(height)
}

fn selected(item: ListItem<'_>, on: bool) -> ListItem<'_> {
    if on {
        item.style(theme::selection())
    } else {
        item
    }
}

fn render_right(f: &mut Frame, area: Rect, m: &Model, snap: &Snapshot) {
    let [identity, bar, content] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .areas(area);
    render_identity(f, identity, m, snap);
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
        Tab::Sessions => render_sessions(f, content, agent),
        Tab::Memory => render_memory(f, content, m, agent),
    }
}

fn render_identity(f: &mut Frame, area: Rect, m: &Model, snap: &Snapshot) {
    let Some(agent) = m.selected_agent(snap) else {
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled("  Choose an agent", theme::title()),
                Span::styled("  ·  then press Enter to open", theme::muted()),
            ])),
            area,
        );
        return;
    };
    let name = agent.display.as_deref().unwrap_or(agent.slug.as_str());
    let (team_name, reference) = match m.sel.as_ref() {
        Some(RailSel::Agent(team, agent_slug)) => {
            let display = snap
                .teams
                .iter()
                .find(|t| &t.slug == team)
                .and_then(|t| t.display.as_ref().ok())
                .map(String::as_str)
                .unwrap_or(team.as_str());
            (display, format!("{team}/{agent_slug}"))
        }
        _ => ("", String::new()),
    };
    let state = if agent.sessions.is_empty() {
        "IDLE"
    } else {
        "● LIVE"
    };
    let lines = vec![
        Line::from(vec![
            Span::styled(format!("  {name}"), theme::title()),
            Span::raw("   "),
            Span::styled(
                state,
                if agent.sessions.is_empty() {
                    theme::muted()
                } else {
                    theme::badge()
                },
            ),
            Span::styled(
                agent
                    .role
                    .as_deref()
                    .map(|role| format!("  ·  {role}"))
                    .unwrap_or_default(),
                theme::muted(),
            ),
        ]),
        Line::from(vec![
            Span::styled(format!("  {team_name}"), theme::model_col()),
            Span::styled(format!("  ·  {reference}"), theme::muted()),
            Span::styled(
                format!(
                    "  ·  {} / {}  ·  {}",
                    agent.runtime.as_deref().unwrap_or("?"),
                    agent.default_model.as_deref().unwrap_or("?"),
                    agent.branch.as_deref().unwrap_or("no branch")
                ),
                theme::muted(),
            ),
        ]),
    ];
    f.render_widget(Paragraph::new(lines), area);
}

fn render_sessions(f: &mut Frame, area: Rect, agent: Option<&AgentRow>) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(theme::border())
        .title(Line::styled("sessions", theme::title()));
    let Some(agent) = agent else {
        render_empty(
            f,
            area,
            block,
            "◇",
            "No agent selected",
            "Choose one from the roster on the left",
        );
        return;
    };
    if agent.sessions.is_empty() {
        f.render_widget(
            empty_paragraph(
                "◇",
                "No session is running",
                "Press n to choose a model and start one",
            )
            .block(block),
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
                    s.status.to_string(),
                    Style::default().fg(badge_style(std::slice::from_ref(s))),
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
        .header(
            Row::new(vec!["SESSION", "MODEL", "STATUS"])
                .style(theme::header())
                .bottom_margin(1),
        )
        .row_highlight_style(theme::selection())
        .block(block),
        area,
    );
}

/// Badge color for an agent's session set: any blocked → red, else any
/// working → green, else any idle → yellow, else dim. Used both for the
/// rail's aggregate live-count badge and per-row in the sessions table.
pub(crate) fn badge_style(sessions: &[crate::tui::snapshot::SessionRow]) -> ratatui::style::Color {
    use ratatui::style::Color;
    if sessions.iter().any(|s| s.status == AgentStatus::Blocked) {
        Color::Red
    } else if sessions.iter().any(|s| s.status == AgentStatus::Working) {
        Color::Green
    } else if sessions.iter().any(|s| s.status == AgentStatus::Idle) {
        Color::Yellow
    } else {
        Color::DarkGray
    }
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
        render_empty(
            f,
            area,
            block,
            "◇",
            "No agent selected",
            "Choose one from the roster on the left",
        );
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
    if let Some(s) = &m.status {
        let (icon, style) = if s.starts_with("error") {
            ("×", theme::status_error())
        } else if s.contains("warning") || s.starts_with("no ") {
            ("!", theme::status_warn())
        } else {
            ("✓", theme::status_msg())
        };
        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(format!(" {icon} "), style),
                Span::styled(s.clone(), style),
            ])),
            area,
        );
        return;
    }
    let actions = match m.tab {
        Tab::Sessions => [
            ("↑↓", "Navigate"),
            ("Enter", "Open"),
            ("n", "New session"),
            ("Tab", "Memory"),
            ("?", "Help"),
            ("q", "Quit"),
        ],
        Tab::Memory => [
            ("↑↓", "Agent"),
            ("j/k", "File"),
            ("Enter", "Preview"),
            ("Tab", "Sessions"),
            ("?", "Help"),
            ("q", "Quit"),
        ],
    };
    let mut spans = vec![Span::raw(" ")];
    for (i, (key, label)) in actions.iter().enumerate() {
        if i > 0 {
            spans.push(Span::styled("   ", theme::muted()));
        }
        spans.push(Span::styled(*key, theme::key()));
        spans.push(Span::styled(format!(" {label}"), theme::hint()));
    }
    f.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn empty_paragraph<'a>(icon: &'a str, title: &'a str, hint: &'a str) -> Paragraph<'a> {
    Paragraph::new(vec![
        Line::from(""),
        Line::styled(format!("    {icon}"), theme::brand()),
        Line::styled(format!("    {title}"), theme::title()),
        Line::styled(format!("    {hint}"), theme::muted()),
    ])
}

fn render_empty(f: &mut Frame, area: Rect, block: Block<'_>, icon: &str, title: &str, hint: &str) {
    f.render_widget(empty_paragraph(icon, title, hint).block(block), area);
}

fn render_popup(f: &mut Frame, popup: &Popup) {
    let height = match popup {
        Popup::NewAgent(_) => 14,
        Popup::Help => 18,
        _ => 12,
    };
    let area = centered(f.area(), 60, height);
    f.render_widget(Clear, area);
    f.render_widget(Block::default().style(theme::popup()), area);
    match popup {
        Popup::Help => {
            let rows = [
                ("↑ / ↓", "Navigate agents"),
                ("Enter", "Open or reattach selected agent"),
                ("n", "Start a new session with a model"),
                ("N", "Create a new agent"),
                ("Tab / m", "Switch Sessions and Memory"),
                ("e", "Edit selected memory file"),
                ("x", "End selected agent's sessions"),
                ("q", "Leave Cortado (sessions keep running)"),
            ];
            let mut lines = vec![
                Line::styled("  Work alongside persistent agents.", theme::primary()),
                Line::from(""),
            ];
            lines.extend(rows.into_iter().map(|(key, action)| {
                Line::from(vec![
                    Span::styled(format!("  {key:<11}"), theme::key()),
                    Span::styled(action, theme::muted()),
                ])
            }));
            lines.push(Line::from(""));
            lines.push(Line::styled("  Press ? or Esc to return", theme::hint()));
            f.render_widget(
                Paragraph::new(lines).block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(theme::border_focused())
                        .title(Line::styled(" Cortado help ", theme::brand())),
                ),
                area,
            );
        }
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
                            " new session · choose a model ",
                            theme::title(),
                        )),
                ),
                area,
            );
        }
        Popup::ConfirmKill { reference, count } => {
            f.render_widget(
                Paragraph::new(vec![
                    Line::from(""),
                    Line::styled(
                        format!("  End {count} session(s) for {reference}?"),
                        theme::primary(),
                    ),
                    Line::from(""),
                    Line::from(vec![
                        Span::styled("  Enter/y", theme::invalid()),
                        Span::styled(" Kill", theme::muted()),
                        Span::styled("     Esc/n", theme::key()),
                        Span::styled(" Cancel", theme::muted()),
                    ]),
                ])
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(theme::border())
                        .title(Line::styled(" confirm kill ", theme::invalid())),
                ),
                area,
            );
        }
        Popup::NewAgent(form) => render_form(f, area, form),
    }
}

fn render_form(f: &mut Frame, area: Rect, form: &NewAgentForm) {
    let field = |label: &str, value: &str, focused: bool| {
        let marker = if focused { "›" } else { " " };
        Line::from(vec![
            Span::styled(
                format!(" {marker} {label:<9}"),
                if focused {
                    theme::key()
                } else {
                    theme::muted()
                },
            ),
            Span::styled(
                value.to_string(),
                if focused {
                    theme::primary()
                } else {
                    theme::muted()
                },
            ),
        ])
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
    let runtime = Line::from(vec![
        Span::styled(
            if form.focus == FormFocus::Runtime {
                " › runtime  "
            } else {
                "   runtime  "
            },
            if form.focus == FormFocus::Runtime {
                theme::key()
            } else {
                theme::muted()
            },
        ),
        choice(
            "Claude",
            form.runtime == cortado_core::entity::RuntimeId::Claude,
        ),
        Span::raw("  "),
        choice(
            "Codex",
            form.runtime == cortado_core::entity::RuntimeId::Codex,
        ),
        Span::raw("  "),
        choice(
            "OpenCode",
            form.runtime == cortado_core::entity::RuntimeId::Opencode,
        ),
    ]);
    let lines = vec![
        Line::from(vec![
            Span::styled("   team      ", theme::muted()),
            Span::styled(form.team.to_string(), theme::model_col()),
        ]),
        Line::from(""),
        field("name", &form.name, form.focus == FormFocus::Name),
        field("role", &form.role, form.focus == FormFocus::Role),
        runtime,
        field("source", source, form.focus == FormFocus::Source),
        field(target_label, &form.target, form.focus == FormFocus::Target),
        field("branch", &form.branch, form.focus == FormFocus::Branch),
        Line::from(""),
        Line::from(vec![
            Span::styled("   Tab", theme::key()),
            Span::styled(" Next", theme::muted()),
            Span::styled("   ←/→", theme::key()),
            Span::styled(" Change", theme::muted()),
            Span::styled("   Enter", theme::key()),
            Span::styled(" Create", theme::muted()),
            Span::styled("   Esc", theme::key()),
            Span::styled(" Cancel", theme::muted()),
        ]),
    ];
    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(theme::border())
                .title(Line::styled(" new agent ", theme::title())),
        ),
        area,
    );
}

fn choice(label: &str, selected: bool) -> Span<'_> {
    if selected {
        Span::styled(format!(" {label} "), theme::selection())
    } else {
        Span::styled(label.to_string(), theme::muted())
    }
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
                    role: Some("Researches topics".into()),
                    runtime: Some("claude".into()),
                    default_model: Some("claude".into()),
                    branch: Some("agent/scout".into()),
                    sessions: vec![SessionRow {
                        name: "cortado_newsletter_scout_1".into(),
                        n: 1,
                        status: AgentStatus::Working,
                        pane_id: "w1:p2".into(),
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
        terminal.draw(|f| render(f, m, snap)).unwrap();
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
    fn tmux_client_parser_prefers_ghostty_and_tracks_every_client() {
        let clients = concat!(
            "55|2000|/dev/ttys005|215|61|16|34|xterm-ghostty\n",
            "44|1000|/dev/ttys004|120|40|9|18|xterm-256color\n"
        );
        let client = parse_tmux_clients(clients).unwrap();

        assert!(client.is_ghostty);
        assert_eq!(client.font_size, Some((16, 34)));
        assert!(client.signature.starts_with("44|1000"));
        assert!(client.signature.contains("\n55|2000"));
    }

    #[test]
    fn tmux_client_parser_waits_when_workspace_is_detached() {
        assert!(parse_tmux_clients("").is_none());
    }

    #[test]
    fn logo_shrinks_before_it_hides() {
        assert_eq!(available_logo_height(56, 23), Some(20));
        assert_eq!(available_logo_height(37, 23), Some(14));
        assert_eq!(available_logo_height(32, 23), None);
    }

    #[test]
    fn logo_is_scaled_to_fifty_percent_and_bottom_right_aligned() {
        let scaled = scale_logo_rect(Rect::new(0, 0, 30, 19));
        assert_eq!(scaled, Rect::new(0, 0, 15, 10));
        assert_eq!(
            bottom_right_rect(Rect::new(10, 20, 30, 20), scaled),
            Rect::new(25, 30, 15, 10)
        );
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
        assert!(text.contains("1 live"));
        assert!(text.contains("C O R T A D O"));
    }

    #[test]
    fn sessions_tab_shows_name_model_status() {
        let m = Model {
            sel: Some(RailSel::Agent(s("newsletter"), s("scout"))),
            ..Default::default()
        };
        let text = rendered(&m, &snap());
        assert!(text.contains("cortado_newsletter_scout_1"));
        assert!(text.contains("kimi"));
        assert!(text.contains("working"));
    }

    #[test]
    fn badge_prefers_blocked_over_working_over_idle() {
        use cortado_herdr::AgentStatus::*;
        let row = |status| crate::tui::snapshot::SessionRow {
            name: "cortado_t_a_1".into(),
            n: 1,
            status,
            pane_id: "w1:p2".into(),
            model: "m".into(),
        };
        assert_eq!(
            badge_style(&[row(Idle), row(Working)]),
            ratatui::style::Color::Green
        );
        assert_eq!(
            badge_style(&[row(Blocked), row(Working)]),
            ratatui::style::Color::Red
        );
        assert_eq!(badge_style(&[row(Idle)]), ratatui::style::Color::Yellow);
        assert_eq!(badge_style(&[]), ratatui::style::Color::DarkGray);
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
        assert!(text.contains("q Quit"));
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
        assert!(text.contains("End 2 session(s) for newsletter/scout?"));
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

    fn rendered_terminal(m: &Model, snap: &Snapshot) -> Terminal<TestBackend> {
        let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
        terminal.draw(|f| render(f, m, snap)).unwrap();
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
        assert!(text.contains("choose a model"));
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
        terminal.draw(|f| render(f, &m, &snap())).unwrap();
        assert!(any_cell_matches(terminal.backend(), |c| {
            c.bg == crate::tui::theme::SELECTION_BG
        }));
    }

    #[test]
    fn selected_team_keeps_semantic_fg() {
        // A selected TEAM row keeps its lavender team fg — semantic color wins
        // over the magenta selection fg, the same way a selected INVALID row
        // stays red. The selection bg still marks the row.
        let m = Model {
            sel: Some(RailSel::Team(s("newsletter"))),
            ..Default::default()
        };
        let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
        terminal.draw(|f| render(f, &m, &snap())).unwrap();
        assert!(any_cell_matches(terminal.backend(), |c| {
            c.fg == crate::tui::theme::TEAM_FG && c.bg == crate::tui::theme::SELECTION_BG
        }));
    }

    #[test]
    fn borders_use_theme_color() {
        let m = Model {
            sel: Some(RailSel::Agent(s("newsletter"), s("scout"))),
            ..Default::default()
        };
        let mut terminal = Terminal::new(TestBackend::new(80, 16)).unwrap();
        terminal.draw(|f| render(f, &m, &snap())).unwrap();
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
        assert!(text.contains("team      newsletter"));
        assert!(text.contains("› name     Editor")); // Name focused on a fresh form
        assert!(text.contains("runtime   Claude   Codex  OpenCode"));
        assert!(
            text.contains("Codex"),
            "Codex must be visible without cycling"
        );
        assert!(text.contains("source   new"));
        assert!(text.contains("Tab Next"));
    }
}
