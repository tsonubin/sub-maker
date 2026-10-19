use std::io::{self, stdout, Stdout};
use std::time::Duration;

use crossterm::event::{
    self, DisableBracketedPaste, EnableBracketedPaste, Event, KeyEvent, KeyEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

pub struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl Tui {
    pub fn new() -> anyhow::Result<Self> {
        enable_raw_mode()?;
        let mut out = stdout();
        execute!(out, EnterAlternateScreen, EnableBracketedPaste)?;
        let mut terminal = Terminal::new(CrosstermBackend::new(out))?;
        terminal.clear()?;
        Ok(Self { terminal })
    }

    pub fn draw<F>(&mut self, f: F) -> anyhow::Result<()>
    where
        F: FnOnce(&mut ratatui::Frame),
    {
        self.terminal.draw(f)?;
        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            DisableBracketedPaste,
            LeaveAlternateScreen
        );
        let _ = self.terminal.show_cursor();
    }
}

/// One unit of user input for the wizard loops.
pub enum Input {
    Key(KeyEvent),
    Paste(String),
}

pub fn next_input(timeout: Duration) -> anyhow::Result<Option<Input>> {
    if !event::poll(timeout)? {
        return Ok(None);
    }
    match event::read()? {
        Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
            Ok(Some(Input::Key(key)))
        }
        Event::Paste(text) => Ok(Some(Input::Paste(text))),
        _ => Ok(None),
    }
}

pub fn cancel_err() -> anyhow::Error {
    anyhow::Error::new(UserCancelled)
}

/// Marker for a deliberate wizard abort (Esc / Ctrl-C / q): not a failure.
#[derive(Debug)]
pub struct UserCancelled;

impl std::fmt::Display for UserCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("cancelled")
    }
}

impl std::error::Error for UserCancelled {}

#[allow(dead_code)]
pub fn restore_for_error() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);
}
