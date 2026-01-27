use crate::error::Result;
use crossterm::{
    cursor::MoveTo,
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, Clear, ClearType, EnterAlternateScreen,
        LeaveAlternateScreen,
    },
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

    /// Suspend the TUI to allow interactive command execution.
    /// Leaves alternate screen and disables raw mode.
    pub fn suspend(&mut self) -> Result<()> {
        disable_raw_mode().map_err(|e| crate::error::Error::Terminal(e.to_string()))?;
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .map_err(|e| crate::error::Error::Terminal(e.to_string()))?;
        Ok(())
    }

    /// Resume the TUI after interactive command execution.
    /// Re-enters alternate screen and enables raw mode.
    pub fn resume(&mut self) -> Result<()> {
        enable_raw_mode().map_err(|e| crate::error::Error::Terminal(e.to_string()))?;
        execute!(
            self.terminal.backend_mut(),
            EnterAlternateScreen,
            EnableMouseCapture,
            // Clear the actual terminal screen and reset cursor
            Clear(ClearType::All),
            MoveTo(0, 0)
        )
        .map_err(|e| crate::error::Error::Terminal(e.to_string()))?;
        // Clear ratatui's internal buffer to force a full redraw
        self.terminal
            .clear()
            .map_err(|e| crate::error::Error::Terminal(e.to_string()))?;
        Ok(())
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
