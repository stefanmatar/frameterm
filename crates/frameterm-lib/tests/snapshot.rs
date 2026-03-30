//! BDD tests for screen snapshots,
//! driven by spec/snapshot.feature.
//!
//! These tests verify that snapshots contain real
//! terminal output from a PTY, not fabricated strings.

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
    last_text_output: Option<String>,
    saved_hash: Option<String>,
}

#[fixture]
fn ctx() -> Ctx {
    Ctx {
        manager: SessionManager::new(),
        last_error: None,
        last_snapshot: None,
        last_text_output: None,
        saved_hash: None,
    }
}

// -- Given steps --

#[given("a session {name} is running")]
fn snap_session_is_running(mut ctx: Ctx, name: String) -> Ctx {
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
    assert!(result.is_ok(), "Failed to spawn: {result:?}");
    let info = ctx.manager.get(&name).unwrap();
    assert!(info.pid.is_some(), "Session '{name}' must have a real PID");
    ctx
}

// -- When steps --

#[when("I run {command}")]
fn snap_run_command(mut ctx: Ctx, command: String) -> Ctx {
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
        "snapshot" => {
            let mut session = "default".to_string();
            let mut format = "json".to_string();
            let mut i = 1;
            while i < parts.len() {
                match parts[i] {
                    "-s" if i + 1 < parts.len() => {
                        session = parts[i + 1].to_string();
                        i += 2;
                    }
                    "--format" if i + 1 < parts.len() => {
                        format = parts[i + 1].to_string();
                        i += 2;
                    }
                    _ => i += 1,
                }
            }
            let fmt = SnapshotFormat::parse(&format);
            match ctx.manager.snapshot(&session, fmt) {
                Ok(snap) => {
                    if fmt == SnapshotFormat::Text {
                        let text = ctx.manager.snapshot_as_text(&session).unwrap();
                        ctx.last_text_output = Some(text);
                    }
                    ctx.last_snapshot = Some(snap);
                }
                Err(e) => {
                    ctx.last_error = Some(e);
                }
            }
        }
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
        _ => {}
    }

    ctx
}

#[when("I take a snapshot and record the content_hash")]
fn snap_take_and_record_hash(mut ctx: Ctx) -> Ctx {
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Failed to take snapshot");
    assert!(
        !snap.content_hash.is_empty(),
        "content_hash must not be empty"
    );
    ctx.saved_hash = Some(snap.content_hash.clone());
    ctx.last_snapshot = Some(snap);
    ctx
}

#[when("I take another snapshot")]
fn snap_take_another(mut ctx: Ctx) -> Ctx {
    let snap = ctx
        .manager
        .snapshot("app", SnapshotFormat::Json)
        .expect("Failed to take snapshot");
    ctx.last_snapshot = Some(snap);
    ctx
}

// -- Then steps --

#[then("the output should be valid JSON")]
fn snap_output_is_json(ctx: &Ctx) {
    let snap = ctx.last_snapshot.as_ref().expect("Expected a snapshot");
    let json = serde_json::to_string(snap).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(parsed.is_object(), "Snapshot JSON must be an object");
}

#[then(
    "it should contain a {field} field \
     with {sub1} and {sub2}"
)]
fn snap_has_field_with_subs(ctx: &Ctx, field: String, sub1: String, sub2: String) {
    let field = unquote(&field);
    let sub1 = unquote(&sub1);
    let sub2 = unquote(&sub2);
    let snap = ctx.last_snapshot.as_ref().expect("Expected a snapshot");
    let json = serde_json::to_value(snap).unwrap();

    let obj = json
        .get(&field)
        .unwrap_or_else(|| panic!("Missing field '{field}'"));
    assert!(
        obj.get(&sub1).is_some(),
        "Missing sub-field '{sub1}' in '{field}'"
    );
    assert!(
        obj.get(&sub2).is_some(),
        "Missing sub-field '{sub2}' in '{field}'"
    );
}

#[then(
    "it should contain a {field} field \
     with {s1}, {s2}, and {s3}"
)]
fn snap_has_field_with_three_subs(ctx: &Ctx, field: String, s1: String, s2: String, s3: String) {
    let field = unquote(&field);
    let s1 = unquote(&s1);
    let s2 = unquote(&s2);
    let s3 = unquote(&s3);
    let snap = ctx.last_snapshot.as_ref().expect("Expected a snapshot");
    let json = serde_json::to_value(snap).unwrap();

    let obj = json
        .get(&field)
        .unwrap_or_else(|| panic!("Missing field '{field}'"));
    assert!(obj.get(&s1).is_some());
    assert!(obj.get(&s2).is_some());
    assert!(obj.get(&s3).is_some());
}

