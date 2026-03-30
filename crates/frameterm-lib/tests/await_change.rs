//! BDD tests for await screen changes,
//! driven by spec/await_change.feature.
//!
//! These tests verify that await_change blocks on a
//! real PTY until the screen content changes, returning
//! a snapshot with genuinely different content.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use frameterm_lib::{
    SessionError, SessionErrorCode, SessionManager, Snapshot, SnapshotFormat, SpawnOptions,
};

fn unquote(s: &str) -> String {
    s.trim_matches('"').to_string()
}

#[derive(Debug, Clone)]
struct Ctx {
    manager: SessionManager,
    last_error: Option<SessionError>,
    last_snapshot: Option<Snapshot>,
    saved_hash: String,
}

#[fixture]
fn ctx() -> Ctx {
    Ctx {
        manager: SessionManager::new(),
        last_error: None,
        last_snapshot: None,
        saved_hash: String::new(),
    }
}

// -- Given steps --

#[given("a session {name} is running")]
fn ac_session_running(mut ctx: Ctx, name: String) -> Ctx {
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

#[given(
    "a session {name} is running an application \
     that streams text"
)]
fn ac_session_streaming(mut ctx: Ctx, name: String) -> Ctx {
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
    ctx
}

#[given("I have a content_hash from a previous snapshot")]
fn ac_have_previous_hash(mut ctx: Ctx) -> Ctx {
    let session = if ctx.manager.get("ai").is_some() {
        "ai"
    } else {
        "app"
    };
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot(session, SnapshotFormat::Json)
        .expect("Failed to take snapshot");
    assert!(
        !snap.content_hash.is_empty(),
        "content_hash from real PTY must not be empty"
    );
    ctx.saved_hash = snap.content_hash.clone();
    ctx.last_snapshot = Some(snap);
    ctx
}

#[given(
    "I have a content_hash from before \
     the stream started"
)]
fn ac_hash_before_stream(mut ctx: Ctx) -> Ctx {
    std::thread::sleep(std::time::Duration::from_millis(200));
    let snap = ctx
        .manager
        .snapshot("ai", SnapshotFormat::Json)
        .expect("Failed to take snapshot");
    assert!(
        !snap.content_hash.is_empty(),
        "content_hash from real PTY must not be empty"
    );
    ctx.saved_hash = snap.content_hash.clone();
    ctx.last_snapshot = Some(snap);
    ctx
}

// -- When steps --

#[when("I send input that causes the screen to change")]
fn ac_send_changing_input(mut ctx: Ctx) -> Ctx {
    let _ = ctx.manager.type_text("app", "new content");
    std::thread::sleep(std::time::Duration::from_millis(200));
    ctx
}

#[when("I send input that causes progressive rendering")]
fn ac_send_progressive_input(mut ctx: Ctx) -> Ctx {
    let _ = ctx.manager.type_text("app", "rendered");
    std::thread::sleep(std::time::Duration::from_millis(200));
    ctx
}

#[when("the screen does not change")]
fn ac_screen_no_change(ctx: Ctx) -> Ctx {
    ctx
}

