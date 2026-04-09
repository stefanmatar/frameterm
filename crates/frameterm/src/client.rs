use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::sync::Arc;
use std::time::Duration;

use crate::daemon::socket_path;
use crate::protocol::{Envelope, Request, Response};

/// Probe whether the daemon is alive by sending a ping and reading the pong.
/// A simple connect() is not enough — it can race with the daemon's accept loop.
fn probe_daemon(sock: &std::path::Path) -> bool {
    let stream = match UnixStream::connect(sock) {
        Ok(s) => s,
        Err(_) => return false,
    };
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok();
    stream.set_write_timeout(Some(Duration::from_secs(2))).ok();
    let mut writer = match stream.try_clone() {
        Ok(w) => w,
        Err(_) => return false,
    };
    let mut reader = BufReader::new(stream);
    let req = "{\"command\":\"ping\"}\n";
    if writer.write_all(req.as_bytes()).is_err() {
        return false;
    }
    if writer.flush().is_err() {
        return false;
    }
    match read_line_lossy(&mut reader) {
        Ok(line) => !line.is_empty(),
        Err(_) => false,
    }
}

pub fn ensure_daemon() -> Result<(), String> {
    let sock = socket_path();

    if probe_daemon(&sock) {
        return Ok(());
    }

    let exe = std::env::current_exe().map_err(|e| format!("cannot find own executable: {e}"))?;

    let child = std::process::Command::new(&exe)
        .arg("daemon")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to spawn daemon: {e}"))?;

    std::mem::forget(child);

    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        if probe_daemon(&sock) {
            return Ok(());
        }
    }

    Err("daemon did not start within 5 seconds".to_string())
}

/// Read a line of bytes from a buffered reader, terminated by `\n`.
/// Returns the bytes including the newline (if present).
fn read_line_lossy(reader: &mut BufReader<UnixStream>) -> Result<String, String> {
    let mut buf = Vec::new();
    loop {
        let available = reader.fill_buf().map_err(|e| format!("read error: {e}"))?;
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

/// Pipe mode: open one persistent daemon connection, relay JSON lines
/// between stdin/stdout and the daemon socket. No fork/exec per command.
///
/// Supports concurrent in-flight requests: stdin lines are forwarded to the
/// daemon as fast as they arrive, and daemon responses are streamed back to
/// stdout independently. Clients use the `id` field in the JSON envelope to
/// match responses to requests.
pub fn run_pipe() -> ! {
    let sock = socket_path();
    let stream = UnixStream::connect(&sock).unwrap_or_else(|e| {
        eprintln!("pipe: cannot connect to daemon: {e}");
        std::process::exit(1);
    });

    // No read timeout — daemon-side commands like wait-for can block
    // for arbitrarily long (up to their own timeout). A socket-level
    // timeout here would silently kill long-running requests.

    let daemon_writer = stream.try_clone().unwrap();
    let daemon_reader_stream = stream;

    // Flag shared between threads — set on error so we exit non-zero.
    let had_error = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Daemon→stdout relay thread: reads response lines from the daemon
    // socket and writes them to stdout as they arrive.
    let had_error2 = Arc::clone(&had_error);
    let reader_handle = std::thread::spawn(move || {
        let mut daemon_reader = BufReader::new(daemon_reader_stream);
        let stdout = std::io::stdout();
        let mut stdout_writer = stdout.lock();

        loop {
            match read_line_lossy(&mut daemon_reader) {
                Ok(resp) if resp.is_empty() => {
                    // Daemon closed connection — normal when the stdin
                    // writer half closes first (EOF). Not an error.
                    break;
                }
                Ok(resp) => {
                    if stdout_writer.write_all(resp.as_bytes()).is_err() {
                        break;
                    }
                    if !resp.ends_with('\n') {
                        let _ = stdout_writer.write_all(b"\n");
                    }
                    let _ = stdout_writer.flush();
                }
                Err(e) => {
                    eprintln!("pipe: daemon connection lost (read): {e}");
                    had_error2.store(true, std::sync::atomic::Ordering::Relaxed);
                    break;
                }
            }
        }
    });

    // Stdin→daemon relay: forward JSON lines from stdin to the daemon
    // as fast as they arrive. Multiple requests can be in flight.
    {
        let mut writer = daemon_writer;
        let stdin = std::io::stdin();
        let mut stdin_reader = BufReader::new(stdin.lock());
        let mut line = String::new();

        loop {
            line.clear();
            match stdin_reader.read_line(&mut line) {
                Ok(0) => break, // clean EOF
                Ok(_) => {}
                Err(e) => {
                    eprintln!("pipe: stdin read error: {e}");
                    had_error.store(true, std::sync::atomic::Ordering::Relaxed);
                    break;
                }
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if writer.write_all(line.as_bytes()).is_err() {
                eprintln!("pipe: daemon connection lost (write)");
                had_error.store(true, std::sync::atomic::Ordering::Relaxed);
                break;
            }
            if !line.ends_with('\n') {
                let _ = writer.write_all(b"\n");
            }
            if writer.flush().is_err() {
                eprintln!("pipe: daemon connection lost (flush)");
                had_error.store(true, std::sync::atomic::Ordering::Relaxed);
                break;
            }
        }
        // Shut down the write half of the socket so the daemon sees EOF
        // and closes its end, allowing the reader thread to finish.
        let _ = writer.shutdown(std::net::Shutdown::Write);
    }

    // Wait for the reader thread to drain remaining responses.
    let _ = reader_handle.join();

    let exit_code = if had_error.load(std::sync::atomic::Ordering::Relaxed) {
        1
    } else {
        0
    };
    std::process::exit(exit_code);
}

pub fn send_request(request: &Request) -> Result<Response, String> {
    let sock = socket_path();
    let stream =
        UnixStream::connect(&sock).map_err(|e| format!("cannot connect to daemon: {e}"))?;

    stream
        .set_read_timeout(Some(Duration::from_secs(120)))
        .map_err(|e| format!("failed to set read timeout: {e}"))?;

    let mut writer = stream
        .try_clone()
        .map_err(|e| format!("failed to clone stream: {e}"))?;
    let mut reader = BufReader::new(stream);

    // Wrap in an envelope with no ID (serial CLI mode).
    let envelope = Envelope {
        id: None,
        request: request.clone(),
    };
    let mut json = serde_json::to_string(&envelope).map_err(|e| format!("serialize error: {e}"))?;
    json.push('\n');
    writer
        .write_all(json.as_bytes())
        .map_err(|e| format!("write error: {e}"))?;
    writer.flush().map_err(|e| format!("flush error: {e}"))?;

    let line = read_line_lossy(&mut reader)?;

    if line.is_empty() {
        return Err("daemon closed connection without responding".to_string());
    }

    serde_json::from_str(&line).map_err(|e| format!("invalid response JSON: {e}"))
}
