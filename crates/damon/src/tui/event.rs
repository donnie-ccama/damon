use crate::tui::app::Event;
use ratatui::crossterm::event::{self, Event as CtEvent};
use std::time::Duration;

/// Next event, or Tick when `timeout` elapses (the 2s stateless refresh).
pub fn next(timeout: Duration) -> std::io::Result<Event> {
    if event::poll(timeout)? {
        match event::read()? {
            CtEvent::Key(k) => Ok(Event::Key(k)),
            _ => Ok(Event::Tick), // resize etc: redraw + refresh
        }
    } else {
        Ok(Event::Tick)
    }
}
