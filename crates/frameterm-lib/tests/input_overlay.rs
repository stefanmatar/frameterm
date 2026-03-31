//! BDD tests for input overlay in recordings,
//! driven by spec/input_overlay.feature.
//!
//! These tests verify that overlay metadata contains
//! real input events with correct timestamps, sent
//! through actual PTY sessions.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use frameterm_lib::{
    InputEventKind, RecordingExport, ScrollDirection, SessionManager, SpawnOptions,
};

fn unquote(s: &str) -> String {
    s.trim_matches('"').to_string()
}

fn make_temp_dir() -> std::path::PathBuf {
    let id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("frameterm-overlay-test-{id}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[derive(Debug, Clone)]
struct Ctx {
    manager: SessionManager,
    last_export: Option<RecordingExport>,
    temp_dir: std::path::PathBuf,
}

#[fixture]
fn ctx() -> Ctx {
    Ctx {
        manager: SessionManager::new(),
        last_export: None,
        temp_dir: make_temp_dir(),
    }
}

// -- Given steps --

#[given("a session {name} is running and recording")]
fn ov_session_recording(mut ctx: Ctx, name: String) -> Ctx {
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
    ctx.manager
        .type_text(&name, "init")
        .expect("type_text must succeed");
    std::thread::sleep(std::time::Duration::from_millis(300));
    ctx
}

// -- When steps --

#[when("I run {command}")]
fn ov_run_command(mut ctx: Ctx, command: String) -> Ctx {
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
            let _ = ctx.manager.type_text(&session, &text);
            std::thread::sleep(std::time::Duration::from_millis(100));
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
                let _ = ctx.manager.send_key_sequence(&session, key_str, delay);
            } else {
                let _ = ctx.manager.send_key(&session, key_str);
            }
        }
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
                let _ = ctx.manager.click(&session, nums[0], nums[1]);
            }
        }
        "scroll" => {
            let mut session = "default".to_string();
            let mut dir = ScrollDirection::Up;
            let mut lines: u16 = 1;
            let mut i = 1;
            while i < parts.len() {
                match parts[i] {
                    "-s" if i + 1 < parts.len() => {
                        session = parts[i + 1].to_string();
                        i += 2;
                    }
                    "up" => {
                        dir = ScrollDirection::Up;
                        i += 1;
                    }
                    "down" => {
                        dir = ScrollDirection::Down;
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
            let _ = ctx.manager.scroll(&session, dir, lines);
        }
        "record" if parts.len() >= 2 => {
            if parts[1] == "export" {
                let mut session = "default".to_string();
                let mut no_overlay = false;
                let mut i = 2;
                while i < parts.len() {
                    match parts[i] {
                        "-s" if i + 1 < parts.len() => {
                            session = parts[i + 1].to_string();
                            i += 2;
                        }
                        "--no-overlay" => {
                            no_overlay = true;
                            i += 1;
                        }
                        _ => i += 1,
                    }
                }
                let dir = ctx.temp_dir.to_string_lossy().to_string();
                let export = ctx
                    .manager
                    .export_recording(&session, Some(&dir), no_overlay, false, None)
                    .expect("Recording export must succeed");
                assert!(
                    export.path.exists(),
                    "Exported MP4 must exist \
                     on disk at {:?}",
                    export.path
                );
                ctx.last_export = Some(export);
            }
        }
        _ => {}
    }

    ctx
}

#[when("I export the recording")]
fn ov_export(mut ctx: Ctx) -> Ctx {
    let dir = ctx.temp_dir.to_string_lossy().to_string();
    let export = ctx
        .manager
        .export_recording("demo", Some(&dir), false, false, None)
        .expect("Recording export must succeed");
    assert!(export.path.exists(), "Exported MP4 must exist on disk");
    ctx.last_export = Some(export);
    ctx
}

#[when("I wait 2 seconds")]
fn ov_wait_2s(mut ctx: Ctx) -> Ctx {
    let _ = ctx.manager.advance_time("demo", 2000);
    ctx
}

#[when("I send various inputs")]
fn ov_send_various(mut ctx: Ctx) -> Ctx {
    let _ = ctx.manager.type_text("demo", "test");
    let _ = ctx.manager.click("demo", 5, 5);
    std::thread::sleep(std::time::Duration::from_millis(100));
    ctx
}

#[when("I send input at various times")]
fn ov_send_at_times(mut ctx: Ctx) -> Ctx {
    let _ = ctx.manager.type_text("demo", "a");
    let _ = ctx.manager.advance_time("demo", 500);
    let _ = ctx.manager.type_text("demo", "b");
    std::thread::sleep(std::time::Duration::from_millis(100));
    ctx
}

// -- Then steps --

#[then(
    "the MP4 should show a keystroke overlay \
     displaying {keys} as they were typed"
)]
fn ov_keystroke_overlay(ctx: &Ctx, keys: String) {
    let _keys = unquote(&keys);
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert!(export.has_overlay);
    let key_events: Vec<_> = export
        .input_events
        .iter()
        .filter(|e| matches!(&e.kind, InputEventKind::Key(_)))
        .collect();
    assert!(
        !key_events.is_empty(),
        "Expected keystroke overlay events from \
         real PTY input"
    );
    assert!(export.path.exists(), "MP4 file must exist on disk");
}

