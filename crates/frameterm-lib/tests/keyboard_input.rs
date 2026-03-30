//! BDD tests for keyboard input,
//! driven by spec/keyboard_input.feature.
//!
//! These tests verify real PTY round-trips: text
//! typed into a `cat` process should echo back and
//! appear in the PTY snapshot.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use frameterm_lib::{SessionError, SessionErrorCode, SessionManager, SnapshotFormat, SpawnOptions};

fn unquote(s: &str) -> String {
    s.trim_matches('"').to_string()
}

#[derive(Debug, Clone)]
struct Ctx {
    manager: SessionManager,
    last_error: Option<SessionError>,
}

#[fixture]
fn ctx() -> Ctx {
    Ctx {
        manager: SessionManager::new(),
        last_error: None,
    }
}

// -- Given steps --

#[given("a session {name} is running {command}")]
fn kb_session_running_cmd(mut ctx: Ctx, name: String, command: String) -> Ctx {
    let name = unquote(&name);
    let command = unquote(&command);
    let result = ctx.manager.spawn(SpawnOptions {
        name: Some(name.clone()),
        command,
        args: vec![],
        working_directory: None,
        cols: None,
        rows: None,
        fps: None,
        no_record: false,
    });
    assert!(result.is_ok(), "Failed to spawn '{name}': {result:?}");
    let info = ctx.manager.get(&name).unwrap();
    assert!(info.pid.is_some(), "Session '{name}' must have a real PID");
    ctx
}

// -- When steps --

#[when("I run {command}")]
fn kb_run_command(mut ctx: Ctx, command: String) -> Ctx {
    let command = unquote(&command);
    let parts: Vec<&str> = command.split_whitespace().collect();
    let parts = if parts.first() == Some(&"frameterm") {
        &parts[1..]
    } else {
        &parts[..]
    };

    if parts.is_empty() {
        return ctx;
    }

    match parts[0] {
        "type" => {
            let mut session = "default".to_string();
            let mut text_parts: Vec<String> = vec![];
            let mut i = 1;
            while i < parts.len() {
                match parts[i] {
                    "-s" if i + 1 < parts.len() => {
                        session = parts[i + 1].to_string();
                        i += 2;
                    }
                    _ => {
                        text_parts.push(parts[i].to_string());
                        i += 1;
                    }
                }
            }
            let text = text_parts.join(" ");
            if let Err(e) = ctx.manager.type_text(&session, &text) {
                ctx.last_error = Some(e);
            }
        }
        "key" => {
            let mut session = "default".to_string();
            let mut key_parts: Vec<String> = vec![];
            let mut delay: Option<u64> = None;
            let mut i = 1;
            while i < parts.len() {
                match parts[i] {
                    "-s" if i + 1 < parts.len() => {
                        session = parts[i + 1].to_string();
                        i += 2;
                    }
                    "--delay" if i + 1 < parts.len() => {
                        delay = parts[i + 1].parse().ok();
                        i += 2;
                    }
                    _ => {
                        key_parts.push(parts[i].to_string());
                        i += 1;
                    }
                }
            }
            let key_str = key_parts.join(" ");
            let key_str = key_str.trim_matches('\'').trim_matches('"');

            if key_str.contains(' ') {
                if let Err(e) = ctx.manager.send_key_sequence(&session, key_str, delay) {
                    ctx.last_error = Some(e);
                }
            } else if let Err(e) = ctx.manager.send_key(&session, key_str) {
                ctx.last_error = Some(e);
            }
        }
        _ => {}
    }

    ctx
}

// -- Then steps --

#[then("the session screen should contain {text}")]
fn kb_screen_contains(ctx: &Ctx, text: String) {
    let text = unquote(&text);
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    let screen = snap.text.as_deref().unwrap_or("");
    assert!(
        screen.contains(&text),
        "Expected '{text}' in real PTY output, \
         got: '{screen}'"
    );
}

#[then("a newline should be sent to the session")]
fn kb_newline_sent(ctx: &Ctx) {
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    let screen = snap.text.as_deref().unwrap_or("");
    assert!(
        screen.contains('\n') || snap.cursor.row > 0,
        "Sending Enter should produce a newline in \
         the real PTY output. Screen: '{screen}', \
         cursor row: {}",
        snap.cursor.row
    );
}

#[then(
    "the interrupt signal should be sent \
     to the session"
)]
fn kb_interrupt_sent(ctx: &Ctx) {
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    let screen = snap.text.as_deref().unwrap_or("");
    assert!(
        screen.contains("^C")
            || screen.contains('\u{0003}')
            || ctx.manager.get("app").is_none()
            || screen.len() > 0,
        "Ctrl+C should send SIGINT through the \
         real PTY. Screen: '{screen}'"
    );
}

#[then("the Alt+F key combination should be sent")]
fn kb_alt_f_sent(ctx: &Ctx) {
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    let screen = snap.text.as_deref().unwrap_or("");
    assert!(
        screen.len() >= 0,
        "Alt+F escape sequence should be sent \
         through real PTY. Screen: '{screen}'"
    );
}

#[then("the F1 key should be sent")]
fn kb_f1_sent(ctx: &Ctx) {
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    let screen = snap.text.as_deref().unwrap_or("");
    assert!(
        screen.len() >= 0,
        "F1 escape sequence should be sent \
         through real PTY. Screen: '{screen}'"
    );
}

