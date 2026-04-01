use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use crate::daemon::socket_path;
use crate::protocol::{Request, Response};

pub fn ensure_daemon() -> Result<(), String> {
    let sock = socket_path();

    if UnixStream::connect(&sock).is_ok() {
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
        if UnixStream::connect(&sock).is_ok() {
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

    let mut json = serde_json::to_string(request).map_err(|e| format!("serialize error: {e}"))?;
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
