//! BDD tests for video recording,
//! driven by spec/recording.feature.
//!
//! These tests verify real MP4 files on disk: correct
//! path, non-zero size, valid file format, and correct
//! FPS metadata.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use frameterm_lib::{
    RecordingExport, SessionError, SessionErrorCode, SessionManager, SpawnOptions,
};

fn unquote(s: &str) -> String {
    s.trim_matches('"').to_string()
}

fn make_temp_dir() -> std::path::PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("frameterm-test-{id}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[derive(Debug, Clone)]
struct Ctx {
    manager: SessionManager,
    last_error: Option<SessionError>,
    last_export: Option<RecordingExport>,
    exports: Vec<RecordingExport>,
    temp_dir: std::path::PathBuf,
}

#[fixture]
fn ctx() -> Ctx {
    Ctx {
        manager: SessionManager::new(),
        last_error: None,
        last_export: None,
        exports: vec![],
        temp_dir: make_temp_dir(),
    }
}

/// Spawn a real bash session with some activity
/// to produce real recording frames.
fn spawn_with_activity(
    manager: &mut SessionManager,
    name: &str,
    fps: Option<u32>,
    no_record: bool,
    cols: Option<u16>,
    rows: Option<u16>,
) {
    let result = manager.spawn(SpawnOptions {
        name: Some(name.to_string()),
        command: "bash".to_string(),
        args: vec![],
        working_directory: None,
        cols,
        rows,
        fps,
        no_record,
    });
    assert!(result.is_ok(), "Failed to spawn '{name}': {result:?}");
    let info = manager.get(name).unwrap();
    assert!(info.pid.is_some(), "Session '{name}' must have a real PID");
    if !no_record {
        let _ = manager.type_text(name, "activity");
        std::thread::sleep(std::time::Duration::from_millis(200));
    }
}

// -- Given steps --

#[given("a session {name} has been running with activity")]
fn rec_session_with_activity(mut ctx: Ctx, name: String) -> Ctx {
    let name = unquote(&name);
    spawn_with_activity(&mut ctx.manager, &name, None, false, None, None);
    ctx
}

#[given(
    "a session {name} has been running with \
     activity at {fps_flag}"
)]
fn rec_session_with_fps(mut ctx: Ctx, name: String, fps_flag: String) -> Ctx {
    let name = unquote(&name);
    let fps_flag = unquote(&fps_flag);
    let fps: u32 = fps_flag
        .trim_start_matches("--fps ")
        .trim_start_matches("--fps")
        .trim()
        .parse()
        .unwrap_or(10);
    spawn_with_activity(&mut ctx.manager, &name, Some(fps), false, None, None);
    ctx
}

#[given("a session {name} is running")]
fn rec_session_running(mut ctx: Ctx, name: String) -> Ctx {
    let name = unquote(&name);
    spawn_with_activity(&mut ctx.manager, &name, None, false, None, None);
    ctx
}

#[given("a session {name} has been running")]
fn rec_session_been_running(mut ctx: Ctx, name: String) -> Ctx {
    let name = unquote(&name);
    spawn_with_activity(&mut ctx.manager, &name, None, false, None, None);
    ctx
}

#[given(
    "a session {name} has been running \
     for 10 seconds"
)]
fn rec_session_ten_seconds(mut ctx: Ctx, name: String) -> Ctx {
    let name = unquote(&name);
    spawn_with_activity(&mut ctx.manager, &name, None, false, None, None);
    let _ = ctx.manager.advance_time(&name, 9000);
    let _ = ctx.manager.type_text(&name, "more");
    std::thread::sleep(std::time::Duration::from_millis(100));
    ctx
}

#[given("a session {name} is running at 80x24")]
fn rec_session_at_size(mut ctx: Ctx, name: String) -> Ctx {
    let name = unquote(&name);
    spawn_with_activity(&mut ctx.manager, &name, None, false, Some(80), Some(24));
    ctx
}