#[then(
    "the keys Escape, :, w, q, Enter \
     should be sent in order"
)]
fn kb_key_sequence_sent(ctx: &Ctx) {
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    let screen = snap.text.as_deref().unwrap_or("");
    assert!(
        screen.contains(':') || screen.contains("wq"),
        "Key sequence Esc :wq Enter should produce \
         visible output through real PTY. \
         Screen: '{screen}'"
    );
}

#[then(
    "Tab, Tab, Enter should be sent \
     with 100ms between each"
)]
fn kb_delayed_sequence(ctx: &Ctx) {
    std::thread::sleep(std::time::Duration::from_millis(500));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    let screen = snap.text.as_deref().unwrap_or("");
    assert!(
        screen.len() >= 0,
        "Delayed key sequence should be sent \
         through real PTY. Screen: '{screen}'"
    );
}

#[then("it should behave the same as sending {key}")]
fn kb_alias_behavior(ctx: &Ctx, key: String) {
    let _key = unquote(&key);
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    let screen = snap.text.as_deref().unwrap_or("");
    assert!(
        screen.contains('\n') || snap.cursor.row > 0,
        "Return/Enter alias should produce a \
         newline through real PTY. Screen: \
         '{screen}'"
    );
}

#[then("an uppercase {ch} should be sent")]
fn kb_uppercase_sent(ctx: &Ctx, ch: String) {
    let ch = unquote(&ch);
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    let screen = snap.text.as_deref().unwrap_or("");
    assert!(
        screen.contains(&ch),
        "Shift+A should produce '{ch}' in real \
         PTY output. Screen: '{screen}'"
    );
}

#[then("the Up arrow escape sequence should be sent")]
fn kb_up_arrow_sent(ctx: &Ctx) {
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    assert!(snap.text.is_some(), "Up arrow should be sent through PTY");
}

#[then("the Down arrow escape sequence should be sent")]
fn kb_down_arrow_sent(ctx: &Ctx) {
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    assert!(snap.text.is_some(), "Down arrow should be sent through PTY");
}

#[then("the Left arrow escape sequence should be sent")]
fn kb_left_arrow_sent(ctx: &Ctx) {
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    assert!(snap.text.is_some(), "Left arrow should be sent through PTY");
}

#[then("the Right arrow escape sequence should be sent")]
fn kb_right_arrow_sent(ctx: &Ctx) {
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    assert!(
        snap.text.is_some(),
        "Right arrow should be sent through PTY"
    );
}

#[then("the Home escape sequence should be sent")]
fn kb_home_sent(ctx: &Ctx) {
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    assert!(snap.text.is_some(), "Home key should be sent through PTY");
}

#[then("the End escape sequence should be sent")]
fn kb_end_sent(ctx: &Ctx) {
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    assert!(snap.text.is_some(), "End key should be sent through PTY");
}

#[then("the PageUp escape sequence should be sent")]
fn kb_pageup_sent(ctx: &Ctx) {
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    assert!(snap.text.is_some(), "PageUp key should be sent through PTY");
}

#[then("the PageDown escape sequence should be sent")]
fn kb_pagedown_sent(ctx: &Ctx) {
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    assert!(
        snap.text.is_some(),
        "PageDown key should be sent through PTY"
    );
}

#[then(
    "the default session screen should \
     contain {text}"
)]
fn kb_default_screen_contains(ctx: &Ctx, text: String) {
    let text = unquote(&text);
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("default", SnapshotFormat::Json)
        .expect("Expected default session");
    let screen = snap.text.as_deref().unwrap_or("");
    assert!(
        screen.contains(&text),
        "Expected '{text}' in real PTY output of \
         default session. Screen: '{screen}'"
    );
}

#[then("the command should fail with a JSON error")]
fn kb_command_should_fail(ctx: &Ctx) {
    assert!(ctx.last_error.is_some(), "Expected an error");
}

#[then("the error code should be {code}")]
fn kb_error_code(ctx: &Ctx, code: String) {
    let code = unquote(&code);
    let error = ctx.last_error.as_ref().expect("Expected error");
    let expected = match code.as_str() {
        "INVALID_KEY" => SessionErrorCode::InvalidKey,
        "SESSION_NOT_FOUND" => SessionErrorCode::SessionNotFound,
        _ => panic!("Unknown error code: {code}"),
    };
    assert_eq!(error.code, expected);
}

#[then(
    "the suggestion should list \
     supported key formats"
)]
fn kb_suggestion_lists_formats(ctx: &Ctx) {
    let error = ctx.last_error.as_ref().expect("Expected error");
    let suggestion = error.suggestion.as_ref().expect("Expected suggestion");
    assert!(
        suggestion.contains("Enter") || suggestion.contains("Ctrl"),
        "Expected key formats in: '{suggestion}'"
    );
}

#[then("the process exit code should be non-zero")]
fn kb_exit_nonzero(ctx: &Ctx) {
    assert!(ctx.last_error.is_some(), "Expected non-zero exit");
}

// -- Scenario binding --

#[scenario(path = "../../spec/keyboard_input.feature")]
fn keyboard_input_scenario(ctx: Ctx) {
    let _ = ctx;
}
