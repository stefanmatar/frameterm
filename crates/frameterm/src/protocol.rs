use serde::{Deserialize, Serialize};

/// Envelope wrapping a command with an optional request ID for multiplexing.
/// When `id` is present the daemon echoes it back in the response, allowing
/// clients to match responses to requests when multiple are in flight.
#[derive(Debug, Serialize, Deserialize)]
pub struct Envelope {
    /// Opaque caller-assigned identifier. Echoed back in the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(flatten)]
    pub request: Request,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
        not: bool,
        timeout: Option<u64>,
    },
    RecordExport {
        session: Option<String>,
        all: bool,
        no_overlay: bool,
        no_footer: bool,
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
    /// Echoed from the request envelope. Present only when the request included an `id`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
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
            id: None,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            ok: false,
            id: None,
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
            id: None,
            data: None,
            error: Some(ErrorPayload {
                code: code.into(),
                message: message.into(),
                suggestion: Some(suggestion.into()),
            }),
        }
    }

    /// Attach a request ID to this response (echoed back from the envelope).
    pub fn with_id(mut self, id: Option<String>) -> Self {
        self.id = id;
        self
    }
}
