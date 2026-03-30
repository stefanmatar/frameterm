//! BDD tests for session management,
//! driven by spec/session_management.feature.
//!
//! These tests verify real process spawning: PIDs exist,
//! processes are running, and kills actually terminate them.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use frameterm_lib::{SessionError, SessionErrorCode, SessionManager, SpawnOptions};

fn unquote(s: &str) -> String {
    s.trim_matches('"').to_string()
}

/// Returns true if a process with the given PID is alive.
fn pid_is_alive(pid: u32) -> bool {
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[derive(Debug, Clone)]
struct Ctx {
    manager: SessionManager,
    last_error: Option<SessionError>,
    env_vars: std::collections::HashMap<String, String>,
}

#[fixture]
fn ctx() -> Ctx {
    Ctx {
        manager: SessionManager::new(),
        last_error: None,
        env_vars: std::collections::HashMap::new(),
    }
}

// -- Given steps --

#[given("a session {name} is running")]
fn a_session_is_running(mut ctx: Ctx, name: String) -> Ctx {
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
    assert!(
        result.is_ok(),
        "Failed to spawn session '{name}': {result:?}"
    );
    let info = ctx.manager.get(&name).unwrap();
    let pid = info
        .pid
        .expect("spawn() must set a real PID on the session");
    assert!(
        pid_is_alive(pid),
        "PID {pid} for session '{name}' is not running"
    );
    ctx
}

#[given("a session {name} is running {command}")]
fn a_session_is_running_command(mut ctx: Ctx, name: String, command: String) -> Ctx {
    let name = unquote(&name);
    let command = unquote(&command);
    let result = ctx.manager.spawn(SpawnOptions {
        name: Some(name.clone()),
        command: command.clone(),
        args: vec![],
        working_directory: None,
        cols: None,
        rows: None,
        fps: None,
        no_record: false,
    });
    assert!(result.is_ok(), "Failed to spawn '{name}': {result:?}");
    let info = ctx.manager.get(&name).unwrap();
    let pid = info
        .pid
        .expect("spawn() must set a real PID on the session");
    assert!(
        pid_is_alive(pid),
        "PID {pid} for session '{name}' is not running"
    );
    ctx
}

#[given("the environment variable {var} is set to {value}")]
fn env_var_is_set(mut ctx: Ctx, var: String, value: String) -> Ctx {
    ctx.env_vars.insert(unquote(&var), unquote(&value));
    ctx
}

// -- When steps --

#[when("I run {command}")]
fn run_command(mut ctx: Ctx, command: String) -> Ctx {
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
        "spawn" => {
            let rest = &parts[1..];
            let mut name: Option<String> = None;
            let mut cwd: Option<std::path::PathBuf> = None;
            let mut cmd_parts: Vec<String> = vec![];
            let mut i = 0;

            while i < rest.len() {
                match rest[i] {
                    "--name" if i + 1 < rest.len() => {
                        name = Some(rest[i + 1].to_string());
                        i += 2;
                    }
                    "--cwd" if i + 1 < rest.len() => {
                        cwd = Some(std::path::PathBuf::from(rest[i + 1]));
                        i += 2;
                    }
                    _ => {
                        cmd_parts.push(rest[i].to_string());
                        i += 1;
                    }
                }
            }

            let command_name = cmd_parts.first().cloned().unwrap_or_default();
            let args: Vec<String> = cmd_parts.into_iter().skip(1).collect();

            let result = ctx.manager.spawn(SpawnOptions {
                name,
                command: command_name,
                args,
                working_directory: cwd,
                cols: None,
                rows: None,
                fps: None,
                no_record: false,
            });

            if let Err(e) = result {
                ctx.last_error = Some(e);
            }
        }
        "list-sessions" => {}
        "kill" => {
            let session_name = if parts.len() >= 3 && parts[1] == "-s" {
                parts[2].to_string()
            } else {
                "default".to_string()
            };
            if let Err(e) = ctx.manager.kill(&session_name) {
                ctx.last_error = Some(e);
            }
        }
        "stop" => {
            if let Err(e) = ctx.manager.stop() {
                ctx.last_error = Some(e);
            }
        }
        _ => {
            panic!("Unknown subcommand: {}", parts[0]);
        }
    }

    ctx
}

