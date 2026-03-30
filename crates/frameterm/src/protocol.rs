use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum Request {
    Spawn {
        name: Option<String>,
        cmd: String,
        args: Vec<String>,
        cwd: Option<String>,
        cols: Option<u16>,
        rows: Option<u16>,
        fps: Option<u32>,
        no_record: bool,
    },
    Snapshot {
        session: String,
        format: String,
        await_change: Option<String>,
        settle: Option<u64>,
        timeout: Option<u64>,
    },
    Type {
        session: String,
        text: String,
    },
    Key {
        session: String,
        keys: String,
        delay: Option<u64>,
    },
    Click {
        session: String,
        row: u16,
        col: u16,
    },
    Scroll {
        session: String,
        direction: String,
        lines: u16,
    },
    Resize {
        session: String,
        cols: u16,
        rows: u16,
    },
    WaitFor {
        session: String,
        pattern: String,
        regex: bool,
        timeout: Option<u64>,
    },
    RecordExport {
        session: Option<String>,
        all: bool,
        no_overlay: bool,
        output: Option<String>,
        width: Option<u32>,
    },
    ListSessions,
    Kill {
        session: String,
    },
    Stop,
    Ping,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorPayload>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl Response {
    pub fn success(data: serde_json::Value) -> Self {
        Self {
            ok: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(ErrorPayload {
                code: code.into(),
                message: message.into(),
                suggestion: None,
            }),
        }
    }

    pub fn error_with_suggestion(
        code: impl Into<String>,
        message: impl Into<String>,
        suggestion: impl Into<String>,
    ) -> Self {
        Self {
            ok: false,
            data: None,
            error: Some(ErrorPayload {
                code: code.into(),
                message: message.into(),
                suggestion: Some(suggestion.into()),
            }),
        }
    }
}
