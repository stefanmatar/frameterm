use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sysinfo::{Pid, System};

use crate::input::{
    ClickEvent, InputEvent, InputEventKind, KeyEvent, ScrollDirection, ScrollEvent, click_to_bytes,
    key_to_bytes, parse_key, parse_key_sequence, scroll_to_bytes,
};
use crate::recording::{DEFAULT_FPS, RecordingExport, RecordingState, WaitStatus, export_mp4};
use crate::snapshot::{Snapshot, SnapshotFormat, content_hash, detect_elements, format_as_text};
use crate::terminal::{Terminal, TerminalSize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub name: String,
    pub command: String,
    pub working_directory: Option<PathBuf>,
    pub pid: Option<u32>,
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionErrorCode {
    #[serde(rename = "SESSION_ALREADY_EXISTS")]
    SessionAlreadyExists,
    #[serde(rename = "SESSION_NOT_FOUND")]
    SessionNotFound,
    #[serde(rename = "SPAWN_FAILED")]
    SpawnFailed,
    #[serde(rename = "INVALID_KEY")]
    InvalidKey,
    #[serde(rename = "COORDINATES_OUT_OF_BOUNDS")]
    CoordinatesOutOfBounds,
    #[serde(rename = "WAIT_TIMEOUT")]
    WaitTimeout,
    #[serde(rename = "AWAIT_TIMEOUT")]
    AwaitTimeout,
    #[serde(rename = "NO_FRAMES_RECORDED")]
    NoFramesRecorded,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionError {
    pub code: SessionErrorCode,
    pub message: String,
    pub suggestion: Option<String>,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SessionError {}

#[derive(Debug, Clone)]
pub struct SpawnOptions {
    pub name: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    pub working_directory: Option<PathBuf>,
    pub cols: Option<u16>,
    pub rows: Option<u16>,
    pub fps: Option<u32>,
    pub no_record: bool,
}

/// Holds PTY handles that are not Clone/Debug.
/// Wrapped in Arc<Mutex<>> so the outer SessionState can be Clone+Debug.
struct PtyHandles {
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

/// Thread-safe wrapper around PTY handles.
#[derive(Clone)]
struct SharedPty {
    inner: Arc<Mutex<PtyHandles>>,
}

impl std::fmt::Debug for SharedPty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedPty").finish()
    }
}

impl SharedPty {
    fn new(handles: PtyHandles) -> Self {
        Self {
            inner: Arc::new(Mutex::new(handles)),
        }
    }

    fn write_bytes(&self, bytes: &[u8]) -> std::io::Result<()> {
        let mut handles = self.inner.lock().unwrap();
        handles.writer.write_all(bytes)?;
        handles.writer.flush()
    }

    fn kill(&self) {
        let mut handles = self.inner.lock().unwrap();
        let _ = handles.child.kill();
        let _ = handles.child.wait();
    }
}

#[derive(Debug, Clone)]
struct SessionState {
    info: SessionInfo,
    terminal: Terminal,
    pty: SharedPty,
    recording: RecordingState,
    sent_keys: Vec<KeyEvent>,
    click_events: Vec<ClickEvent>,
    scroll_events: Vec<ScrollEvent>,
}

/// Manages the lifecycle of terminal sessions.
///
/// Uses `Arc<Mutex<Inner>>` so that the manager can be `Clone + Debug`
/// (required by rstest-bdd fixtures).
#[derive(Clone)]
pub struct SessionManager {
    inner: Arc<Mutex<SessionManagerInner>>,
}

impl std::fmt::Debug for SessionManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionManager").finish()
    }
}