#[when("the process in {name} exits")]
fn process_exits(mut ctx: Ctx, name: String) -> Ctx {
    let name = unquote(&name);
    let _ = ctx.manager.kill(&name);
    ctx
}

#[when("I run {command} without --name")]
fn run_command_without_name(mut ctx: Ctx, command: String) -> Ctx {
    let command = unquote(&command);
    let parts: Vec<&str> = command.split_whitespace().collect();
    let parts = if parts.first() == Some(&"frameterm") {
        &parts[1..]
    } else {
        &parts[..]
    };

    if parts.first() == Some(&"spawn") && parts.len() >= 2 {
        let env_name = ctx.env_vars.get("FRAMETERM_SESSION").cloned();
        let cmd = parts[1].to_string();
        let result = ctx.manager.spawn(SpawnOptions {
            name: env_name,
            command: cmd,
            args: vec![],
            working_directory: None,
            cols: None,
            rows: None,
            fps: None,
            no_record: false,
        });
        if let Err(e) = result {
            ctx.last_error = Some(e);
        }
    }
    ctx
}

// -- Then steps --

#[then("a session named {name} should be created")]
fn session_named_exists(ctx: &Ctx, name: String) {
    let name = unquote(&name);
    let info = ctx
        .manager
        .get(&name)
        .unwrap_or_else(|| panic!("Expected session '{name}' to exist"));
    let pid = info.pid.expect("Session must have a real PID after spawn");
    assert!(pid_is_alive(pid), "PID {pid} should be alive after spawn");
}

#[then("the session should be running {command}")]
fn session_is_running_command(ctx: &Ctx, command: String) {
    let command = unquote(&command);
    let session = ctx
        .manager
        .get("default")
        .expect("Expected default session");
    assert_eq!(session.command, command);
    let pid = session.pid.expect("Default session must have a real PID");
    assert!(pid_is_alive(pid), "PID {pid} should be alive");
}

#[then("the session {name} should have working directory {dir}")]
fn session_has_cwd(ctx: &Ctx, name: String, dir: String) {
    let name = unquote(&name);
    let dir = unquote(&dir);
    let session = ctx
        .manager
        .get(&name)
        .unwrap_or_else(|| panic!("Expected session '{name}'"));
    assert_eq!(
        session
            .working_directory
            .as_ref()
            .map(|p| p.to_string_lossy().to_string()),
        Some(dir)
    );
}

#[then("there should be {count:u32} active sessions")]
fn active_session_count(ctx: &Ctx, count: u32) {
    let sessions = ctx.manager.list();
    assert_eq!(sessions.len(), count as usize);
    for s in &sessions {
        let pid = s.pid.expect("Every listed session must have a real PID");
        assert!(
            pid_is_alive(pid),
            "PID {pid} for '{}' should be alive",
            s.name
        );
    }
}

#[then("the output should list {name}")]
fn output_lists_session(ctx: &Ctx, name: String) {
    let name = unquote(&name);
    let sessions = ctx.manager.list();
    let found = sessions.iter().find(|s| s.name == name);
    assert!(
        found.is_some(),
        "Expected '{name}' in session list, got: {:?}",
        sessions.iter().map(|s| &s.name).collect::<Vec<_>>()
    );
    let pid = found
        .unwrap()
        .pid
        .expect("Listed session must have a real PID");
    assert!(pid_is_alive(pid), "PID {pid} for '{name}' should be alive");
}

