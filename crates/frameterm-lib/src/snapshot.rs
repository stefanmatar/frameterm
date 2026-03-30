use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::terminal::{CursorPosition, TerminalSize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UiElement {
    #[serde(rename = "type")]
    pub element_type: String,
    pub text: String,
    pub row: u16,
    pub col: u16,
    pub width: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub focused: Option<bool>,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub size: TerminalSize,
    pub cursor: CursorPosition,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub content_hash: String,
    pub elements: Vec<UiElement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotFormat {
    Json,
    Compact,
    Text,
}

impl SnapshotFormat {
    pub fn parse(s: &str) -> Self {
        match s {
            "compact" => Self::Compact,
            "text" => Self::Text,
            _ => Self::Json,
        }
    }
}

/// Compute a SHA-256 content hash for the screen text.
pub fn content_hash(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let result = hasher.finalize();
    format!("{:064x}", result)
}

/// Detect UI elements in the screen buffer.
pub fn detect_elements(text: &str) -> Vec<UiElement> {
    let mut elements = Vec::new();

    for (row_idx, line) in text.lines().enumerate() {
        let row = row_idx as u16;
        detect_buttons(line, row, &mut elements);
        detect_toggles(line, row, &mut elements);
    }

    elements
}

fn detect_buttons(line: &str, row: u16, elements: &mut Vec<UiElement>) {
    let mut search_from = 0;
    while let Some(start) = line[search_from..].find('[') {
        let abs_start = search_from + start;
        if let Some(end) = line[abs_start..].find(']') {
            let full = &line[abs_start..abs_start + end + 1];
            let inner = &line[abs_start + 1..abs_start + end];
            if inner == "x" || inner == " " || inner.is_empty() {
                search_from = abs_start + end + 1;
                continue;
            }
            let text_str = full.to_string();
            let focused = is_focused_button(line, abs_start);
            elements.push(UiElement {
                element_type: "button".to_string(),
                text: text_str,
                row,
                col: abs_start as u16,
                width: (end + 1) as u16,
                checked: None,
                focused: if focused { Some(true) } else { None },
                confidence: 0.9,
            });
            search_from = abs_start + end + 1;
        } else {
            break;
        }
    }
}

fn detect_toggles(line: &str, row: u16, elements: &mut Vec<UiElement>) {
    let toggle_patterns = [("[x]", true), ("[ ]", false)];

    for &(pattern, checked) in &toggle_patterns {
        let mut search_from = 0;
        while let Some(pos) = line[search_from..].find(pattern) {
            let abs_pos = search_from + pos;
            let after = &line[abs_pos + pattern.len()..];
            let label = after.split_whitespace().next().unwrap_or("");
            let full_text = if label.is_empty() {
                pattern.to_string()
            } else {
                format!("{pattern} {label}")
            };
            elements.push(UiElement {
                element_type: "toggle".to_string(),
                text: pattern.to_string(),
                row,
                col: abs_pos as u16,
                width: full_text.len() as u16,
                checked: Some(checked),
                focused: None,
                confidence: 0.9,
            });
            search_from = abs_pos + pattern.len();
        }
    }
}

fn is_focused_button(line: &str, pos: usize) -> bool {
    if pos > 0 {
        let before = &line[..pos];
        let trimmed = before.trim_end();
        if trimmed.ends_with('>') {
            return true;
        }
    }
    false
}

/// Format a snapshot as plain text.
pub fn format_as_text(snap: &Snapshot) -> String {
    let text = snap.text.as_deref().unwrap_or("");
    let cursor_line = format!(
        "[cursor: row={}, col={}, visible={}]",
        snap.cursor.row, snap.cursor.col, snap.cursor.visible
    );
    format!("{text}\n{cursor_line}")
}