struct SessionManagerInner {
    sessions: HashMap<String, SessionState>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SessionManagerInner {
                sessions: HashMap::new(),
            })),
        }
    }

    pub fn spawn(&mut self, opts: SpawnOptions) -> Result<String, SessionError> {
        let name = opts
            .name
            .unwrap_or_else(|| Self::resolve_session_name(None));

        let mut inner = self.inner.lock().unwrap();

        if inner.sessions.contains_key(&name) {
            return Err(SessionError {
                code: SessionErrorCode::SessionAlreadyExists,
                message: format!("Session '{name}' already exists"),
                suggestion: Some(format!(
                    "Use a different name or kill the existing session with: frameterm kill -s {name}"
                )),
            });
        }

        let cols = opts.cols.unwrap_or(80);
        let rows = opts.rows.unwrap_or(24);
        let fps = opts.fps.unwrap_or(DEFAULT_FPS);

        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| SessionError {
                code: SessionErrorCode::SpawnFailed,
                message: format!("Failed to open PTY: {e}"),
                suggestion: None,
            })?;

        let mut cmd = CommandBuilder::new(&opts.command);
        for arg in &opts.args {
            cmd.arg(arg);
        }
        if let Some(ref cwd) = opts.working_directory {
            cmd.cwd(cwd);
        }

        let child = pair.slave.spawn_command(cmd).map_err(|e| SessionError {
            code: SessionErrorCode::SpawnFailed,
            message: format!("Command not found: '{}' ({e})", opts.command),
            suggestion: Some(format!(
                "Check that '{}' is installed and in your PATH",
                opts.command
            )),
        })?;

        let writer = pair.master.take_writer().map_err(|e| SessionError {
            code: SessionErrorCode::SpawnFailed,
            message: format!("Failed to get PTY writer: {e}"),
            suggestion: None,
        })?;

        let pid = child.process_id();

        let pty_handles = PtyHandles { writer, child };
        let shared_pty = SharedPty::new(pty_handles);

        let terminal = Terminal::new(TerminalSize { cols, rows });

        let reader = pair.master.try_clone_reader().map_err(|e| SessionError {
            code: SessionErrorCode::SpawnFailed,
            message: format!("Failed to get PTY reader: {e}"),
            suggestion: None,
        })?;

        let parser_handle = terminal.parser_handle();
        std::thread::spawn(move || {
            pty_reader_thread(reader, parser_handle);
        });

        // Drop the slave so the child owns the only reference to it.
        drop(pair.slave);

        // Allow the background reader thread to process initial PTY output
        // (e.g. shell prompt) before returning control to the caller.
        std::thread::sleep(std::time::Duration::from_millis(100));

        let info = SessionInfo {
            name: name.clone(),
            command: opts.command,
            working_directory: opts.working_directory,
            pid,
            cols,
            rows,
        };

        let recording = if opts.no_record {
            RecordingState::disabled()
        } else {
            RecordingState::new(fps)
        };

        let state = SessionState {
            info,
            terminal,
            pty: shared_pty,
            recording,
            sent_keys: Vec::new(),
            click_events: Vec::new(),
            scroll_events: Vec::new(),
        };

        inner.sessions.insert(name.clone(), state);
        drop(inner);

        if !opts.no_record {
            let inner_clone = Arc::clone(&self.inner);
            let session_name = name.clone();
            std::thread::spawn(move || {
                recording_capture_thread(inner_clone, session_name, fps);
            });
        }

        Ok(name)
    }

    pub fn kill(&mut self, name: &str) -> Result<(), SessionError> {
        let mut inner = self.inner.lock().unwrap();
        if let Some(state) = inner.sessions.remove(name) {
            state.pty.kill();
            Ok(())
        } else {
            Err(SessionError {
                code: SessionErrorCode::SessionNotFound,
                message: format!("Session '{name}' not found"),
                suggestion: Some("Run frameterm list-sessions to see active sessions".to_string()),
            })
        }
    }

    pub fn list(&self) -> Vec<SessionInfo> {
        let inner = self.inner.lock().unwrap();
        inner.sessions.values().map(|s| s.info.clone()).collect()
    }

    pub fn stop(&mut self) -> Result<(), SessionError> {
        let mut inner = self.inner.lock().unwrap();
        for (_, state) in inner.sessions.drain() {
            state.pty.kill();
        }
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<SessionInfo> {
        let inner = self.inner.lock().unwrap();
        inner.sessions.get(name).map(|s| s.info.clone())
    }

    pub fn resize(&mut self, name: &str, cols: u16, rows: u16) -> Result<(), SessionError> {
        let mut inner = self.inner.lock().unwrap();
        let state = inner.sessions.get_mut(name).ok_or_else(|| SessionError {
            code: SessionErrorCode::SessionNotFound,
            message: format!("Session '{name}' not found"),
            suggestion: Some("Run frameterm list-sessions to see active sessions".to_string()),
        })?;

        state.terminal.resize(cols, rows);
        state.info.cols = cols;
        state.info.rows = rows;

        let cells = state.terminal.cells();
        state.recording.capture_frame(cells, cols, rows);

        Ok(())
    }

    pub fn snapshot(&self, name: &str, format: SnapshotFormat) -> Result<Snapshot, SessionError> {
        let inner = self.inner.lock().unwrap();
        let state = inner.sessions.get(name).ok_or_else(|| SessionError {
            code: SessionErrorCode::SessionNotFound,
            message: format!("Session '{name}' not found"),
            suggestion: Some("Run frameterm list-sessions to see active sessions".to_string()),
        })?;

        let text = state.terminal.text();
        let cursor = state.terminal.cursor();
        let hash = content_hash(&text);
        let elements = detect_elements(&text);

        let snap = Snapshot {
            size: state.terminal.size,
            cursor,
            text: match format {
                SnapshotFormat::Compact => None,
                _ => Some(text),
            },
            content_hash: hash,
            elements,
        };

        Ok(snap)
    }

    pub fn snapshot_await_change(
        &self,
        name: &str,
        previous_hash: &str,
        _settle_ms: Option<u64>,
        timeout_ms: Option<u64>,
    ) -> Result<Snapshot, SessionError> {
        let timeout = std::time::Duration::from_millis(timeout_ms.unwrap_or(30_000));
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_millis(50);

        loop {
            let snap = self.snapshot(name, SnapshotFormat::Json)?;
            if snap.content_hash != previous_hash {
                return Ok(snap);
            }
            if start.elapsed() >= timeout {
                return Err(SessionError {
                    code: SessionErrorCode::AwaitTimeout,
                    message: "Screen did not change within timeout".to_string(),
                    suggestion: Some(
                        "Increase --timeout or check that the application is responding"
                            .to_string(),
                    ),
                });
            }
            std::thread::sleep(poll_interval);
        }
    }

    pub fn type_text(&mut self, name: &str, text: &str) -> Result<(), SessionError> {
        let mut inner = self.inner.lock().unwrap();
        let state = inner.sessions.get_mut(name).ok_or_else(|| SessionError {
            code: SessionErrorCode::SessionNotFound,
            message: format!("Session '{name}' not found"),
            suggestion: Some("Run frameterm list-sessions to see active sessions".to_string()),
        })?;

        state
            .pty
            .write_bytes(text.as_bytes())
            .map_err(|e| SessionError {
                code: SessionErrorCode::SpawnFailed,
                message: format!("Failed to write to PTY: {e}"),
                suggestion: None,
            })?;

        for ch in text.chars() {
            let key_event = KeyEvent {
                key: ch.to_string(),
                display: ch.to_string(),
            };
            state.sent_keys.push(key_event.clone());
            state.recording.record_input(InputEvent {
                kind: InputEventKind::Key(key_event),
                timestamp_ms: state.recording.current_timestamp_ms(),
            });
        }

        std::thread::sleep(std::time::Duration::from_millis(50));

        let cells = state.terminal.cells();
        state
            .recording
            .capture_frame(cells, state.terminal.size.cols, state.terminal.size.rows);

        Ok(())
    }

    pub fn send_key(&mut self, name: &str, key_name: &str) -> Result<(), SessionError> {
        {
            let inner = self.inner.lock().unwrap();
            if !inner.sessions.contains_key(name) {
                return Err(SessionError {
                    code: SessionErrorCode::SessionNotFound,
                    message: format!("Session '{name}' not found"),
                    suggestion: Some(
                        "Run frameterm list-sessions to see active sessions".to_string(),
                    ),
                });
            }
        }

        let key_event = parse_key(key_name).ok_or_else(|| SessionError {
            code: SessionErrorCode::InvalidKey,
            message: format!("Invalid key: '{key_name}'"),
            suggestion: Some(
                "Supported: Enter, Tab, Escape, Up, Down, Left, Right, \
                 Home, End, PageUp, PageDown, F1-F12, Ctrl+<key>, \
                 Alt+<key>, Shift+<key>, or any single character"
                    .to_string(),
            ),
        })?;

        let bytes = key_to_bytes(key_name).ok_or_else(|| SessionError {
            code: SessionErrorCode::InvalidKey,
            message: format!("Cannot convert key '{key_name}' to bytes"),
            suggestion: None,
        })?;

        let mut inner = self.inner.lock().unwrap();
        let state = inner.sessions.get_mut(name).unwrap();

        state.pty.write_bytes(&bytes).map_err(|e| SessionError {
            code: SessionErrorCode::SpawnFailed,
            message: format!("Failed to write key to PTY: {e}"),
            suggestion: None,
        })?;

        state.sent_keys.push(key_event.clone());
        state.recording.record_input(InputEvent {
            kind: InputEventKind::Key(key_event),
            timestamp_ms: state.recording.current_timestamp_ms(),
        });

        let cells = state.terminal.cells();
        state
            .recording
            .capture_frame(cells, state.terminal.size.cols, state.terminal.size.rows);

        Ok(())
    }

    pub fn send_key_sequence(
        &mut self,
        name: &str,
        sequence: &str,
        delay_ms: Option<u64>,
    ) -> Result<(), SessionError> {
        let keys = parse_key_sequence(sequence);
        let delay = delay_ms.unwrap_or(0);

        for key_name in &keys {
            self.send_key(name, key_name)?;
            if delay > 0 {
                let mut inner = self.inner.lock().unwrap();
                if let Some(state) = inner.sessions.get_mut(name) {
                    state.recording.advance_time(delay);
                }
            }
        }
        Ok(())
    }

    pub fn click(&mut self, name: &str, row: u16, col: u16) -> Result<(), SessionError> {
        let mut inner = self.inner.lock().unwrap();
        let state = inner.sessions.get_mut(name).ok_or_else(|| SessionError {
            code: SessionErrorCode::SessionNotFound,
            message: format!("Session '{name}' not found"),
            suggestion: Some("Run frameterm list-sessions to see active sessions".to_string()),
        })?;

        if row >= state.terminal.size.rows || col >= state.terminal.size.cols {
            return Err(SessionError {
                code: SessionErrorCode::CoordinatesOutOfBounds,
                message: format!(
                    "Coordinates ({row}, {col}) are out of bounds for terminal size {}x{}",
                    state.terminal.size.cols, state.terminal.size.rows
                ),
                suggestion: Some(format!(
                    "Terminal is {}x{}, coordinates must be within bounds",
                    state.terminal.size.cols, state.terminal.size.rows
                )),
            });
        }

        let bytes = click_to_bytes(row, col);
        let _ = state.pty.write_bytes(&bytes);

        let click = ClickEvent { row, col };
        state.click_events.push(click.clone());
        state.recording.record_input(InputEvent {
            kind: InputEventKind::Click(click),
            timestamp_ms: state.recording.current_timestamp_ms(),
        });

        let cells = state.terminal.cells();
        state
            .recording
            .capture_frame(cells, state.terminal.size.cols, state.terminal.size.rows);

        Ok(())
    }

    pub fn scroll(
        &mut self,
        name: &str,
        direction: ScrollDirection,
        lines: u16,
    ) -> Result<(), SessionError> {
        let mut inner = self.inner.lock().unwrap();
        let state = inner.sessions.get_mut(name).ok_or_else(|| SessionError {
            code: SessionErrorCode::SessionNotFound,
            message: format!("Session '{name}' not found"),
            suggestion: Some("Run frameterm list-sessions to see active sessions".to_string()),
        })?;

        let bytes = scroll_to_bytes(direction, lines);
        let _ = state.pty.write_bytes(&bytes);

        let scroll = ScrollEvent { direction, lines };
        state.scroll_events.push(scroll.clone());
        state.recording.record_input(InputEvent {
            kind: InputEventKind::Scroll(scroll),
            timestamp_ms: state.recording.current_timestamp_ms(),
        });

        let cells = state.terminal.cells();
        state
            .recording
            .capture_frame(cells, state.terminal.size.cols, state.terminal.size.rows);

        Ok(())
    }

    pub fn wait_for(
        &self,
        name: &str,
        pattern: &str,
        regex: bool,
        timeout_ms: Option<u64>,
    ) -> Result<(), SessionError> {
        {
            let mut inner = self.inner.lock().unwrap();
            let state = inner.sessions.get_mut(name).ok_or_else(|| SessionError {
                code: SessionErrorCode::SessionNotFound,
                message: format!("Session '{name}' not found"),
                suggestion: Some("Run frameterm list-sessions to see active sessions".to_string()),
            })?;
            let started_ms = state.recording.current_timestamp_ms();
            state.recording.wait_status = Some(WaitStatus::Waiting {
                text: pattern.to_string(),
                started_ms,
            });
        }

        let timeout = std::time::Duration::from_millis(timeout_ms.unwrap_or(30_000));
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_millis(50);

        loop {
            let found = {
                let inner = self.inner.lock().unwrap();
                let state = inner.sessions.get(name).unwrap();
                if regex {
                    state.terminal.matches_regex(pattern)
                } else {
                    state.terminal.contains(pattern)
                }
            };

            if found {
                let mut inner = self.inner.lock().unwrap();
                if let Some(state) = inner.sessions.get_mut(name) {
                    let found_ms = state.recording.current_timestamp_ms();
                    state.recording.wait_status = Some(WaitStatus::Found {
                        text: pattern.to_string(),
                        found_ms,
                    });
                }
                return Ok(());
            }

            if start.elapsed() >= timeout {
                let mut inner = self.inner.lock().unwrap();
                if let Some(state) = inner.sessions.get_mut(name) {
                    state.recording.wait_status = None;
                }
                return Err(SessionError {
                    code: SessionErrorCode::WaitTimeout,
                    message: format!("Timed out waiting for '{pattern}' to appear on screen"),
                    suggestion: Some(
                        "Increase --timeout or check that the application will produce the expected output"
                            .to_string(),
                    ),
                });
            }

            std::thread::sleep(poll_interval);
        }
    }

    pub fn write_to_screen(&mut self, name: &str, text: &str) -> Result<(), SessionError> {
        let mut inner = self.inner.lock().unwrap();
        let state = inner.sessions.get_mut(name).ok_or_else(|| SessionError {
            code: SessionErrorCode::SessionNotFound,
            message: format!("Session '{name}' not found"),
            suggestion: None,
        })?;

        state.terminal.process(text.as_bytes());

        let cells = state.terminal.cells();
        state
            .recording
            .capture_frame(cells, state.terminal.size.cols, state.terminal.size.rows);

        Ok(())
    }

    pub fn export_recording(
        &self,
        name: &str,
        output_dir: Option<&str>,
        no_overlay: bool,
        width: Option<u32>,
    ) -> Result<RecordingExport, SessionError> {
        let inner = self.inner.lock().unwrap();
        let state = inner.sessions.get(name).ok_or_else(|| SessionError {
            code: SessionErrorCode::SessionNotFound,
            message: format!("Session '{name}' not found"),
            suggestion: Some("Run frameterm list-sessions to see active sessions".to_string()),
        })?;

        if !state.recording.enabled {
            return Err(SessionError {
                code: SessionErrorCode::NoFramesRecorded,
                message: format!("Recording is disabled for session '{name}'"),
                suggestion: Some("Remove --no-record flag when spawning the session".to_string()),
            });
        }

        if !state.recording.has_frames() {
            return Err(SessionError {
                code: SessionErrorCode::NoFramesRecorded,
                message: format!("No frames recorded for session '{name}'"),
                suggestion: Some(
                    "Interact with the session before exporting the recording".to_string(),
                ),
            });
        }

        let dir = output_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));

        let timestamp = state.recording.current_timestamp_ms();
        let filename = format!("frameterm-{name}-{timestamp}.mp4");
        let path = dir.join(&filename);

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let has_overlay = !no_overlay && state.recording.overlay_enabled;

        let overlay_events = if has_overlay {
            state.recording.input_events.clone()
        } else {
            Vec::new()
        };

        export_mp4(
            &state.recording.frames,
            state.recording.fps,
            &path,
            has_overlay,
            &overlay_events,
            width,
        )
        .map_err(|e| SessionError {
            code: SessionErrorCode::NoFramesRecorded,
            message: format!("Failed to export recording: {e}"),
            suggestion: None,
        })?;

        let first_ts = state
            .recording
            .frames
            .first()
            .map(|f| f.timestamp_ms)
            .unwrap_or(0);
        let last_ts = state
            .recording
            .frames
            .last()
            .map(|f| f.timestamp_ms)
            .unwrap_or(0);
        let duration_ms = last_ts - first_ts;

        Ok(RecordingExport {
            path,
            fps: state.recording.fps,
            duration_ms,
            frame_count: state.recording.frames.len(),
            has_overlay,
            input_events: overlay_events,
        })
    }

    pub fn sent_keys(&self, name: &str) -> Vec<KeyEvent> {
        let inner = self.inner.lock().unwrap();
        inner
            .sessions
            .get(name)
            .map(|s| s.sent_keys.clone())
            .unwrap_or_default()
    }

    pub fn click_events(&self, name: &str) -> Vec<ClickEvent> {
        let inner = self.inner.lock().unwrap();
        inner
            .sessions
            .get(name)
            .map(|s| s.click_events.clone())
            .unwrap_or_default()
    }

    pub fn scroll_events(&self, name: &str) -> Vec<ScrollEvent> {
        let inner = self.inner.lock().unwrap();
        inner
            .sessions
            .get(name)
            .map(|s| s.scroll_events.clone())
            .unwrap_or_default()
    }

    pub fn recording_state(&self, name: &str) -> Option<RecordingState> {
        let inner = self.inner.lock().unwrap();
        inner.sessions.get(name).map(|s| s.recording.clone())
    }

    pub fn advance_time(&mut self, name: &str, ms: u64) -> Result<(), SessionError> {
        let mut inner = self.inner.lock().unwrap();
        let state = inner.sessions.get_mut(name).ok_or_else(|| SessionError {
            code: SessionErrorCode::SessionNotFound,
            message: format!("Session '{name}' not found"),
            suggestion: None,
        })?;
        state.recording.advance_time(ms);
        Ok(())
    }

    pub fn simulate_activity(&mut self, name: &str) -> Result<(), SessionError> {
        self.type_text(name, "activity")?;
        let mut inner = self.inner.lock().unwrap();
        let state = inner.sessions.get_mut(name).unwrap();
        state.recording.advance_time(1000);
        let cells = state.terminal.cells();
        state
            .recording
            .capture_frame(cells, state.terminal.size.cols, state.terminal.size.rows);
        Ok(())
    }

    pub fn screen_text(&self, name: &str) -> Option<String> {
        let inner = self.inner.lock().unwrap();
        inner.sessions.get(name).map(|s| s.terminal.text())
    }

    pub fn snapshot_as_text(&self, name: &str) -> Result<String, SessionError> {
        let snap = self.snapshot(name, SnapshotFormat::Json)?;
        Ok(format_as_text(&snap))
    }

    pub fn resolve_session_name(name: Option<&str>) -> String {
        if let Some(n) = name {
            return n.to_string();
        }
        if let Ok(env_name) = std::env::var("FRAMETERM_SESSION") {
            if !env_name.is_empty() {
                return env_name;
            }
        }
        "default".to_string()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Background thread that reads from the PTY and feeds bytes into the vt100 parser.
fn pty_reader_thread(mut reader: Box<dyn Read + Send>, parser: Arc<Mutex<vt100::Parser>>) {
    let mut buf = [0u8; 4096];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                let mut p = parser.lock().unwrap();
                p.process(&buf[..n]);
            }
            Err(_) => break,
        }
    }
}

