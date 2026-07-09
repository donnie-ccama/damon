//! `damon ui` — stateless ratatui TUI. Every tick and every action re-derives
//! the world from the filesystem and tmux; the Model holds only UI state.
pub mod app;
pub mod event;
pub mod popup;
pub mod snapshot;
pub mod view;

use app::{Action, Event, Model, Preview};
use damon_core::config::Config;
use damon_core::models::ModelsFile;
use damon_core::store::Store;
use damon_tmux::Tmux;
use ratatui::crossterm::event::KeyCode;
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use snapshot::Snapshot;
use std::io::stdout;
use std::io::IsTerminal;
use std::time::Duration;

pub fn run() -> anyhow::Result<()> {
    if !std::io::stdout().is_terminal() {
        anyhow::bail!("damon ui needs an interactive terminal");
    }
    let config = Config::load()?;
    // ratatui::init() enables raw mode + alternate screen and installs a
    // panic hook that restores the terminal.
    let terminal = ratatui::init();
    let result = event_loop(terminal, &config);
    ratatui::restore();
    result
}

fn event_loop(mut terminal: ratatui::DefaultTerminal, config: &Config) -> anyhow::Result<()> {
    let mut model = Model::default();
    let mut world = load_world(config);
    loop {
        terminal.draw(|f| match &world {
            Ok(snap) => view::render(f, &model, snap, chrono::Utc::now().timestamp()),
            Err(msg) => view::render_error(f, msg),
        })?;
        let ev = event::next(Duration::from_secs(2))?;
        let mut refresh = matches!(ev, Event::Tick);
        match &world {
            Ok(snap) => {
                for action in app::update(&mut model, snap, ev) {
                    match action {
                        Action::Edit { path } => {
                            let status = match suspend(&mut terminal, || {
                                crate::commands::memory::spawn_editor(&path)
                            }) {
                                Ok(Ok(s)) if s.success() => format!("edited {}", path.display()),
                                Ok(Ok(s)) => format!("editor exited {}", s.code().unwrap_or(-1)),
                                Ok(Err(e)) => format!("error: {e:#}"),
                                Err(e) => format!("error: {e}"),
                            };
                            model.status = Some(status);
                            refresh = true;
                        }
                        other => {
                            if execute_action(other, &mut model) {
                                return Ok(());
                            }
                            refresh = true;
                        }
                    }
                }
            }
            Err(_) => {
                if let Event::Key(k) = ev {
                    if k.code == KeyCode::Char('q') {
                        return Ok(());
                    }
                }
                refresh = true; // spec: error state retries every tick
            }
        }
        if refresh {
            world = load_world(config);
        }
    }
}

/// The whole world, from scratch: Store + tmux + models.toml.
fn load_world(config: &Config) -> Result<Snapshot, String> {
    let inner = || -> anyhow::Result<Snapshot> {
        let store = Store::new(config.root()?);
        let tmux = Tmux::new(config.tmux.socket.clone());
        let live = snapshot::live_sessions(&tmux)?;
        let models = ModelsFile::load()?;
        Ok(Snapshot::build(&store, &live, &models)?)
    };
    inner().map_err(|e| format!("{e:#}"))
}

/// Leave the alternate screen + raw mode, run `f` (e.g. an editor) against the
/// real terminal, then restore the TUI and force a full redraw.
fn suspend<T>(
    terminal: &mut ratatui::DefaultTerminal,
    f: impl FnOnce() -> T,
) -> std::io::Result<T> {
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;
    let out = f();
    execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    terminal.clear()?;
    Ok(out)
}

/// Run one Action through the same cores the CLI verbs use. Returns true on quit.
fn execute_action(action: Action, m: &mut Model) -> bool {
    match action {
        Action::Quit => return true,
        Action::Open {
            reference,
            model,
            fresh,
        } => match crate::commands::open::open_session(&reference, model.as_deref(), fresh) {
            Ok(out) => {
                let mut msg = format!("session {}", out.session);
                if !out.warnings.is_empty() {
                    msg.push_str(&format!(" (warning: {})", out.warnings.join("; ")));
                }
                m.status = Some(msg);
            }
            Err(e) => m.status = Some(format!("error: {e:#}")),
        },
        Action::Kill { reference } => match crate::commands::sessions::kill_agent(&reference) {
            Ok(out) if out.failed.is_empty() => {
                m.status = Some(format!("killed {}", out.killed.len()));
            }
            Ok(out) => {
                m.status = Some(format!(
                    "killed {}, failed {}: {}",
                    out.killed.len(),
                    out.failed.len(),
                    out.failed.join("; ")
                ));
            }
            Err(e) => m.status = Some(format!("error: {e:#}")),
        },
        Action::CreateAgent {
            reference,
            runtime,
            role,
            repo,
            branch,
        } => match crate::commands::agent::create(&reference, runtime, role, repo, branch) {
            Ok((team, slug)) => m.status = Some(format!("created agent {team}/{slug}")),
            Err(e) => m.status = Some(format!("error: {e:#}")),
        },
        Action::Preview { title, path } => match std::fs::read_to_string(&path) {
            Ok(content) => {
                m.preview = Some(Preview {
                    title,
                    content,
                    scroll: 0,
                    path,
                })
            }
            Err(e) => m.status = Some(format!("error: {}: {e}", path.display())),
        },
        Action::Edit { .. } => {} // handled in event_loop before execute_action
    }
    false
}