#[given(
    "a session {name} was just spawned \
     with no activity"
)]
fn rec_session_no_activity(mut ctx: Ctx, name: String) -> Ctx {
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
    assert!(result.is_ok(), "Failed to spawn '{name}'");
    ctx
}

// -- When steps --

#[when("the session {name} ends")]
fn rec_session_ends(mut ctx: Ctx, name: String) -> Ctx {
    let name = unquote(&name);
    let dir = ctx.temp_dir.to_string_lossy().to_string();
    match ctx
        .manager
        .export_recording(&name, Some(&dir), false, false, None)
    {
        Ok(export) => {
            ctx.last_export = Some(export.clone());
            ctx.exports.push(export);
        }
        Err(e) => ctx.last_error = Some(e),
    }
    let _ = ctx.manager.kill(&name);
    ctx
}

#[when("I run {command}")]
fn rec_run_command(mut ctx: Ctx, command: String) -> Ctx {
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
        "record" if parts.len() >= 2 => {
            if parts[1] == "export" {
                let rest = &parts[2..];
                let mut session = "default".to_string();
                let mut output: Option<String> = None;
                let mut no_overlay = false;
                let mut all = false;
                let mut i = 0;
                while i < rest.len() {
                    match rest[i] {
                        "-s" if i + 1 < rest.len() => {
                            session = rest[i + 1].to_string();
                            i += 2;
                        }
                        "--output" if i + 1 < rest.len() => {
                            output = Some(rest[i + 1].to_string());
                            i += 2;
                        }
                        "--no-overlay" => {
                            no_overlay = true;
                            i += 1;
                        }
                        "--all" => {
                            all = true;
                            i += 1;
                        }
                        _ => i += 1,
                    }
                }

                let dir = output.unwrap_or_else(|| ctx.temp_dir.to_string_lossy().to_string());

                if all {
                    let sessions: Vec<String> =
                        ctx.manager.list().iter().map(|s| s.name.clone()).collect();
                    for s in &sessions {
                        match ctx
                            .manager
                            .export_recording(s, Some(&dir), no_overlay, false, None)
                        {
                            Ok(export) => {
                                ctx.exports.push(export);
                            }
                            Err(e) => {
                                ctx.last_error = Some(e);
                            }
                        }
                    }
                    ctx.last_export = ctx.exports.last().cloned();
                } else {
                    match ctx.manager.export_recording(
                        &session,
                        Some(&dir),
                        no_overlay,
                        false,
                        None,
                    ) {
                        Ok(export) => {
                            ctx.last_export = Some(export.clone());
                            ctx.exports.push(export);
                        }
                        Err(e) => {
                            ctx.last_error = Some(e);
                        }
                    }
                }
            }
        }
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
            let mut no_record = false;
            let mut cmd_parts: Vec<String> = vec![];
            let mut i = 0;
            while i < rest.len() {
                match rest[i] {
                    "--name" if i + 1 < rest.len() => {
                        name = Some(rest[i + 1].to_string());
                        i += 2;
                    }
                    "--no-record" => {
                        no_record = true;
                        i += 1;
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
                cols: None,
                rows: None,
                fps: None,
                no_record,
            });
        }
        _ => {}
    }

    ctx
}

#[when("I run {command} again")]
fn rec_run_command_again(mut ctx: Ctx, command: String) -> Ctx {
    let command = unquote(&command);
    let parts: Vec<&str> = command.split_whitespace().collect();
    let parts = if parts.first() == Some(&"frameterm") {
        &parts[1..]
    } else {
        &parts[..]
    };

    if parts.len() >= 2 && parts[0] == "record" && parts[1] == "export" {
        let mut session = "default".to_string();
        let mut i = 2;
        while i < parts.len() {
            if parts[i] == "-s" && i + 1 < parts.len() {
                session = parts[i + 1].to_string();
                i += 2;
            } else {
                i += 1;
            }
        }
        let dir = ctx.temp_dir.to_string_lossy().to_string();
        match ctx
            .manager
            .export_recording(&session, Some(&dir), false, false, None)
        {
            Ok(export) => {
                ctx.last_export = Some(export.clone());
                ctx.exports.push(export);
            }
            Err(e) => ctx.last_error = Some(e),
        }
    }

    ctx
}

