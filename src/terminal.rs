use crate::error::Result;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;

pub struct Terminal {
    terminal: ratatui::Terminal<CrosstermBackend<io::Stdout>>,
}

impl Terminal {
    pub fn new() -> Result<Self> {
        enable_raw_mode().map_err(|e| crate::error::Error::Terminal(e.to_string()))?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
            .map_err(|e| crate::error::Error::Terminal(e.to_string()))?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = ratatui::Terminal::new(backend)
            .map_err(|e| crate::error::Error::Terminal(e.to_string()))?;
        Ok(Self { terminal })
    }

    pub fn draw<F>(&mut self, f: F) -> Result<()>
    where
        F: FnOnce(&mut Frame),
    {
        self.terminal
            .draw(f)
            .map_err(|e| crate::error::Error::Terminal(e.to_string()))?;
        Ok(())
    }

    pub fn size(&self) -> Result<Rect> {
        self.terminal
            .size()
            .map_err(|e| crate::error::Error::Terminal(e.to_string()))
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        disable_raw_mode().ok();
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .ok();
    }
}