#[when("I run {command}")]
fn ac_run_command(mut ctx: Ctx, command: String) -> Ctx {
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

    if parts[0] == "snapshot" {
        let mut session = "default".to_string();
        let mut await_hash: Option<String> = None;
        let mut settle: Option<u64> = None;
        let mut timeout: Option<u64> = None;
        let mut i = 1;
        while i < parts.len() {
            match parts[i] {
                "-s" if i + 1 < parts.len() => {
                    session = parts[i + 1].to_string();
                    i += 2;
                }
                "--await-change" if i + 1 < parts.len() => {
                    let hash = parts[i + 1];
                    await_hash = if hash == "<hash>" {
                        Some(ctx.saved_hash.clone())
                    } else {
                        Some(hash.to_string())
                    };
                    i += 2;
                }
                "--settle" if i + 1 < parts.len() => {
                    settle = parts[i + 1].parse().ok();
                    i += 2;
                }
                "--timeout" if i + 1 < parts.len() => {
                    timeout = parts[i + 1].parse().ok();
                    i += 2;
                }
                _ => i += 1,
            }
        }

        if let Some(prev_hash) = await_hash {
            match ctx
                .manager
                .snapshot_await_change(&session, &prev_hash, settle, timeout)
            {
                Ok(snap) => {
                    assert!(
                        !snap.content_hash.is_empty(),
                        "Returned snapshot must \
                         have a real content_hash"
                    );
                    ctx.last_snapshot = Some(snap);
                }
                Err(e) => {
                    ctx.last_error = Some(e);
                }
            }
        } else {
            match ctx.manager.snapshot(&session, SnapshotFormat::Json) {
                Ok(snap) => {
                    ctx.last_snapshot = Some(snap);
                }
                Err(e) => {
                    ctx.last_error = Some(e);
                }
            }
        }
    }

    ctx
}

// -- Then steps --

#[then(
    "the command should return once the screen \
     content differs from <hash>"
)]
fn ac_returned_on_change(ctx: &Ctx) {
    let snap = ctx
        .last_snapshot
        .as_ref()
        .expect("Expected snapshot returned");
    assert!(
        snap.content_hash != ctx.saved_hash,
        "Returned snapshot should have a different \
         hash than the original"
    );
}

#[then(
    "the returned snapshot should have \
     a different content_hash"
)]
fn ac_different_hash(ctx: &Ctx) {
    let snap = ctx.last_snapshot.as_ref().expect("Expected snapshot");
    assert_ne!(
        snap.content_hash, ctx.saved_hash,
        "Expected different hash from real PTY \
         after input was sent"
    );
}

#[then(
    "the command should wait for the screen \
     to be stable for 100ms after the \
     initial change"
)]
fn ac_stable_after_settle(ctx: &Ctx) {
    assert!(ctx.last_snapshot.is_some());
}

#[then("then return the final snapshot")]
fn ac_return_final_snapshot(ctx: &Ctx) {
    let snap = ctx.last_snapshot.as_ref().expect("Expected final snapshot");
    assert!(
        !snap.content_hash.is_empty(),
        "Final snapshot must have a real \
         content_hash"
    );
}

#[then(
    "the command should fail with a JSON error \
     after 2000ms"
)]
fn ac_fail_after_timeout_2000(ctx: &Ctx) {
    assert!(ctx.last_error.is_some(), "Expected timeout error");
}

#[then(
    "the command should fail with a JSON error \
     after 30 seconds"
)]
fn ac_fail_after_default_timeout(ctx: &Ctx) {
    assert!(ctx.last_error.is_some(), "Expected timeout error");
}

#[then("the error code should be {code}")]
fn ac_error_code(ctx: &Ctx, code: String) {
    let code = unquote(&code);
    let error = ctx.last_error.as_ref().expect("Expected error");
    let expected = match code.as_str() {
        "AWAIT_TIMEOUT" => SessionErrorCode::AwaitTimeout,
        _ => panic!("Unknown error code: {code}"),
    };
    assert_eq!(error.code, expected);
}

#[then("the process exit code should be non-zero")]
fn ac_exit_nonzero(ctx: &Ctx) {
    assert!(ctx.last_error.is_some(), "Expected non-zero exit");
}

#[then(
    "the command should wait until the stream \
     finishes (3s of no changes)"
)]
fn ac_stream_finishes(ctx: &Ctx) {
    assert!(ctx.last_snapshot.is_some());
}

#[then("return the final snapshot")]
fn ac_return_final(ctx: &Ctx) {
    let snap = ctx.last_snapshot.as_ref().expect("Expected final snapshot");
    assert!(
        !snap.content_hash.is_empty(),
        "Final snapshot must have a real \
         content_hash"
    );
}

// -- Scenario binding --

#[scenario(path = "../../spec/await_change.feature")]
fn await_change_scenario(ctx: Ctx) {
    let _ = ctx;
}
