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
use std::io::{self, Write};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressState {
    Hidden,
    Normal(u8),
    Error(Option<u8>),
    Indeterminate,
}

impl ProgressState {
    pub(crate) fn osc9_4_sequence(self) -> Vec<u8> {
        let payload = match self {
            Self::Hidden => "\x1b]9;4;0\x07".to_string(),
            Self::Normal(percent) => format!("\x1b]9;4;1;{}\x07", percent.min(100)),
            Self::Error(Some(percent)) => format!("\x1b]9;4;2;{}\x07", percent.min(100)),
            Self::Error(None) => "\x1b]9;4;2\x07".to_string(),
            Self::Indeterminate => "\x1b]9;4;3\x07".to_string(),
        };
        payload.into_bytes()
    }
}

pub struct Terminal {
    terminal: ratatui::Terminal<CrosstermBackend<io::Stdout>>,
    last_progress: ProgressState,
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
        Ok(Self {
            terminal,
            last_progress: ProgressState::Hidden,
        })
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

    pub fn set_progress(&mut self, state: ProgressState) -> Result<()> {
        if self.last_progress == state {
            return Ok(());
        }

        self.terminal
            .backend_mut()
            .write_all(&state.osc9_4_sequence())
            .map_err(crate::error::Error::Io)?;
        Write::flush(self.terminal.backend_mut()).map_err(crate::error::Error::Io)?;
        self.last_progress = state;
        Ok(())
    }

    /// Suspend the TUI to allow interactive command execution.
    /// Leaves alternate screen and disables raw mode.
    pub fn suspend(&mut self) -> Result<()> {
        self.set_progress(ProgressState::Hidden)?;
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
        let _ = self.set_progress(ProgressState::Hidden);
        disable_raw_mode().ok();
        execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        )
        .ok();
    }
}

#[cfg(test)]
mod tests {
    use super::ProgressState;

    #[test]
    fn osc9_4_hidden_sequence_is_correct() {
        assert_eq!(ProgressState::Hidden.osc9_4_sequence(), b"\x1b]9;4;0\x07");
    }

    #[test]
    fn osc9_4_indeterminate_sequence_is_correct() {
        assert_eq!(
            ProgressState::Indeterminate.osc9_4_sequence(),
            b"\x1b]9;4;3\x07"
        );
    }

    #[test]
    fn osc9_4_percent_sequence_clamps_values() {
        assert_eq!(
            ProgressState::Normal(150).osc9_4_sequence(),
            b"\x1b]9;4;1;100\x07"
        );
    }

    #[test]
    fn osc9_4_error_without_percent_sequence_is_correct() {
        assert_eq!(
            ProgressState::Error(None).osc9_4_sequence(),
            b"\x1b]9;4;2\x07"
        );
    }
}
