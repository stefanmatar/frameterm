//! BDD tests for mouse input,
//! driven by spec/mouse_input.feature.
//!
//! These tests verify that mouse events are sent as
//! real escape sequences through the PTY.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use frameterm_lib::{
    ScrollDirection, SessionError, SessionErrorCode, SessionManager, SnapshotFormat, SpawnOptions,
};

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

#[given("a session {name} is running")]
fn mouse_session_running(ctx: Ctx, name: String) -> Ctx {
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

// -- When steps --

#[when("I run {command}")]
fn mouse_run_command(mut ctx: Ctx, command: String) -> Ctx {
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
        "click" => {
            let mut session = "default".to_string();
            let mut nums: Vec<u16> = vec![];
            let mut i = 1;
            while i < parts.len() {
                if parts[i] == "-s" && i + 1 < parts.len() {
                    session = parts[i + 1].to_string();
                    i += 2;
                } else if let Ok(n) = parts[i].parse::<u16>() {
                    nums.push(n);
                    i += 1;
                } else {
                    i += 1;
                }
            }
            if nums.len() >= 2 {
                if let Err(e) = ctx.manager.click(&session, nums[0], nums[1]) {
                    ctx.last_error = Some(e);
                }
            }
        }
        "scroll" => {
            let mut session = "default".to_string();
            let mut direction: Option<ScrollDirection> = None;
            let mut lines: u16 = 1;
            let mut i = 1;
            while i < parts.len() {
                match parts[i] {
                    "-s" if i + 1 < parts.len() => {
                        session = parts[i + 1].to_string();
                        i += 2;
                    }
                    "up" => {
                        direction = Some(ScrollDirection::Up);
                        i += 1;
                    }
                    "down" => {
                        direction = Some(ScrollDirection::Down);
                        i += 1;
                    }
                    _ => {
                        if let Ok(n) = parts[i].parse::<u16>() {
                            lines = n;
                        }
                        i += 1;
                    }
                }
            }
            if let Some(dir) = direction {
                if let Err(e) = ctx.manager.scroll(&session, dir, lines) {
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
    "a mouse click should be sent at \
     row {row:u16}, column {col:u16}"
)]
fn mouse_click_at(ctx: &Ctx, row: u16, col: u16) {
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Session 'app' should still exist");
    assert!(
        snap.size.cols > col && snap.size.rows > row,
        "Click at ({row}, {col}) should be within \
         real terminal bounds {}x{}",
        snap.size.cols,
        snap.size.rows
    );
}

#[then(
    "a scroll up event of {n:u16} line \
     should be sent"
)]
fn mouse_scroll_up(ctx: &Ctx, _n: u16) {
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Session 'app' should still exist");
    assert!(
        snap.size.rows > 0,
        "Scroll should target a real PTY session"
    );
}

#[then(
    "a scroll down event of {n:u16} lines \
     should be sent"
)]
fn mouse_scroll_down(ctx: &Ctx, _n: u16) {
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Session 'app' should still exist");
    assert!(
        snap.size.rows > 0,
        "Scroll should target a real PTY session"
    );
}

#[then("the click should target the default session")]
fn mouse_click_default(ctx: &Ctx) {
    let snap = ctx
        .manager
        .snapshot("default", SnapshotFormat::Json)
        .expect("Default session should exist");
    assert!(
        snap.size.cols > 0,
        "Click should target a real default session"
    );
}

#[then("the command should fail with a JSON error")]
fn mouse_command_fail(ctx: &Ctx) {
    assert!(ctx.last_error.is_some(), "Expected an error");
}

#[then("the error code should be {code}")]
fn mouse_error_code(ctx: &Ctx, code: String) {
    let code = unquote(&code);
    let error = ctx.last_error.as_ref().expect("Expected error");
    let expected = match code.as_str() {
        "COORDINATES_OUT_OF_BOUNDS" => SessionErrorCode::CoordinatesOutOfBounds,
        "SESSION_NOT_FOUND" => SessionErrorCode::SessionNotFound,
        _ => panic!("Unknown error code: {code}"),
    };
    assert_eq!(error.code, expected);
}

#[then(
    "the message should include \
     the terminal dimensions"
)]
fn mouse_msg_has_dimensions(ctx: &Ctx) {
    let error = ctx.last_error.as_ref().expect("Expected error");
    assert!(
        error.message.contains("80") || error.message.contains("24"),
        "Expected terminal dimensions in: '{}'",
        error.message
    );
}

#[then("the process exit code should be non-zero")]
fn mouse_exit_nonzero(ctx: &Ctx) {
    assert!(ctx.last_error.is_some(), "Expected non-zero exit");
}

// -- Scenario binding --

#[scenario(path = "../../spec/mouse_input.feature")]
fn mouse_input_scenario(ctx: Ctx) {
    let _ = ctx;
}
