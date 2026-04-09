use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
// Note: Mutex is still used for last_activity only. SessionManager uses interior mutability.

use frameterm_lib::{ScrollDirection, SessionError, SessionManager, SnapshotFormat, SpawnOptions};

use crate::protocol::{Envelope, Request, Response};

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

    let manager = Arc::new(SessionManager::new());
    let last_activity = Arc::new(Mutex::new(Instant::now()));

    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                // Accepted sockets may inherit non-blocking from the listener
                // on some platforms. Force blocking with a read timeout.
                stream.set_nonblocking(false).ok();
                stream.set_read_timeout(Some(Duration::from_secs(120))).ok();
                *last_activity.lock().unwrap() = Instant::now();
                let mgr = Arc::clone(&manager);
                let activity = Arc::clone(&last_activity);
                std::thread::spawn(move || {
                    handle_connection(stream, &mgr, &activity);
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                let idle = last_activity.lock().unwrap().elapsed();
                let has_sessions = !manager.list().is_empty();
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
        match reader.fill_buf() {
            Ok(available) => {
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
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if !buf.is_empty() {
                    // Partial line read, keep trying
                    std::thread::sleep(Duration::from_millis(1));
                    continue;
                }
                // No data yet, wait briefly and retry
                std::thread::sleep(Duration::from_millis(1));
                continue;
            }
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                continue;
            }
            Err(e) => return Err(e),
        }
    }
    Ok(String::from_utf8_lossy(&buf).into_owned())
}

fn handle_connection(
    stream: UnixStream,
    manager: &Arc<SessionManager>,
    last_activity: &Arc<Mutex<Instant>>,
) {
    let mut reader = BufReader::new(match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    });
    let writer = Arc::new(Mutex::new(stream));

    while let Ok(line) = read_line_lossy(&mut reader) {
        if line.is_empty() {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        *last_activity.lock().unwrap() = Instant::now();

        let envelope: Envelope = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(e) => {
                let resp = Response::error("INVALID_REQUEST", format!("Bad JSON: {e}"));
                let _ = send_response(&writer, &resp);
                continue;
            }
        };

        if envelope.id.is_some() {
            // Multiplexed: dispatch on a separate thread so this connection
            // can accept more requests while long-running commands (e.g.
            // wait-for) are still in flight.
            let mgr = Arc::clone(manager);
            let w = Arc::clone(&writer);
            std::thread::spawn(move || {
                let response = dispatch(envelope.request, &mgr).with_id(envelope.id);
                let _ = send_response(&w, &response);
            });
        } else {
            // No ID: serial dispatch (backward compatible, zero overhead).
            let response = dispatch(envelope.request, manager);
            if send_response(&writer, &response).is_err() {
                break;
            }
        }
    }
}

fn send_response(writer: &Arc<Mutex<UnixStream>>, resp: &Response) -> std::io::Result<()> {
    let mut json = serde_json::to_string(resp)?;
    json.push('\n');
    let mut w = writer.lock().unwrap();
    w.write_all(json.as_bytes())?;
    w.flush()
}

fn dispatch(request: Request, manager: &SessionManager) -> Response {
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
            match manager.spawn(opts) {
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
            if let Some(prev_hash) = await_change {
                match manager.snapshot_await_change(&session, &prev_hash, settle, timeout) {
                    Ok(snap) => Response::success(serde_json::to_value(&snap).unwrap()),
                    Err(e) => session_error_response(&e),
                }
            } else {
                let fmt = SnapshotFormat::parse(&format);
                match manager.snapshot(&session, fmt) {
                    Ok(snap) => {
                        if fmt == SnapshotFormat::Text {
                            match manager.snapshot_as_text(&session) {
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

        Request::Type { session, text } => match manager.type_text(&session, &text) {
            Ok(()) => Response::success(serde_json::json!({
                "session": session,
                "status": "typed",
            })),
            Err(e) => session_error_response(&e),
        },

        Request::Key {
            session,
            keys,
            delay,
        } => {
            let key_parts: Vec<&str> = keys.split_whitespace().collect();
            if key_parts.len() > 1 || delay.is_some() {
                match manager.send_key_sequence(&session, &keys, delay) {
                    Ok(()) => Response::success(serde_json::json!({
                        "session": session,
                        "status": "sent",
                    })),
                    Err(e) => session_error_response(&e),
                }
            } else {
                match manager.send_key(&session, &keys) {
                    Ok(()) => Response::success(serde_json::json!({
                        "session": session,
                        "status": "sent",
                    })),
                    Err(e) => session_error_response(&e),
                }
            }
        }

        Request::Click { session, row, col } => match manager.click(&session, row, col) {
            Ok(()) => Response::success(serde_json::json!({
                "session": session,
                "status": "clicked",
            })),
            Err(e) => session_error_response(&e),
        },

        Request::Scroll {
            session,
            direction,
            lines,
        } => {
            let dir = match direction.to_lowercase().as_str() {
                "up" => ScrollDirection::Up,
                _ => ScrollDirection::Down,
            };
            match manager.scroll(&session, dir, lines) {
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
        } => match manager.resize(&session, cols, rows) {
            Ok(()) => Response::success(serde_json::json!({
                "session": session,
                "status": "resized",
            })),
            Err(e) => session_error_response(&e),
        },

        Request::WaitFor {
            session,
            pattern,
            regex,
            not,
            timeout,
        } => {
            if not {
                match manager.wait_for_not(&session, &pattern, regex, timeout) {
                    Ok(()) => Response::success(serde_json::json!({
                        "session": session,
                        "status": "cleared",
                    })),
                    Err(e) => session_error_response(&e),
                }
            } else {
                match manager.wait_for(&session, &pattern, regex, timeout) {
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
            if all {
                let sessions: Vec<String> = manager.list().iter().map(|s| s.name.clone()).collect();
                let mut exports = Vec::new();
                for s in &sessions {
                    match manager.export_recording(
                        s,
                        output.as_deref(),
                        no_overlay,
                        no_footer,
                        width,
                    ) {
                        Ok(export) => exports.push(serde_json::to_value(&export).unwrap()),
                        Err(e) => return session_error_response(&e),
                    }
                }
                Response::success(serde_json::json!({ "exports": exports }))
            } else {
                let name = session.unwrap_or_else(|| "default".to_string());
                match manager.export_recording(
                    &name,
                    output.as_deref(),
                    no_overlay,
                    no_footer,
                    width,
                ) {
                    Ok(export) => Response::success(serde_json::to_value(&export).unwrap()),
                    Err(e) => session_error_response(&e),
                }
            }
        }

        Request::ListSessions => {
            let sessions = manager.list();
            Response::success(serde_json::to_value(&sessions).unwrap())
        }

        Request::Kill { session } => match manager.kill(&session) {
            Ok(()) => Response::success(serde_json::json!({
                "session": session,
                "status": "killed",
            })),
            Err(e) => session_error_response(&e),
        },

        Request::Stop => {
            let _ = manager.stop();
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
