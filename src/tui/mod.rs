mod animations;
mod helpers;
mod input;
mod render;
mod state;

use crate::config::SystemConfig;
use crate::preferences::UserPreferences;
use anyhow::Result;
use crossterm::event::{self, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, Stdout};
use std::time::Duration;

pub fn run(config: &mut SystemConfig, prefs: &mut UserPreferences) -> Result<()> {
    let mut dashboard = state::Dashboard::new(config.clone(), prefs.clone())?;
    let mut terminal = TerminalSession::new()?;

    loop {
        terminal.terminal.draw(|frame| dashboard.render(frame))?;
        dashboard.perform_pending_actions()?;

        if event::poll(Duration::from_millis(120))? {
            if let Event::Key(key) = event::read()? {
                if dashboard.handle_key(key)? {
                    break;
                }
            }
        }
    }

    *config = dashboard.config;
    *prefs = dashboard.prefs;
    Ok(())
}

struct TerminalSession {
    terminal: Terminal<CrosstermBackend<Stdout>>,
}

impl TerminalSession {
    fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal })
    }
}

impl Drop for TerminalSession {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(self.terminal.backend_mut(), LeaveAlternateScreen);
        let _ = self.terminal.show_cursor();
    }
}