#[then(
    "it should contain a {field} field \
     with the screen contents"
)]
fn snap_has_text_field(ctx: &Ctx, field: String) {
    let field = unquote(&field);
    let snap = ctx.last_snapshot.as_ref().expect("Expected a snapshot");
    let json = serde_json::to_value(snap).unwrap();
    let val = json
        .get(&field)
        .unwrap_or_else(|| panic!("Missing field '{field}'"));
    let text_str = val.as_str().unwrap_or("");
    assert!(
        !text_str.is_empty(),
        "The '{field}' field should contain actual \
         terminal output from a real PTY, \
         but it was empty"
    );
}

#[then("it should contain a {field} field")]
fn snap_has_field(ctx: &Ctx, field: String) {
    let field = unquote(&field);
    let snap = ctx.last_snapshot.as_ref().expect("Expected a snapshot");
    let json = serde_json::to_value(snap).unwrap();
    assert!(json.get(&field).is_some(), "Missing field '{field}'");
}

#[then("it should contain an {field} array")]
fn snap_has_array_field(ctx: &Ctx, field: String) {
    let field = unquote(&field);
    let snap = ctx.last_snapshot.as_ref().expect("Expected a snapshot");
    let json = serde_json::to_value(snap).unwrap();
    let val = json
        .get(&field)
        .unwrap_or_else(|| panic!("Missing field '{field}'"));
    assert!(val.is_array(), "'{field}' should be an array");
}

#[then("it should not contain a {field} field")]
fn snap_missing_field(ctx: &Ctx, field: String) {
    let field = unquote(&field);
    let snap = ctx.last_snapshot.as_ref().expect("Expected a snapshot");
    let json = serde_json::to_value(snap).unwrap();
    assert!(
        json.get(&field).is_none(),
        "Field '{field}' should be absent"
    );
}

#[then(
    "the output should be plain text \
     representing the screen"
)]
fn snap_plain_text_output(ctx: &Ctx) {
    let text = ctx.last_text_output.as_ref().expect("Expected text output");
    assert!(!text.is_empty(), "Text output should not be empty");
}

#[then("it should include a cursor position indicator")]
fn snap_cursor_indicator(ctx: &Ctx) {
    let text = ctx.last_text_output.as_ref().expect("Expected text output");
    assert!(
        text.contains("[cursor:"),
        "Expected cursor indicator in: {text}"
    );
}

#[then("I should receive a snapshot of the default session")]
fn snap_default_session(ctx: &Ctx) {
    let snap = ctx.last_snapshot.as_ref().expect("Expected a snapshot");
    assert!(snap.size.cols > 0);
    assert!(snap.size.rows > 0);
    assert!(
        !snap.content_hash.is_empty(),
        "Snapshot from default session must have \
         a real content_hash"
    );
}

#[then("the command should fail with a JSON error")]
fn snap_command_should_fail(ctx: &Ctx) {
    assert!(ctx.last_error.is_some(), "Expected an error");
}

#[then("the error code should be {code}")]
fn snap_error_code(ctx: &Ctx, code: String) {
    let code = unquote(&code);
    let error = ctx.last_error.as_ref().expect("Expected error");
    let expected = match code.as_str() {
        "SESSION_NOT_FOUND" => SessionErrorCode::SessionNotFound,
        _ => panic!("Unknown error code: {code}"),
    };
    assert_eq!(error.code, expected);
}

#[then("the suggestion should mention {text}")]
fn snap_suggestion_mentions(ctx: &Ctx, text: String) {
    let text = unquote(&text);
    let error = ctx.last_error.as_ref().expect("Expected error");
    let suggestion = error.suggestion.as_ref().expect("Expected suggestion");
    assert!(
        suggestion.contains(&text),
        "Expected '{text}' in: '{suggestion}'"
    );
}

#[then("the process exit code should be non-zero")]
fn snap_exit_nonzero(ctx: &Ctx) {
    assert!(ctx.last_error.is_some(), "Expected non-zero exit");
}

#[then("the content_hash should be different")]
fn snap_hash_different(ctx: &Ctx) {
    let saved = ctx.saved_hash.as_ref().expect("Expected saved hash");
    let snap = ctx.last_snapshot.as_ref().expect("Expected snapshot");
    assert_ne!(
        &snap.content_hash, saved,
        "content_hash must change after typing \
         real text into the PTY"
    );
}

#[then("the content_hash should be the same")]
fn snap_hash_same(ctx: &Ctx) {
    let saved = ctx.saved_hash.as_ref().expect("Expected saved hash");
    let snap = ctx.last_snapshot.as_ref().expect("Expected snapshot");
    assert_eq!(
        &snap.content_hash, saved,
        "content_hash must be stable when no input \
         was sent to the PTY"
    );
}

// -- Scenario binding --

#[scenario(path = "../../spec/snapshot.feature")]
fn snapshot_scenario(ctx: Ctx) {
    let _ = ctx;
}
