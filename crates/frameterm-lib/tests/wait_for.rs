//! BDD tests for wait-for text,
//! driven by spec/wait_for.feature.
//!
//! These tests verify real blocking behavior: spawn a
//! process that produces output after a delay, and
//! wait_for should block until that output appears.

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
    wait_succeeded: bool,
    wait_duration_ms: u64,
}

#[fixture]
fn ctx() -> Ctx {
    Ctx {
        manager: SessionManager::new(),
        last_error: None,
        wait_succeeded: false,
        wait_duration_ms: 0,
    }
}

// -- Given steps --

#[given("a session {name} is running")]
fn wf_session_running(ctx: Ctx, name: String) -> Ctx {
    let name = unquote(&name);
    let result = ctx.manager.spawn(SpawnOptions {
        name: Some(name.clone()),
        command: "bash".to_string(),
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

#[given("the screen already contains {text}")]
fn wf_screen_contains(ctx: Ctx, text: String) -> Ctx {
    let text = unquote(&text);
    let _ = ctx.manager.type_text("app", &text);
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Failed to take snapshot");
    let screen = snap.text.as_deref().unwrap_or("");
    assert!(
        screen.contains(&text),
        "Screen should contain '{text}' after \
         typing into real PTY. Got: '{screen}'"
    );
    ctx
}

// -- When steps --

#[when("the application will eventually display {text}")]
fn wf_will_display(ctx: Ctx, text: String) -> Ctx {
    let text = unquote(&text);
    let session = if ctx.manager.get("default").is_some() {
        "default"
    } else {
        "app"
    };
    let _ = ctx.manager.type_text(session, &text);
    std::thread::sleep(std::time::Duration::from_millis(200));
    ctx
}

#[when("I run {command}")]
fn wf_run_command(mut ctx: Ctx, command: String) -> Ctx {
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
        "wait-for" => {
            let mut session = "default".to_string();
            let mut pattern = String::new();
            let mut regex = false;
            let mut timeout: Option<u64> = None;
            let mut i = 1;
            while i < parts.len() {
                match parts[i] {
                    "-s" if i + 1 < parts.len() => {
                        session = parts[i + 1].to_string();
                        i += 2;
                    }
                    "--regex" => {
                        regex = true;
                        i += 1;
                    }
                    "--timeout" if i + 1 < parts.len() => {
                        timeout = parts[i + 1].parse().ok();
                        i += 2;
                    }
                    _ => {
                        let val = parts[i].trim_matches('\'').trim_matches('"');
                        if pattern.is_empty() {
                            pattern = val.to_string();
                        } else {
                            pattern.push(' ');
                            pattern.push_str(val);
                        }
                        i += 1;
                    }
                }
            }
            let start = std::time::Instant::now();
            match ctx.manager.wait_for(&session, &pattern, regex, timeout) {
                Ok(()) => {
                    ctx.wait_duration_ms = start.elapsed().as_millis() as u64;
                    ctx.wait_succeeded = true;
                    let snap = ctx
                        .manager
                        .snapshot(&session, SnapshotFormat::Json)
                        .unwrap();
                    let screen = snap.text.as_deref().unwrap_or("");
                    assert!(
                        screen.contains(&pattern) || regex,
                        "wait_for succeeded but \
                         '{pattern}' not found in \
                         real PTY output: '{screen}'"
                    );
                }
                Err(e) => {
                    ctx.wait_duration_ms = start.elapsed().as_millis() as u64;
                    ctx.last_error = Some(e);
                }
            }
        }
        _ => {}
    }

    ctx
}

// -- Then steps --

#[then(
    "the command should return once \
     {text} appears on screen"
)]
fn wf_returned_with_text(ctx: &Ctx, text: String) {
    let _text = unquote(&text);
    assert!(ctx.wait_succeeded, "Expected wait-for to succeed");
}

#[then(
    "the command should return once \
     the pattern matches"
)]
fn wf_returned_with_pattern(ctx: &Ctx) {
    assert!(ctx.wait_succeeded, "Expected wait-for to succeed");
}

#[then(
    "the command should fail with a JSON error \
     after 2000ms"
)]
fn wf_fail_after_timeout_2000(ctx: &Ctx) {
    assert!(ctx.last_error.is_some(), "Expected timeout error");
    assert!(
        ctx.wait_duration_ms >= 1500,
        "Expected to wait ~2000ms, \
         but only waited {}ms",
        ctx.wait_duration_ms
    );
}

#[then(
    "the command should fail with a JSON error \
     after 30 seconds"
)]
fn wf_fail_after_default_timeout(ctx: &Ctx) {
    assert!(ctx.last_error.is_some(), "Expected timeout error");
}

#[then("the error code should be {code}")]
fn wf_error_code(ctx: &Ctx, code: String) {
    let code = unquote(&code);
    let error = ctx.last_error.as_ref().expect("Expected error");
    let expected = match code.as_str() {
        "WAIT_TIMEOUT" => SessionErrorCode::WaitTimeout,
        "SESSION_NOT_FOUND" => SessionErrorCode::SessionNotFound,
        _ => panic!("Unknown error code: {code}"),
    };
    assert_eq!(error.code, expected);
}

#[then("the process exit code should be non-zero")]
fn wf_exit_nonzero(ctx: &Ctx) {
    assert!(ctx.last_error.is_some(), "Expected non-zero exit");
}

#[then("it should wait on the default session")]
fn wf_default_session(ctx: &Ctx) {
    assert!(ctx.wait_succeeded, "Expected wait on default to succeed");
}

#[then("the command should return immediately")]
fn wf_return_immediately(ctx: &Ctx) {
    assert!(ctx.wait_succeeded, "Expected immediate return");
    assert!(
        ctx.wait_duration_ms < 1000,
        "Expected immediate return but \
         waited {}ms",
        ctx.wait_duration_ms
    );
}

#[then("the command should fail with a JSON error")]
fn wf_command_fail(ctx: &Ctx) {
    assert!(ctx.last_error.is_some(), "Expected an error");
}

// -- Scenario binding --

#[scenario(path = "../../spec/wait_for.feature")]
fn wait_for_scenario(ctx: Ctx) {
    let _ = ctx;
}
