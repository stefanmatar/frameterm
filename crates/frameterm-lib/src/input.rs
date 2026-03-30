use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InputEvent {
    pub kind: InputEventKind,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum InputEventKind {
    Key(KeyEvent),
    Click(ClickEvent),
    Scroll(ScrollEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KeyEvent {
    pub key: String,
    pub display: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClickEvent {
    pub row: u16,
    pub col: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScrollEvent {
    pub direction: ScrollDirection,
    pub lines: u16,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScrollDirection {
    Up,
    Down,
}

/// Parse a key name into a validated key event.
/// Returns None if the key name is invalid.
pub fn parse_key(name: &str) -> Option<KeyEvent> {
    let name = name.trim();

    if let Some(rest) = name.strip_prefix("Ctrl+") {
        if rest.len() == 1 && rest.chars().next().unwrap().is_alphabetic() {
            return Some(KeyEvent {
                key: name.to_string(),
                display: name.to_string(),
            });
        }
        return None;
    }

    if let Some(rest) = name.strip_prefix("Alt+") {
        if rest.len() == 1 && rest.chars().next().unwrap().is_alphabetic() {
            return Some(KeyEvent {
                key: name.to_string(),
                display: name.to_string(),
            });
        }
        return None;
    }

    if let Some(rest) = name.strip_prefix("Shift+") {
        if rest.len() == 1 && rest.chars().next().unwrap().is_alphabetic() {
            return Some(KeyEvent {
                key: name.to_string(),
                display: rest.to_uppercase(),
            });
        }
        return None;
    }

    if let Some(rest) = name.strip_prefix('F') {
        if let Ok(n) = rest.parse::<u8>() {
            if (1..=12).contains(&n) {
                return Some(KeyEvent {
                    key: name.to_string(),
                    display: name.to_string(),
                });
            }
        }
    }

    match name {
        "Enter" | "Return" => Some(KeyEvent {
            key: "Enter".to_string(),
            display: "Enter".to_string(),
        }),
        "Tab" => Some(KeyEvent {
            key: "Tab".to_string(),
            display: "Tab".to_string(),
        }),
        "Escape" | "Esc" => Some(KeyEvent {
            key: "Escape".to_string(),
            display: "Esc".to_string(),
        }),
        "Space" => Some(KeyEvent {
            key: "Space".to_string(),
            display: "Space".to_string(),
        }),
        "Backspace" => Some(KeyEvent {
            key: "Backspace".to_string(),
            display: "Backspace".to_string(),
        }),
        "Delete" => Some(KeyEvent {
            key: "Delete".to_string(),
            display: "Delete".to_string(),
        }),
        "Up" => Some(KeyEvent {
            key: "Up".to_string(),
            display: "Up".to_string(),
        }),
        "Down" => Some(KeyEvent {
            key: "Down".to_string(),
            display: "Down".to_string(),
        }),
        "Left" => Some(KeyEvent {
            key: "Left".to_string(),
            display: "Left".to_string(),
        }),
        "Right" => Some(KeyEvent {
            key: "Right".to_string(),
            display: "Right".to_string(),
        }),
        "Home" => Some(KeyEvent {
            key: "Home".to_string(),
            display: "Home".to_string(),
        }),
        "End" => Some(KeyEvent {
            key: "End".to_string(),
            display: "End".to_string(),
        }),
        "PageUp" => Some(KeyEvent {
            key: "PageUp".to_string(),
            display: "PgUp".to_string(),
        }),
        "PageDown" => Some(KeyEvent {
            key: "PageDown".to_string(),
            display: "PgDn".to_string(),
        }),
        "Insert" => Some(KeyEvent {
            key: "Insert".to_string(),
            display: "Ins".to_string(),
        }),
        _ if name.len() == 1 => {
            let ch = name.chars().next().unwrap();
            if ch.is_ascii_graphic() || ch == ' ' {
                Some(KeyEvent {
                    key: name.to_string(),
                    display: name.to_string(),
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Convert a key name to the bytes that should be written to a PTY.
pub fn key_to_bytes(name: &str) -> Option<Vec<u8>> {
    let name = name.trim();

    if let Some(rest) = name.strip_prefix("Ctrl+") {
        if rest.len() == 1 {
            let ch = rest.chars().next().unwrap().to_ascii_uppercase();
            if ch.is_ascii_alphabetic() {
                let ctrl_byte = (ch as u8) - b'A' + 1;
                return Some(vec![ctrl_byte]);
            }
        }
        return None;
    }

    if let Some(rest) = name.strip_prefix("Alt+") {
        if rest.len() == 1 {
            let ch = rest.chars().next().unwrap();
            if ch.is_ascii_alphabetic() {
                return Some(vec![0x1b, ch.to_ascii_lowercase() as u8]);
            }
        }
        return None;
    }

    if let Some(rest) = name.strip_prefix("Shift+") {
        if rest.len() == 1 {
            let ch = rest.chars().next().unwrap();
            if ch.is_ascii_alphabetic() {
                return Some(ch.to_ascii_uppercase().to_string().into_bytes());
            }
        }
        return None;
    }

    if let Some(rest) = name.strip_prefix('F') {
        if let Ok(n) = rest.parse::<u8>() {
            return match n {
                1 => Some(b"\x1bOP".to_vec()),
                2 => Some(b"\x1bOQ".to_vec()),
                3 => Some(b"\x1bOR".to_vec()),
                4 => Some(b"\x1bOS".to_vec()),
                5 => Some(b"\x1b[15~".to_vec()),
                6 => Some(b"\x1b[17~".to_vec()),
                7 => Some(b"\x1b[18~".to_vec()),
                8 => Some(b"\x1b[19~".to_vec()),
                9 => Some(b"\x1b[20~".to_vec()),
                10 => Some(b"\x1b[21~".to_vec()),
                11 => Some(b"\x1b[23~".to_vec()),
                12 => Some(b"\x1b[24~".to_vec()),
                _ => None,
            };
        }
    }

    match name {
        "Enter" | "Return" => Some(vec![b'\r']),
        "Tab" => Some(vec![b'\t']),
        "Escape" | "Esc" => Some(vec![0x1b]),
        "Space" => Some(vec![b' ']),
        "Backspace" => Some(vec![0x7f]),
        "Delete" => Some(b"\x1b[3~".to_vec()),
        "Up" => Some(b"\x1b[A".to_vec()),
        "Down" => Some(b"\x1b[B".to_vec()),
        "Right" => Some(b"\x1b[C".to_vec()),
        "Left" => Some(b"\x1b[D".to_vec()),
        "Home" => Some(b"\x1b[H".to_vec()),
        "End" => Some(b"\x1b[F".to_vec()),
        "PageUp" => Some(b"\x1b[5~".to_vec()),
        "PageDown" => Some(b"\x1b[6~".to_vec()),
        "Insert" => Some(b"\x1b[2~".to_vec()),
        _ if name.len() == 1 => {
            let ch = name.chars().next().unwrap();
            if ch.is_ascii_graphic() || ch == ' ' {
                Some(ch.to_string().into_bytes())
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Convert a mouse click to X10 escape sequence bytes.
pub fn click_to_bytes(row: u16, col: u16) -> Vec<u8> {
    let button = 0u8;
    let cx = (col + 1).min(255) as u8 + 32;
    let cy = (row + 1).min(255) as u8 + 32;
    let mut seq = vec![0x1b, b'[', b'M', button + 32, cx, cy];
    let release_cx = cx;
    let release_cy = cy;
    seq.extend_from_slice(&[0x1b, b'[', b'M', 3 + 32, release_cx, release_cy]);
    seq
}

/// Convert a scroll event to escape sequence bytes.
pub fn scroll_to_bytes(direction: ScrollDirection, lines: u16) -> Vec<u8> {
    let button = match direction {
        ScrollDirection::Up => 64u8,
        ScrollDirection::Down => 65u8,
    };
    let cx = 1u8 + 32;
    let cy = 1u8 + 32;
    let single: &[u8] = &[0x1b, b'[', b'M', button + 32, cx, cy];
    single.repeat(lines as usize)
}

/// Parse a key sequence string like "Escape : w q Enter" into individual key names.
pub fn parse_key_sequence(seq: &str) -> Vec<String> {
    seq.split_whitespace().map(|s| s.to_string()).collect()
}
