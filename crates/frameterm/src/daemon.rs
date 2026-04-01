use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use frameterm_lib::{ScrollDirection, SessionError, SessionManager, SnapshotFormat, SpawnOptions};

use crate::protocol::{Request, Response};

const IDLE_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub fn socket_path() -> PathBuf {
    if let Ok(dir) = std::env::var("FRAMETERM_SOCKET_DIR") {
        return PathBuf::from(dir).join("frameterm.sock");
    }
    if let Ok(dir) = std::env::var("XDG_RUNTIME_DIR") {
        let p = PathBuf::from(dir).join("frameterm");
        let _ = std::fs::create_dir_all(&p);
        return p.join("frameterm.sock");
    }
    if let Some(home) = dirs_home() {
        let p = home.join(".frameterm");
        let _ = std::fs::create_dir_all(&p);
        return p.join("frameterm.sock");
    }
    let p = PathBuf::from("/tmp/frameterm");
    let _ = std::fs::create_dir_all(&p);
    p.join("frameterm.sock")
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var("HOME").ok().map(PathBuf::from)
}

pub fn run_daemon() {
    let sock = socket_path();
    if let Some(parent) = sock.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::remove_file(&sock);

    let listener = match UnixListener::bind(&sock) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("frameterm daemon: failed to bind {}: {e}", sock.display());
            std::process::exit(1);
        }
    };

    listener
        .set_nonblocking(true)
        .expect("failed to set listener non-blocking");

    let manager = Arc::new(Mutex::new(SessionManager::new()));
    let last_activity = Arc::new(Mutex::new(Instant::now()));

    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                *last_activity.lock().unwrap() = Instant::now();
                let mgr = Arc::clone(&manager);
                let activity = Arc::clone(&last_activity);
                std::thread::spawn(move || {
                    handle_connection(stream, &mgr, &activity);
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                let idle = last_activity.lock().unwrap().elapsed();
                let has_sessions = {
                    let mgr = manager.lock().unwrap();
                    !mgr.list().is_empty()
                };
                if idle >= IDLE_TIMEOUT && !has_sessions {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => {
                eprintln!("frameterm daemon: accept error: {e}");
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }

    let _ = std::fs::remove_file(&sock);
}

/// Read a line of bytes from a buffered reader, terminated by `\n`.
/// Uses lossy UTF-8 conversion to handle streams that may contain
/// invalid byte sequences (e.g. from TUI apps with rich unicode rendering).
fn read_line_lossy(reader: &mut BufReader<UnixStream>) -> Result<String, std::io::Error> {
    let mut buf = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            break;
        }
        if let Some(pos) = available.iter().position(|&b| b == b'\n') {
            buf.extend_from_slice(&available[..=pos]);
            reader.consume(pos + 1);
            break;
        }
        buf.extend_from_slice(available);
        let len = available.len();
        reader.consume(len);
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn handle_connection(
    stream: UnixStream,
    manager: &Arc<Mutex<SessionManager>>,
    last_activity: &Arc<Mutex<Instant>>,
) {
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let mut writer = stream;

    loop {
        let line = match read_line_lossy(&mut reader) {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.is_empty() {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        *last_activity.lock().unwrap() = Instant::now();

        let request: Request = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => {
                let resp = Response::error("INVALID_REQUEST", format!("Bad JSON: {e}"));
                let _ = send_response(&mut writer, &resp);
                continue;
            }
        };

        let response = dispatch(request, manager);
        if send_response(&mut writer, &response).is_err() {
            break;
        }
    }
}

fn send_response(writer: &mut UnixStream, resp: &Response) -> std::io::Result<()> {
    let mut json = serde_json::to_string(resp)?;
    json.push('\n');
    writer.write_all(json.as_bytes())?;
    writer.flush()
}

fn dispatch(request: Request, manager: &Arc<Mutex<SessionManager>>) -> Response {
    match request {
        Request::Spawn {
            name,
            cmd,
            args,
            cwd,
            cols,
            rows,
            fps,
            no_record,
        } => {
            let opts = SpawnOptions {
                name,
                command: cmd,
                args,
                working_directory: cwd.map(PathBuf::from),
                cols,
                rows,
                fps,
                no_record,
            };
            let mut mgr = manager.lock().unwrap();
            match mgr.spawn(opts) {
                Ok(session_name) => Response::success(serde_json::json!({
                    "session": session_name,
                    "status": "created",
                })),
                Err(e) => session_error_response(&e),
            }
        }

        Request::Snapshot {
            session,
            format,
            await_change,
            settle,
            timeout,
        } => {
            let mgr = manager.lock().unwrap();
            if let Some(prev_hash) = await_change {
                match mgr.snapshot_await_change(&session, &prev_hash, settle, timeout) {
                    Ok(snap) => Response::success(serde_json::to_value(&snap).unwrap()),
                    Err(e) => session_error_response(&e),
                }
            } else {
                let fmt = SnapshotFormat::parse(&format);
                match mgr.snapshot(&session, fmt) {
                    Ok(snap) => {
                        if fmt == SnapshotFormat::Text {
                            match mgr.snapshot_as_text(&session) {
                                Ok(text) => Response::success(serde_json::json!({
                                    "text": text,
                                    "snapshot": serde_json::to_value(&snap).unwrap(),
                                })),
                                Err(e) => session_error_response(&e),
                            }
                        } else {
                            Response::success(serde_json::to_value(&snap).unwrap())
                        }
                    }
                    Err(e) => session_error_response(&e),
                }
            }
        }

        Request::Type { session, text } => {
            let mut mgr = manager.lock().unwrap();
            match mgr.type_text(&session, &text) {
                Ok(()) => Response::success(serde_json::json!({
                    "session": session,
                    "status": "typed",
                })),
                Err(e) => session_error_response(&e),
            }
        }

        Request::Key {
            session,
            keys,
            delay,
        } => {
            let mut mgr = manager.lock().unwrap();
            let key_parts: Vec<&str> = keys.split_whitespace().collect();
            if key_parts.len() > 1 || delay.is_some() {
                match mgr.send_key_sequence(&session, &keys, delay) {
                    Ok(()) => Response::success(serde_json::json!({
                        "session": session,
                        "status": "sent",
                    })),
                    Err(e) => session_error_response(&e),
                }
            } else {
                match mgr.send_key(&session, &keys) {
                    Ok(()) => Response::success(serde_json::json!({
                        "session": session,
                        "status": "sent",
                    })),
                    Err(e) => session_error_response(&e),
                }
            }
        }

        Request::Click { session, row, col } => {
            let mut mgr = manager.lock().unwrap();
            match mgr.click(&session, row, col) {
                Ok(()) => Response::success(serde_json::json!({
                    "session": session,
                    "status": "clicked",
                })),
                Err(e) => session_error_response(&e),
            }
        }

        Request::Scroll {
            session,
            direction,
            lines,
        } => {
            let dir = match direction.to_lowercase().as_str() {
                "up" => ScrollDirection::Up,
                _ => ScrollDirection::Down,
            };
            let mut mgr = manager.lock().unwrap();
            match mgr.scroll(&session, dir, lines) {
                Ok(()) => Response::success(serde_json::json!({
                    "session": session,
                    "status": "scrolled",
                })),
                Err(e) => session_error_response(&e),
            }
        }

        Request::Resize {
            session,
            cols,
            rows,
        } => {
            let mut mgr = manager.lock().unwrap();
            match mgr.resize(&session, cols, rows) {
                Ok(()) => Response::success(serde_json::json!({
                    "session": session,
                    "status": "resized",
                })),
                Err(e) => session_error_response(&e),
            }
        }

        Request::WaitFor {
            session,
            pattern,
            regex,
            not,
            timeout,
        } => {
            let mgr = manager.lock().unwrap();
            if not {
                match mgr.wait_for_not(&session, &pattern, regex, timeout) {
                    Ok(()) => Response::success(serde_json::json!({
                        "session": session,
                        "status": "cleared",
                    })),
                    Err(e) => session_error_response(&e),
                }
            } else {
                match mgr.wait_for(&session, &pattern, regex, timeout) {
                    Ok(()) => Response::success(serde_json::json!({
                        "session": session,
                        "status": "found",
                    })),
                    Err(e) => session_error_response(&e),
                }
            }
        }

        Request::RecordExport {
            session,
            all,
            no_overlay,
            no_footer,
            output,
            width,
        } => {
            let mgr = manager.lock().unwrap();
            if all {
                let sessions: Vec<String> = mgr.list().iter().map(|s| s.name.clone()).collect();
                let mut exports = Vec::new();
                for s in &sessions {
                    match mgr.export_recording(s, output.as_deref(), no_overlay, no_footer, width) {
                        Ok(export) => exports.push(serde_json::to_value(&export).unwrap()),
                        Err(e) => return session_error_response(&e),
                    }
                }
                Response::success(serde_json::json!({ "exports": exports }))
            } else {
                let name = session.unwrap_or_else(|| "default".to_string());
                match mgr.export_recording(&name, output.as_deref(), no_overlay, no_footer, width) {
                    Ok(export) => Response::success(serde_json::to_value(&export).unwrap()),
                    Err(e) => session_error_response(&e),
                }
            }
        }

        Request::ListSessions => {
            let mgr = manager.lock().unwrap();
            let sessions = mgr.list();
            Response::success(serde_json::to_value(&sessions).unwrap())
        }

        Request::Kill { session } => {
            let mut mgr = manager.lock().unwrap();
            match mgr.kill(&session) {
                Ok(()) => Response::success(serde_json::json!({
                    "session": session,
                    "status": "killed",
                })),
                Err(e) => session_error_response(&e),
            }
        }

        Request::Stop => {
            let mut mgr = manager.lock().unwrap();
            let _ = mgr.stop();
            Response::success(serde_json::json!({ "status": "stopped" }))
        }

        Request::Ping => Response::success(serde_json::json!({ "status": "pong" })),
    }
}

fn session_error_response(e: &SessionError) -> Response {
    let code = serde_json::to_value(&e.code).unwrap();
    let code_str = code.as_str().unwrap_or("UNKNOWN");
    match &e.suggestion {
        Some(s) => Response::error_with_suggestion(code_str, &e.message, s),
        None => Response::error(code_str, &e.message),
    }
}
