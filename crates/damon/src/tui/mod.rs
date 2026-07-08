//! `damon ui` — stateless ratatui TUI. Every tick and every action re-derives
//! the world from the filesystem and tmux; the Model holds only UI state.
use std::io::IsTerminal;

pub fn run() -> anyhow::Result<()> {
    if !std::io::stdout().is_terminal() {
        anyhow::bail!("damon ui needs an interactive terminal");
    }
    // ratatui::init() enables raw mode + alternate screen and installs a
    // panic hook that restores the terminal, so a TUI crash never mangles
    // the user's shell.
    let terminal = ratatui::init();
    let result = event_loop(terminal);
    ratatui::restore();
    result
}

fn event_loop(mut terminal: ratatui::DefaultTerminal) -> anyhow::Result<()> {
    use ratatui::crossterm::event::{self, Event, KeyCode};
    loop {
        terminal.draw(|f| {
            f.render_widget(
                ratatui::widgets::Paragraph::new("damon — press q to quit"),
                f.area(),
            );
        })?;
        if let Event::Key(k) = event::read()? {
            if k.code == KeyCode::Char('q') {
                return Ok(());
            }
        }
    }
}