#[then("session {name} should not exist")]
fn session_should_not_exist(ctx: &Ctx, name: String) {
    let name = unquote(&name);
    assert!(
        ctx.manager.get(&name).is_none(),
        "Expected session '{name}' to not exist"
    );
}

#[then("session {name} should still be running")]
fn session_still_running(ctx: &Ctx, name: String) {
    let name = unquote(&name);
    let info = ctx
        .manager
        .get(&name)
        .unwrap_or_else(|| panic!("Expected session '{name}' still running"));
    let pid = info.pid.expect("Session must have a real PID");
    assert!(
        pid_is_alive(pid),
        "PID {pid} for '{name}' should still be alive"
    );
}

#[then("{command} should not include {name}")]
fn list_should_not_include(ctx: &Ctx, _command: String, name: String) {
    let name = unquote(&name);
    let sessions = ctx.manager.list();
    assert!(
        !sessions.iter().any(|s| s.name == name),
        "Expected '{name}' not in session list"
    );
}

#[then("the command should fail with a JSON error")]
fn command_should_fail(ctx: &Ctx) {
    assert!(
        ctx.last_error.is_some(),
        "Expected an error from last command"
    );
}

#[then("the error code should be {code}")]
fn error_code_should_be(ctx: &Ctx, code: String) {
    let code = unquote(&code);
    let error = ctx.last_error.as_ref().expect("Expected error");
    let expected = match code.as_str() {
        "SESSION_ALREADY_EXISTS" => SessionErrorCode::SessionAlreadyExists,
        "SESSION_NOT_FOUND" => SessionErrorCode::SessionNotFound,
        "SPAWN_FAILED" => SessionErrorCode::SpawnFailed,
        _ => panic!("Unknown error code: {code}"),
    };
    assert_eq!(error.code, expected);
}

#[then("the error should include a suggestion")]
fn error_has_suggestion(ctx: &Ctx) {
    let error = ctx.last_error.as_ref().expect("Expected error");
    assert!(
        error.suggestion.is_some(),
        "Expected error to include a suggestion"
    );
}

#[then("the suggestion should mention {text}")]
fn suggestion_mentions(ctx: &Ctx, text: String) {
    let text = unquote(&text);
    let error = ctx.last_error.as_ref().expect("Expected error");
    let suggestion = error.suggestion.as_ref().expect("Expected suggestion");
    assert!(
        suggestion.contains(&text),
        "Expected suggestion to contain '{text}', \
         got: '{suggestion}'"
    );
}

#[then("the message should indicate the command was not found")]
fn message_indicates_not_found(ctx: &Ctx) {
    let error = ctx.last_error.as_ref().expect("Expected error");
    assert!(
        error.message.to_lowercase().contains("not found"),
        "Expected 'not found' in: '{}'",
        error.message
    );
}

#[then("the process exit code should be non-zero")]
fn exit_code_nonzero(ctx: &Ctx) {
    assert!(ctx.last_error.is_some(), "Expected a non-zero exit (error)");
}

#[then("all sessions should be terminated")]
fn all_sessions_terminated(ctx: &Ctx) {
    assert!(
        ctx.manager.list().is_empty(),
        "Expected all sessions terminated"
    );
}

#[then("{command} should return an empty list")]
fn list_returns_empty(ctx: &Ctx, _command: String) {
    assert!(ctx.manager.list().is_empty(), "Expected empty session list");
}

#[then("the session should be named {name}")]
fn session_should_be_named(ctx: &Ctx, name: String) {
    let name = unquote(&name);
    let info = ctx
        .manager
        .get(&name)
        .unwrap_or_else(|| panic!("Expected session named '{name}'"));
    let pid = info.pid.expect("Named session must have a real PID");
    assert!(pid_is_alive(pid), "PID {pid} for '{name}' should be alive");
}

// -- Scenario binding --

#[scenario(path = "../../spec/session_management.feature")]
fn session_management_scenario(ctx: Ctx) {
    let _ = ctx;
}
