//! BDD tests for terminal control,
//! driven by spec/terminal_control.feature.
//!
//! These tests verify that terminal dimensions are
//! reported from the actual PTY, not from in-memory
//! bookkeeping.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use frameterm_lib::{SessionManager, Snapshot, SnapshotFormat, SpawnOptions};

fn unquote(s: &str) -> String {
    s.trim_matches('"').to_string()
}

#[derive(Debug, Clone)]
struct Ctx {
    manager: SessionManager,
    last_snapshot: Option<Snapshot>,
}

#[fixture]
fn ctx() -> Ctx {
    Ctx {
        manager: SessionManager::new(),
        last_snapshot: None,
    }
}

// -- Given steps --

#[given("a session {name} is running")]
fn tc_session_is_running(mut ctx: Ctx, name: String) -> Ctx {
    let name = unquote(&name);
    let _ = ctx.manager.spawn(SpawnOptions {
        name: Some(name),
        command: "bash".to_string(),
        args: vec![],
        working_directory: None,
        cols: None,
        rows: None,
        fps: None,
        no_record: false,
    });
    ctx
}

// -- When steps --

#[when("I run {command}")]
fn tc_run_command(mut ctx: Ctx, command: String) -> Ctx {
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
        "resize" => {
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
                let _ = ctx.manager.resize(&session, nums[0], nums[1]);
            }
        }
        "spawn" => {
            let rest = &parts[1..];
            let mut name: Option<String> = None;
            let mut cols: Option<u16> = None;
            let mut rows: Option<u16> = None;
            let mut cmd_parts: Vec<String> = vec![];
            let mut i = 0;
            while i < rest.len() {
                match rest[i] {
                    "--name" if i + 1 < rest.len() => {
                        name = Some(rest[i + 1].to_string());
                        i += 2;
                    }
                    "--cols" if i + 1 < rest.len() => {
                        cols = rest[i + 1].parse().ok();
                        i += 2;
                    }
                    "--rows" if i + 1 < rest.len() => {
                        rows = rest[i + 1].parse().ok();
                        i += 2;
                    }
                    _ => {
                        cmd_parts.push(rest[i].to_string());
                        i += 1;
                    }
                }
            }
            let cmd = cmd_parts.first().cloned().unwrap_or_default();
            let _ = ctx.manager.spawn(SpawnOptions {
                name,
                command: cmd,
                args: vec![],
                working_directory: None,
                cols,
                rows,
                fps: None,
                no_record: false,
            });
        }
        _ => {}
    }

    ctx
}

#[when("I take a snapshot")]
fn tc_take_snapshot(mut ctx: Ctx) -> Ctx {
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Failed to take snapshot");
    ctx.last_snapshot = Some(snap);
    ctx
}

#[when("I take a snapshot of {name}")]
fn tc_take_snapshot_of(mut ctx: Ctx, name: String) -> Ctx {
    let name = unquote(&name);
    let snap = ctx
        .manager
        .snapshot(&name, SnapshotFormat::Json)
        .expect("Failed to take snapshot");
    ctx.last_snapshot = Some(snap);
    ctx
}

// -- Then steps --

#[then(
    "the session terminal should be \
     {cols:u16} columns by {rows:u16} rows"
)]
fn tc_terminal_size(ctx: &Ctx, cols: u16, rows: u16) {
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Expected session 'app'");
    assert_eq!(
        snap.size.cols, cols,
        "Snapshot cols from PTY should be {cols}, \
         got {}",
        snap.size.cols
    );
    assert_eq!(
        snap.size.rows, rows,
        "Snapshot rows from PTY should be {rows}, \
         got {}",
        snap.size.rows
    );
}

#[then(
    "the snapshot size should show \
     cols={cols:u16} and rows={rows:u16}"
)]
fn tc_snapshot_size(ctx: &Ctx, cols: u16, rows: u16) {
    let snap = ctx.last_snapshot.as_ref().expect("Expected a snapshot");
    assert_eq!(
        snap.size.cols, cols,
        "Expected cols={cols}, got {}",
        snap.size.cols
    );
    assert_eq!(
        snap.size.rows, rows,
        "Expected rows={rows}, got {}",
        snap.size.rows
    );
    let text = snap.text.as_deref().unwrap_or("");
    assert!(
        !text.is_empty() || snap.content_hash.len() > 0,
        "Snapshot should come from a real PTY \
         (text or hash must be populated)"
    );
}

// -- Scenario binding --

#[scenario(path = "../../spec/terminal_control.feature")]
fn terminal_control_scenario(ctx: Ctx) {
    let _ = ctx;
}