/// Background thread that periodically captures frames for recording.
///
/// Locks the session manager briefly to snapshot terminal cells, then releases
/// before sleeping to avoid blocking other operations. Also samples CPU and
/// memory usage of the child process at ~500ms intervals.
fn recording_capture_thread(
    inner: Arc<Mutex<SessionManagerInner>>,
    session_name: String,
    fps: u32,
) {
    let interval = Duration::from_millis(1000 / fps.max(1) as u64);
    let metrics_interval = Duration::from_millis(500);

    let pid = {
        let guard = inner.lock().unwrap();
        guard.sessions.get(&session_name).and_then(|s| s.info.pid)
    };

    let mut sys = System::new();
    let sysinfo_pid = pid.map(Pid::from_u32);
    let mut last_metrics_refresh = std::time::Instant::now();

    if let Some(pid) = sysinfo_pid {
        sys.refresh_processes_specifics(
            sysinfo::ProcessesToUpdate::Some(&[pid]),
            true,
            sysinfo::ProcessRefreshKind::everything(),
        );
    }

    loop {
        std::thread::sleep(interval);

        let mut metrics_refreshed = false;
        if last_metrics_refresh.elapsed() >= metrics_interval {
            if let Some(pid) = sysinfo_pid {
                sys.refresh_processes_specifics(
                    sysinfo::ProcessesToUpdate::Some(&[pid]),
                    true,
                    sysinfo::ProcessRefreshKind::everything(),
                );
            }
            last_metrics_refresh = std::time::Instant::now();
            metrics_refreshed = true;
        }

        let (cpu_percent, memory_bytes) = sysinfo_pid
            .and_then(|pid| sys.process(pid))
            .map(|proc_info| (proc_info.cpu_usage(), proc_info.memory()))
            .unwrap_or((0.0, 0));

        let mut guard = inner.lock().unwrap();
        let state = match guard.sessions.get_mut(&session_name) {
            Some(s) => s,
            None => break,
        };
        if !state.recording.enabled {
            break;
        }
        state.recording.cpu_percent = cpu_percent;
        state.recording.memory_bytes = memory_bytes;
        if metrics_refreshed {
            let ts = state.recording.current_timestamp_ms();
            state.recording.push_cpu_sample(ts, cpu_percent);
            state.recording.push_mem_sample(ts, memory_bytes);
        }
        let cells = state.terminal.cells();
        let cols = state.terminal.size.cols;
        let rows = state.terminal.size.rows;
        state.recording.capture_frame(cells, cols, rows);
    }
}