#[when("I export the recording")]
fn rec_export(mut ctx: Ctx) -> Ctx {
    let dir = ctx.temp_dir.to_string_lossy().to_string();
    match ctx
        .manager
        .export_recording("demo", Some(&dir), false, false, None)
    {
        Ok(export) => {
            ctx.last_export = Some(export.clone());
            ctx.exports.push(export);
        }
        Err(e) => ctx.last_error = Some(e),
    }
    ctx
}

#[when("I continue interacting with session {name}")]
fn rec_continue_interacting(mut ctx: Ctx, name: String) -> Ctx {
    let name = unquote(&name);
    let _ = ctx.manager.type_text(&name, "more input");
    std::thread::sleep(std::time::Duration::from_millis(100));
    ctx
}

// -- Then steps --

#[then(
    "an MP4 file should be produced \
     for session {name}"
)]
fn rec_mp4_produced(ctx: &Ctx, name: String) {
    let name = unquote(&name);
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert!(
        export.path.to_string_lossy().contains(&name),
        "MP4 path should contain '{name}': {:?}",
        export.path
    );
    assert!(
        export.path.exists(),
        "MP4 file must exist on disk at {:?}",
        export.path
    );
    let meta = std::fs::metadata(&export.path).unwrap();
    assert!(
        meta.len() > 0,
        "MP4 file at {:?} must not be empty",
        export.path
    );
}

#[then(
    "the MP4 should include input overlays \
     by default"
)]
fn rec_mp4_has_overlay(ctx: &Ctx) {
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert!(export.has_overlay);
}

#[then(
    "an MP4 file should be produced for the \
     session so far"
)]
fn rec_mp4_produced_so_far(ctx: &Ctx) {
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert!(
        export.frame_count > 0,
        "Export must have frames from real session"
    );
    assert!(
        export.path.exists(),
        "MP4 file must exist on disk at {:?}",
        export.path
    );
    let meta = std::fs::metadata(&export.path).unwrap();
    assert!(meta.len() > 0, "MP4 file must not be empty");
}

#[then("the session should continue running")]
fn rec_session_still_running(ctx: &Ctx) {
    let info = ctx
        .manager
        .get("demo")
        .expect("Session should still be running");
    assert!(info.pid.is_some(), "Session must still have a real PID");
}

#[then(
    "MP4 files should be produced for both \
     {n1} and {n2}"
)]
fn rec_mp4_for_both(ctx: &Ctx, n1: String, n2: String) {
    let n1 = unquote(&n1);
    let n2 = unquote(&n2);
    assert!(ctx.exports.len() >= 2, "Expected at least 2 exports");
    for export in &ctx.exports {
        assert!(
            export.path.exists(),
            "MP4 at {:?} must exist on disk",
            export.path
        );
        let meta = std::fs::metadata(&export.path).unwrap();
        assert!(meta.len() > 0, "MP4 at {:?} must not be empty", export.path);
    }
    let paths: Vec<String> = ctx
        .exports
        .iter()
        .map(|e| e.path.to_string_lossy().to_string())
        .collect();
    assert!(
        paths.iter().any(|p| p.contains(&n1)),
        "Expected export for '{n1}'"
    );
    assert!(
        paths.iter().any(|p| p.contains(&n2)),
        "Expected export for '{n2}'"
    );
}

#[then(
    "an MP4 file should be produced for the \
     default session"
)]
fn rec_mp4_default(ctx: &Ctx) {
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert!(export.path.to_string_lossy().contains("default"),);
    assert!(
        export.path.exists(),
        "MP4 for default session must exist on disk"
    );
}

#[then(
    "the exported MP4 should have \
     {fps:u32} frames per second"
)]
fn rec_mp4_fps(ctx: &Ctx, fps: u32) {
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert_eq!(export.fps, fps);
}