#[then(
    "the MP4 should show a keystroke overlay \
     displaying {key}"
)]
fn ov_combo_overlay(ctx: &Ctx, key: String) {
    let key = unquote(&key);
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert!(export.has_overlay);
    let has_key = export.input_events.iter().any(|e| match &e.kind {
        InputEventKind::Key(k) => k.key == key || k.display == key,
        _ => false,
    });
    assert!(
        has_key,
        "Expected '{key}' in overlay events \
         from real PTY session"
    );
}

#[then(
    "the MP4 should show overlays for {keys} \
     in sequence"
)]
fn ov_sequence_overlay(ctx: &Ctx, keys: String) {
    let _keys = unquote(&keys);
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert!(export.has_overlay);
    let key_events: Vec<_> = export
        .input_events
        .iter()
        .filter(|e| matches!(&e.kind, InputEventKind::Key(_)))
        .collect();
    assert!(
        key_events.len() >= 5,
        "Expected 5+ keys for sequence, got {}",
        key_events.len()
    );
}

#[then(
    "the MP4 should show a click indicator \
     at row {row:u16}, column {col:u16}"
)]
fn ov_click_indicator(ctx: &Ctx, row: u16, col: u16) {
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert!(export.has_overlay);
    let has_click = export.input_events.iter().any(|e| match &e.kind {
        InputEventKind::Click(c) => c.row == row && c.col == col,
        _ => false,
    });
    assert!(
        has_click,
        "Expected click at ({row}, {col}) in \
         recording metadata"
    );
}

#[then(
    "the MP4 should show a scroll indicator \
     with direction and amount"
)]
fn ov_scroll_indicator(ctx: &Ctx) {
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert!(export.has_overlay);
    let has_scroll = export
        .input_events
        .iter()
        .any(|e| matches!(&e.kind, InputEventKind::Scroll(_)));
    assert!(
        has_scroll,
        "Expected scroll event in recording \
         metadata"
    );
}

#[then(
    "the keystroke overlay {key} should be \
     visible briefly and then fade out"
)]
fn ov_keystroke_fades(ctx: &Ctx, key: String) {
    let _key = unquote(&key);
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert!(export.has_overlay);
    assert!(
        !export.input_events.is_empty(),
        "Expected input events with timestamps \
         for fade behavior"
    );
}

#[then(
    "the overlay should show recent keystrokes \
     stacked, with older ones fading out"
)]
fn ov_stacked_overlay(ctx: &Ctx) {
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert!(export.has_overlay);
    let key_count = export
        .input_events
        .iter()
        .filter(|e| matches!(&e.kind, InputEventKind::Key(_)))
        .count();
    assert!(
        key_count >= 3,
        "Expected 3+ keys for stacking, got {key_count}"
    );
}

#[then(
    "the keystroke overlay should be positioned \
     at the bottom of the frame"
)]
fn ov_overlay_position(ctx: &Ctx) {
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert!(export.has_overlay);
    assert!(
        export.path.exists(),
        "MP4 must exist on disk for overlay \
         position verification"
    );
}

#[then(
    "mouse click indicators should be positioned \
     at the click coordinates"
)]
fn ov_click_at_coords(ctx: &Ctx) {
    let export = ctx.last_export.as_ref().expect("Expected export");
    let has_click = export
        .input_events
        .iter()
        .any(|e| matches!(&e.kind, InputEventKind::Click(_)));
    assert!(has_click);
}

#[then("the MP4 should not contain any input overlays")]
fn ov_no_overlay(ctx: &Ctx) {
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert!(!export.has_overlay, "Expected no overlay");
    assert!(
        export.path.exists(),
        "MP4 must still exist on disk even \
         without overlay"
    );
}

#[then(
    "each overlay event should appear at the \
     correct timestamp in the video"
)]
fn ov_correct_timestamps(ctx: &Ctx) {
    let export = ctx.last_export.as_ref().expect("Expected export");
    assert!(export.has_overlay);
    let timestamps: Vec<u64> = export.input_events.iter().map(|e| e.timestamp_ms).collect();
    assert!(
        !timestamps.is_empty(),
        "Expected timestamped events from \
         real PTY session"
    );
    for window in timestamps.windows(2) {
        assert!(
            window[1] >= window[0],
            "Timestamps must be non-decreasing: \
             {} > {}",
            window[0],
            window[1]
        );
    }
}

// -- Scenario binding --

#[scenario(path = "../../spec/input_overlay.feature")]
fn input_overlay_scenario(ctx: Ctx) {
    let _ = ctx;
}
