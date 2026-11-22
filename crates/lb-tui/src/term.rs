use std::io::{self, Stdout};

use crossterm::{
    event::{
        DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture,
    },
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use ratatui::{backend::CrosstermBackend, Terminal};

use crate::event::{translate_event, EditorEvent};
use crate::render::{render_frame, View};

// ─── Tui ─────────────────────────────────────────────────────────────────────

/// Owns the terminal handle. Restores the terminal on `Drop` even if
/// `leave()` is never called explicitly.
pub struct Tui {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    /// Guards against double-cleanup (e.g. `leave()` then `Drop`).
    active: bool,
}

impl Tui {
    /// Enter alternate screen, enable raw mode and mouse capture.
    pub fn enter() -> io::Result<Self> {
        enable_raw_mode()?;
        let mut out = io::stdout();
        execute!(
            out,
            EnterAlternateScreen,
            EnableMouseCapture,
            EnableBracketedPaste,
        )?;
        let backend = CrosstermBackend::new(io::stdout());
        let terminal = Terminal::new(backend)?;
        Ok(Self { terminal, active: true })
    }

    /// Restore the terminal. Idempotent.
    pub fn leave(mut self) -> io::Result<()> {
        self.cleanup()
    }

    /// Block until the next terminal event, then translate and return it.
    pub fn next_event(&self) -> io::Result<EditorEvent> {
        loop {
            let raw = crossterm::event::read()?;
            if let Some(ev) = translate_event(raw) {
                return Ok(ev);
            }
            // Unknown event variant — loop and read the next one.
        }
    }

    /// Render one frame from `view`.
    pub fn render(&mut self, view: &View) -> io::Result<()> {
        self.terminal.draw(|frame| render_frame(frame, view))?;
        Ok(())
    }

    /// Terminal dimensions as `(width, height)`.
    pub fn size(&self) -> io::Result<(u16, u16)> {
        let size = self.terminal.size()?;
        Ok((size.width, size.height))
    }

    fn cleanup(&mut self) -> io::Result<()> {
        if self.active {
            self.active = false;
            disable_raw_mode()?;
            execute!(
                io::stdout(),
                LeaveAlternateScreen,
                DisableMouseCapture,
                DisableBracketedPaste,
            )?;
        }
        Ok(())
    }
}

impl Drop for Tui {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