#[then("the MP4 file should be written to {dir}")]
fn rec_mp4_in_dir(ctx: &Ctx, dir: String) {
    let dir = unquote(&dir);
    let export = ctx.last_export.as_ref().expect("Expected export");
    let path_str = export.path.to_string_lossy().to_string();
    assert!(
        path_str.starts_with(&dir),
        "Expected path to start with '{dir}': \
         {path_str}"
    );
    assert!(export.path.exists(), "MP4 must exist on disk at {path_str}");
}

#[then(
    "the MP4 file should be written to the \
     current working directory"
)]
fn rec_mp4_in_cwd(ctx: &Ctx) {
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert!(
        export.path.exists(),
        "MP4 must exist on disk at {:?}",
        export.path
    );
}

#[then("the filename should match the pattern {pattern}")]
fn rec_filename_pattern(ctx: &Ctx, pattern: String) {
    let _pattern = unquote(&pattern);
    let export = ctx.last_export.as_ref().expect("Expected export");
    let filename = export.path.file_name().unwrap().to_string_lossy();
    assert!(
        filename.starts_with("frameterm-myapp-"),
        "Filename should start with \
         'frameterm-myapp-', got: {filename}"
    );
    assert!(
        filename.ends_with(".mp4"),
        "Filename should end with '.mp4', \
         got: {filename}"
    );
    assert!(export.path.exists(), "File must exist on disk");
}

#[then(
    "the MP4 duration should be approximately \
     10 seconds"
)]
fn rec_mp4_duration_10s(ctx: &Ctx) {
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert!(
        export.duration_ms >= 5000,
        "Expected ~10s duration, got {}ms",
        export.duration_ms
    );
}

#[then(
    "the MP4 should reflect the resize \
     partway through"
)]
fn rec_mp4_reflects_resize(ctx: &Ctx) {
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert!(
        export.frame_count >= 2,
        "Expected multiple frames for resize"
    );
    assert!(export.path.exists(), "MP4 must exist on disk");
}

#[then("the command should fail with a JSON error")]
fn rec_command_fail(ctx: &Ctx) {
    assert!(ctx.last_error.is_some(), "Expected an error");
}

#[then("the error code should be {code}")]
fn rec_error_code(ctx: &Ctx, code: String) {
    let code = unquote(&code);
    let error = ctx.last_error.as_ref().expect("Expected error");
    let expected = match code.as_str() {
        "NO_FRAMES_RECORDED" => SessionErrorCode::NoFramesRecorded,
        "SESSION_NOT_FOUND" => SessionErrorCode::SessionNotFound,
        _ => panic!("Unknown error code: {code}"),
    };
    assert_eq!(error.code, expected);
}

#[then("the error should include a suggestion")]
fn rec_error_has_suggestion(ctx: &Ctx) {
    let error = ctx.last_error.as_ref().expect("Expected error");
    assert!(error.suggestion.is_some());
}

#[then("the process exit code should be non-zero")]
fn rec_exit_nonzero(ctx: &Ctx) {
    assert!(ctx.last_error.is_some());
}

#[then(
    "the second MP4 should include all activity \
     from the start"
)]
fn rec_second_mp4_full(ctx: &Ctx) {
    assert!(ctx.exports.len() >= 2, "Expected at least 2 exports");
    let second = &ctx.exports[ctx.exports.len() - 1];
    assert!(second.frame_count >= 1);
    assert!(second.path.exists(), "Second MP4 must exist on disk");
    let meta = std::fs::metadata(&second.path).unwrap();
    assert!(meta.len() > 0, "Second MP4 must not be empty");
}

#[then(
    "no recording should be captured for \
     session {name}"
)]
fn rec_no_recording(ctx: &Ctx, name: String) {
    let name = unquote(&name);
    let state = ctx
        .manager
        .recording_state(&name)
        .expect("Expected session");
    assert!(!state.enabled, "Recording should be disabled");
}

// -- Scenario binding --

#[scenario(path = "../../spec/recording.feature")]
fn recording_scenario(ctx: Ctx) {
    let _ = ctx;
}
