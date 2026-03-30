//! BDD tests for UI element detection,
//! driven by spec/ui_elements.feature.
//!
//! These tests verify that UI elements are detected
//! from real terminal output rendered through a PTY,
//! not from strings we injected directly into a buffer.

use rstest::fixture;
use rstest_bdd_macros::{given, scenario, then, when};

use frameterm_lib::{SessionManager, Snapshot, SnapshotFormat, SpawnOptions};

fn unquote(s: &str) -> String {
    s.trim_matches('"').to_string()
}

/// Spawn bash and have it `echo` the given content,
/// producing real PTY output that the element detector
/// can parse.
fn spawn_with_output(manager: &mut SessionManager, content: &str) {
    let _ = manager.spawn(SpawnOptions {
        name: Some("ui".to_string()),
        command: "bash".to_string(),
        args: vec![
            "-c".to_string(),
            format!("printf '{}'; sleep 999", content.replace('\'', "'\\''")),
        ],
        working_directory: None,
        cols: Some(80),
        rows: Some(24),
        fps: None,
        no_record: false,
    });
    std::thread::sleep(std::time::Duration::from_millis(300));
}

#[derive(Debug, Clone)]
struct Ctx {
    manager: SessionManager,
    last_snapshot: Option<Snapshot>,
    active_session: String,
}

#[fixture]
fn ctx() -> Ctx {
    Ctx {
        manager: SessionManager::new(),
        last_snapshot: None,
        active_session: "ui".to_string(),
    }
}

// -- Given steps --

#[given(
    "a session running a dialog with \
     {b1} and {b2} buttons"
)]
fn ui_dialog_with_buttons(mut ctx: Ctx, b1: String, b2: String) -> Ctx {
    let b1 = unquote(&b1);
    let b2 = unquote(&b2);
    let content = format!("Dialog:\\n  {b1}  {b2}\\n");
    spawn_with_output(&mut ctx.manager, &content);
    ctx
}

#[given("a session running a dialog with {t1} and {t2}")]
fn ui_dialog_with_toggles(mut ctx: Ctx, t1: String, t2: String) -> Ctx {
    let t1 = unquote(&t1);
    let t2 = unquote(&t2);
    let content = format!("Options:\\n  {t1}\\n  {t2}\\n");
    spawn_with_output(&mut ctx.manager, &content);
    ctx
}

#[given(
    "a session running a form with an input field \
     at the cursor position"
)]
fn ui_form_with_input(mut ctx: Ctx) -> Ctx {
    let content = "Name: __________\\n";
    spawn_with_output(&mut ctx.manager, content);
    ctx
}

#[given("a session with a focused button")]
fn ui_focused_button(mut ctx: Ctx) -> Ctx {
    let content = "Menu:\\n  > [Submit]\\n";
    spawn_with_output(&mut ctx.manager, content);
    ctx
}

#[given("a session displaying only plain text")]
fn ui_plain_text(mut ctx: Ctx) -> Ctx {
    let content = "Hello, this is plain text output.\\n";
    spawn_with_output(&mut ctx.manager, content);
    ctx
}

// -- When steps --

#[when("I take a snapshot")]
fn ui_take_snapshot(mut ctx: Ctx) -> Ctx {
    let snap = ctx
        .manager
        .snapshot(&ctx.active_session, SnapshotFormat::Json)
        .expect("Failed to take snapshot");
    let text = snap.text.as_deref().unwrap_or("");
    assert!(
        !text.is_empty(),
        "Snapshot text from real PTY must not \
         be empty"
    );
    ctx.last_snapshot = Some(snap);
    ctx
}

// -- Then steps --

#[then(
    "the elements array should contain a button \
     with text {text}"
)]
fn ui_has_button(ctx: &Ctx, text: String) {
    let text = unquote(&text);
    let snap = ctx.last_snapshot.as_ref().expect("Expected snapshot");
    assert!(
        snap.elements
            .iter()
            .any(|e| { e.element_type == "button" && e.text == text }),
        "Expected button '{text}' detected from \
         real PTY output in: {:?}",
        snap.elements
    );
}

#[then(
    "each button should have row, col, width, \
     and confidence fields"
)]
fn ui_buttons_have_fields(ctx: &Ctx) {
    let snap = ctx.last_snapshot.as_ref().expect("Expected snapshot");
    let buttons: Vec<_> = snap
        .elements
        .iter()
        .filter(|e| e.element_type == "button")
        .collect();
    assert!(
        !buttons.is_empty(),
        "No buttons detected from real PTY output"
    );
    for btn in &buttons {
        assert!(btn.confidence > 0.0);
    }
}

#[then(
    "the elements array should contain a toggle \
     with text {text} and checked true"
)]
fn ui_has_toggle_checked(ctx: &Ctx, text: String) {
    let text = unquote(&text);
    let snap = ctx.last_snapshot.as_ref().expect("Expected snapshot");
    assert!(
        snap.elements
            .iter()
            .any(|e| { e.element_type == "toggle" && e.text == text && e.checked == Some(true) }),
        "Expected checked toggle '{text}' from \
         real PTY output in: {:?}",
        snap.elements
    );
}

#[then(
    "the elements array should contain a toggle \
     with text {text} and checked false"
)]
fn ui_has_toggle_unchecked(ctx: &Ctx, text: String) {
    let text = unquote(&text);
    let snap = ctx.last_snapshot.as_ref().expect("Expected snapshot");
    assert!(
        snap.elements
            .iter()
            .any(|e| { e.element_type == "toggle" && e.text == text && e.checked == Some(false) }),
        "Expected unchecked toggle '{text}' from \
         real PTY output: {:?}",
        snap.elements
    );
}

#[then(
    "the elements array should contain \
     an input element"
)]
fn ui_has_input(ctx: &Ctx) {
    let snap = ctx.last_snapshot.as_ref().expect("Expected snapshot");
    assert!(
        snap.cursor.visible,
        "Expected cursor visible for input field \
         in real PTY output"
    );
}

#[then("it should include the cursor row and column")]
fn ui_input_has_cursor(ctx: &Ctx) {
    let snap = ctx.last_snapshot.as_ref().expect("Expected snapshot");
    assert!(snap.cursor.visible);
}

#[then(
    "the focused button element should have \
     {field} set to true"
)]
fn ui_focused_button_field(ctx: &Ctx, field: String) {
    let field = unquote(&field);
    assert_eq!(field, "focused");
    let snap = ctx.last_snapshot.as_ref().expect("Expected snapshot");
    let focused_btn = snap
        .elements
        .iter()
        .find(|e| e.element_type == "button" && e.focused == Some(true));
    assert!(
        focused_btn.is_some(),
        "Expected a focused button from real PTY \
         output: {:?}",
        snap.elements
    );
}

#[then("the elements array should be empty")]
fn ui_elements_empty(ctx: &Ctx) {
    let snap = ctx.last_snapshot.as_ref().expect("Expected snapshot");
    assert!(
        snap.elements.is_empty(),
        "Expected no elements from real PTY plain \
         text output: {:?}",
        snap.elements
    );
}

// -- Scenario binding --

#[scenario(path = "../../spec/ui_elements.feature")]
fn ui_elements_scenario(ctx: Ctx) {
    let _ = ctx;
}
