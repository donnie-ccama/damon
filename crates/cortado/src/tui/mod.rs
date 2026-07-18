//! `cortado ui` — stateless ratatui TUI. Every tick and every action re-derives
//! the world from the filesystem and tmux; the Model holds only UI state.
pub mod app;
pub mod event;
pub mod popup;
pub mod snapshot;
pub mod theme;
pub mod view;

use app::{Action, Event, Model, Preview};
use cortado_core::config::Config;
use cortado_core::models::ModelsFile;
use cortado_core::store::Store;
use cortado_tmux::Tmux;
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
    let config = Config::load()?;
    if should_bootstrap(
        &config,
        std::env::var_os("CORTADO_WORKSPACE").is_some(),
        std::env::var_os("TMUX").is_some(),
    ) {
        return bootstrap_workspace(&config);
    }
    if !std::io::stdout().is_terminal() {
        anyhow::bail!("cortado ui needs an interactive terminal");
    }
    // ratatui::init() enables raw mode + alternate screen and installs a
    // panic hook that restores the terminal.
    let terminal = ratatui::init();
    let result = event_loop(terminal, &config);
    ratatui::restore();
    result
}

/// Workspace mode, invoked from a plain shell: become the workspace instead
/// of drawing the rail inline. Inside the workspace pane (CORTADO_WORKSPACE)
/// or any tmux ($TMUX) we draw the rail directly.
fn should_bootstrap(config: &Config, in_workspace_pane: bool, in_tmux: bool) -> bool {
    config.terminal.launcher == cortado_core::config::Launcher::Workspace
        && !in_workspace_pane
        && !in_tmux
}

fn bootstrap_workspace(config: &Config) -> anyhow::Result<()> {
    let tmux = Tmux::new(config.tmux.socket.clone());
    let rail = vec![
        std::env::current_exe()?.display().to_string(),
        "ui".to_string(),
    ];
    let home = std::env::var("HOME").unwrap_or_else(|_| "/".to_string());
    cortado_term::workspace::ensure_workspace(&tmux, std::path::Path::new(&home), &rail)?;
    cortado_term::open_window(
        config.terminal.window,
        &config.tmux.socket,
        cortado_term::workspace::WORKSPACE_SESSION,
    )?;
    println!("workspace {}", cortado_term::workspace::WORKSPACE_SESSION);
    Ok(())
}

fn event_loop(mut terminal: ratatui::DefaultTerminal, config: &Config) -> anyhow::Result<()> {
    let mut model = Model::default();
    let mut logo = view::LogoState::new();
    let mut world = load_world(config);
    loop {
        // The workspace process can start detached and survive several
        // terminal windows. Refresh between event reads so a newly attached
        // client supplies the right cell metrics for the logo.
        logo.refresh();
        terminal.draw(|f| match &world {
            Ok(snap) => view::render_with_logo(
                f,
                &model,
                snap,
                chrono::Utc::now().timestamp(),
                logo.image_mut(),
            ),
            Err(msg) => view::render_error(f, msg),
        })?;
        let ev = event::next(Duration::from_secs(2))?;
        let mut refresh = matches!(ev, Event::Tick);
        match &world {
            Ok(snap) => {
                for action in app::update(&mut model, snap, ev) {
                    match action {
                        Action::Edit { path } => {
                            let result = suspend(&mut terminal, || {
                                crate::commands::memory::spawn_editor(&path)
                            });
                            // A direct terminal may discard graphics when the
                            // alternate screen changes; tmux clients may also
                            // have changed while the editor was open.
                            logo.invalidate();
                            let status = match result {
                                Ok(Ok(s)) if s.success() => {
                                    // File may have changed; refresh the open
                                    // preview if it's this file.
                                    match app::refresh_preview(&mut model, &path) {
                                        Ok(()) => format!("edited {}", path.display()),
                                        Err(e) => {
                                            format!("edited {} (preview: {e})", path.display())
                                        }
                                    }
                                }
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
/// real terminal, then restore the TUI and force a full redraw. Every error
/// path leaves the terminal in the TUI-expected state (raw on + alt screen),
/// because the event loop keeps drawing after this returns.
fn suspend<T>(
    terminal: &mut ratatui::DefaultTerminal,
    f: impl FnOnce() -> T,
) -> std::io::Result<T> {
    // Leave TUI mode for the child process.
    disable_raw_mode()?;
    if let Err(e) = execute!(stdout(), LeaveAlternateScreen) {
        // Raw mode is already off; restore it so we don't return with the
        // terminal in a mixed (raw-off, still-alt-screen) state.
        let _ = enable_raw_mode();
        return Err(e);
    }

    let out = f();

    // Restore TUI mode. Attempt both steps even if the first fails, and
    // report the first error rather than short-circuiting mid-restore.
    let enter = execute!(stdout(), EnterAlternateScreen);
    let raw = enable_raw_mode();
    enter.and(raw)?;
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
                    max_scroll: std::cell::Cell::new(0),
                })
            }
            Err(e) => m.status = Some(format!("error: {}: {e}", path.display())),
        },
        Action::Edit { .. } => {} // handled in event_loop before execute_action
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use cortado_core::config::Launcher;

    #[test]
    fn bootstraps_only_outside_workspace_and_tmux_in_workspace_mode() {
        let ws = |l: Launcher| {
            let mut c = Config::default();
            c.terminal.launcher = l;
            c
        };
        assert!(should_bootstrap(&ws(Launcher::Workspace), false, false));
        assert!(!should_bootstrap(&ws(Launcher::Workspace), true, false)); // already the rail
        assert!(!should_bootstrap(&ws(Launcher::Workspace), false, true)); // user is in tmux
        assert!(!should_bootstrap(&ws(Launcher::Ghostty), false, false)); // legacy mode
    }
}
