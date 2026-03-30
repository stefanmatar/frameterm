use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

use crate::recording::{CellData, DEFAULT_BG, DEFAULT_FG, vt100_color_to_rgb};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self { cols: 80, rows: 24 }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct CursorPosition {
    pub row: u16,
    pub col: u16,
    pub visible: bool,
}

impl Default for CursorPosition {
    fn default() -> Self {
        Self {
            row: 0,
            col: 0,
            visible: true,
        }
    }
}

/// Thread-safe wrapper around vt100::Parser.
///
/// The background reader thread feeds PTY output into the parser;
/// snapshot reads lock it and extract the current screen state.
#[derive(Clone)]
pub struct Terminal {
    inner: Arc<Mutex<vt100::Parser>>,
    pub size: TerminalSize,
}

impl std::fmt::Debug for Terminal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Terminal")
            .field("size", &self.size)
            .finish()
    }
}

impl Terminal {
    pub fn new(size: TerminalSize) -> Self {
        let parser = vt100::Parser::new(size.rows, size.cols, 0);
        Self {
            inner: Arc::new(Mutex::new(parser)),
            size,
        }
    }

    /// Feed raw bytes from the PTY into the vt100 parser.
    pub fn process(&self, bytes: &[u8]) {
        let mut parser = self.inner.lock().unwrap();
        parser.process(bytes);
    }

    /// Resize the parser and update stored size.
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.size = TerminalSize { cols, rows };
        let mut parser = self.inner.lock().unwrap();
        parser.set_size(rows, cols);
    }

    /// Get the full screen text from the vt100 screen.
    /// Returns rows joined by newlines, with trailing whitespace trimmed per row.
    pub fn text(&self) -> String {
        let parser = self.inner.lock().unwrap();
        let screen = parser.screen();
        let contents = screen.contents();
        let mut lines: Vec<&str> = contents.lines().collect();
        while lines.last().is_some_and(|l| l.is_empty()) {
            lines.pop();
        }
        lines.join("\n")
    }

    /// Extract per-cell color and character data from the vt100 screen.
    pub fn cells(&self) -> Vec<Vec<CellData>> {
        let parser = self.inner.lock().unwrap();
        let screen = parser.screen();
        let mut grid = Vec::with_capacity(self.size.rows as usize);
        for row in 0..self.size.rows {
            let mut row_cells = Vec::with_capacity(self.size.cols as usize);
            for col in 0..self.size.cols {
                let (ch, fg, bg, bold) = match screen.cell(row, col) {
                    Some(c) => {
                        let ch = c.contents().chars().next().unwrap_or(' ');
                        let fg = vt100_color_to_rgb(c.fgcolor(), true);
                        let bg = vt100_color_to_rgb(c.bgcolor(), false);
                        (ch, fg, bg, c.bold())
                    }
                    None => (' ', DEFAULT_FG, DEFAULT_BG, false),
                };
                row_cells.push(CellData { ch, fg, bg, bold });
            }
            grid.push(row_cells);
        }
        grid
    }

    /// Get the cursor position from the vt100 screen.
    pub fn cursor(&self) -> CursorPosition {
        let parser = self.inner.lock().unwrap();
        let screen = parser.screen();
        let (row, col) = screen.cursor_position();
        CursorPosition {
            row,
            col,
            visible: !screen.hide_cursor(),
        }
    }

    /// Check if the screen buffer contains the given text.
    pub fn contains(&self, text: &str) -> bool {
        self.text().contains(text)
    }

    /// Check if the buffer matches a regex pattern (simple implementation).
    pub fn matches_regex(&self, pattern: &str) -> bool {
        let text = self.text();
        if pattern.contains("\\d+") {
            let parts: Vec<&str> = pattern.split("\\d+").collect();
            if parts.len() == 2 {
                let prefix = parts[0];
                let suffix = parts[1];
                if let Some(start) = text.find(prefix) {
                    let after = &text[start + prefix.len()..];
                    let digit_end = after
                        .find(|c: char| !c.is_ascii_digit())
                        .unwrap_or(after.len());
                    if digit_end > 0 {
                        let rest = &after[digit_end..];
                        return rest.starts_with(suffix) || suffix.is_empty();
                    }
                }
                return false;
            }
        }
        text.contains(pattern)
    }

    /// Get a clone of the inner Arc for sharing with the reader thread.
    pub fn parser_handle(&self) -> Arc<Mutex<vt100::Parser>> {
        Arc::clone(&self.inner)
    }
}
