use fontdue::Font;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::LazyLock;
use std::time::Instant;

use crate::input::{InputEvent, InputEventKind, ScrollDirection};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WaitStatus {
    Waiting { text: String, started_ms: u64 },
    Found { text: String, found_ms: u64 },
}

/// Default recording FPS.
pub const DEFAULT_FPS: u32 = 10;

/// Default font size in pixels for rasterization (used when no target resolution is given).
const DEFAULT_FONT_PX: f32 = 16.0;

static FONT: LazyLock<Font> = LazyLock::new(|| {
    let bytes = include_bytes!("../fonts/JetBrainsMono-Regular.ttf");
    Font::from_bytes(bytes as &[u8], fontdue::FontSettings::default()).expect("valid TTF font")
});

/// Fallback font for emoji and symbols not covered by JetBrains Mono.
static FONT_EMOJI: LazyLock<Font> = LazyLock::new(|| {
    let bytes = include_bytes!("../fonts/NotoEmoji-Regular.ttf");
    Font::from_bytes(bytes as &[u8], fontdue::FontSettings::default()).expect("valid emoji font")
});

/// Look up the best font for a character. Returns primary font if it has
/// the glyph, otherwise tries the emoji fallback. Returns None if neither
/// font covers it.
fn font_for_char(ch: char) -> Option<&'static Font> {
    let primary = &*FONT;
    if primary.lookup_glyph_index(ch) != 0 {
        return Some(primary);
    }
    let fallback = &*FONT_EMOJI;
    if fallback.lookup_glyph_index(ch) != 0 {
        return Some(fallback);
    }
    None
}

/// Catppuccin Mocha palette: default background.
pub const DEFAULT_BG: [u8; 3] = [0x1e, 0x1e, 0x2e];
/// Catppuccin Mocha palette: default foreground.
pub const DEFAULT_FG: [u8; 3] = [0xcd, 0xd6, 0xf4];

/// Catppuccin Crust — outer canvas background, slightly darker than the terminal.
const CANVAS_BG: [u8; 3] = [0x11, 0x11, 0x1b];
/// Catppuccin Mocha standard 16-color palette.
const ANSI_PALETTE: [[u8; 3]; 16] = [
    [0x45, 0x47, 0x5a], // 0  Black
    [0xf3, 0x8b, 0xa8], // 1  Red
    [0xa6, 0xe3, 0xa1], // 2  Green
    [0xf9, 0xe2, 0xaf], // 3  Yellow
    [0x89, 0xb4, 0xfa], // 4  Blue
    [0xf5, 0xc2, 0xe7], // 5  Magenta
    [0x94, 0xe2, 0xd5], // 6  Cyan
    [0xba, 0xc2, 0xde], // 7  White
    [0x58, 0x5b, 0x70], // 8  Bright Black
    [0xf3, 0x8b, 0xa8], // 9  Bright Red
    [0xa6, 0xe3, 0xa1], // 10 Bright Green
    [0xf9, 0xe2, 0xaf], // 11 Bright Yellow
    [0x89, 0xb4, 0xfa], // 12 Bright Blue
    [0xf5, 0xc2, 0xe7], // 13 Bright Magenta
    [0x94, 0xe2, 0xd5], // 14 Bright Cyan
    [0xa6, 0xad, 0xc8], // 15 Bright White
];

/// Convert an ANSI 256-color index to RGB.
fn idx_to_rgb(idx: u8) -> [u8; 3] {
    if idx < 16 {
        return ANSI_PALETTE[idx as usize];
    }
    if idx < 232 {
        let i = idx - 16;
        let ri = i / 36;
        let gi = (i % 36) / 6;
        let bi = i % 6;
        let to_val = |v: u8| if v == 0 { 0 } else { 55 + 40 * v };
        return [to_val(ri), to_val(gi), to_val(bi)];
    }
    let gray = 8 + 10 * (idx - 232);
    [gray, gray, gray]
}

/// Convert a `vt100::Color` to an RGB triple.
pub fn vt100_color_to_rgb(color: vt100::Color, is_fg: bool) -> [u8; 3] {
    match color {
        vt100::Color::Default => {
            if is_fg {
                DEFAULT_FG
            } else {
                DEFAULT_BG
            }
        }
        vt100::Color::Idx(i) => idx_to_rgb(i),
        vt100::Color::Rgb(r, g, b) => [r, g, b],
    }
}

#[derive(Debug, Clone)]
pub struct CellData {
    pub ch: char,
    pub fg: [u8; 3],
    pub bg: [u8; 3],
    pub bold: bool,
}

#[derive(Debug, Clone)]
pub struct Frame {
    pub timestamp_ms: u64,
    pub cells: Vec<Vec<CellData>>,
    pub cols: u16,
    pub rows: u16,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub cpu_history: Vec<(u64, f32)>,
    pub mem_history: Vec<(u64, u64)>,
    pub wait_status: Option<WaitStatus>,
}

/// Maximum number of history samples to retain (10 seconds at 500ms intervals).
const HISTORY_CAP: usize = 20;

/// Duration of history window in milliseconds.
const HISTORY_WINDOW_MS: u64 = 10_000;

#[derive(Debug, Clone)]
pub struct RecordingState {
    pub enabled: bool,
    pub fps: u32,
    pub frames: Vec<Frame>,
    pub input_events: Vec<InputEvent>,
    pub overlay_enabled: bool,
    pub elapsed_ms: u64,
    real_start_time: Option<Instant>,
    test_mode: bool,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
    pub cpu_history: Vec<(u64, f32)>,
    pub mem_history: Vec<(u64, u64)>,
    pub wait_status: Option<WaitStatus>,
}

impl RecordingState {
    pub fn new(fps: u32) -> Self {
        Self {
            enabled: true,
            fps,
            frames: Vec::new(),
            input_events: Vec::new(),
            overlay_enabled: true,
            elapsed_ms: 0,
            real_start_time: None,
            test_mode: false,
            cpu_percent: 0.0,
            memory_bytes: 0,
            cpu_history: Vec::new(),
            mem_history: Vec::new(),
            wait_status: None,
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            fps: 0,
            frames: Vec::new(),
            input_events: Vec::new(),
            overlay_enabled: true,
            elapsed_ms: 0,
            real_start_time: None,
            test_mode: false,
            cpu_percent: 0.0,
            memory_bytes: 0,
            cpu_history: Vec::new(),
            mem_history: Vec::new(),
            wait_status: None,
        }
    }

    pub fn current_timestamp_ms(&self) -> u64 {
        if self.test_mode {
            return self.elapsed_ms;
        }
        match self.real_start_time {
            Some(start) => start.elapsed().as_millis() as u64,
            None => 0,
        }
    }

    fn ensure_start_time(&mut self) {
        if !self.test_mode && self.real_start_time.is_none() {
            self.real_start_time = Some(Instant::now());
        }
    }

    pub fn capture_frame(&mut self, cells: Vec<Vec<CellData>>, cols: u16, rows: u16) {
        if !self.enabled {
            return;
        }
        self.ensure_start_time();
        let timestamp_ms = self.current_timestamp_ms();
        let cutoff = timestamp_ms.saturating_sub(HISTORY_WINDOW_MS);
        let cpu_hist: Vec<(u64, f32)> = self
            .cpu_history
            .iter()
            .filter(|(ts, _)| *ts >= cutoff)
            .copied()
            .collect();
        let mem_hist: Vec<(u64, u64)> = self
            .mem_history
            .iter()
            .filter(|(ts, _)| *ts >= cutoff)
            .copied()
            .collect();
        self.frames.push(Frame {
            timestamp_ms,
            cells,
            cols,
            rows,
            cpu_percent: self.cpu_percent,
            memory_bytes: self.memory_bytes,
            cpu_history: cpu_hist,
            mem_history: mem_hist,
            wait_status: self.wait_status.clone(),
        });
    }

    pub fn push_cpu_sample(&mut self, timestamp_ms: u64, cpu_percent: f32) {
        self.cpu_history.push((timestamp_ms, cpu_percent));
        if self.cpu_history.len() > HISTORY_CAP {
            self.cpu_history.remove(0);
        }
    }

    pub fn push_mem_sample(&mut self, timestamp_ms: u64, bytes: u64) {
        self.mem_history.push((timestamp_ms, bytes));
        if self.mem_history.len() > HISTORY_CAP {
            self.mem_history.remove(0);
        }
    }

    pub fn record_input(&mut self, event: InputEvent) {
        if !self.enabled {
            return;
        }
        self.input_events.push(event);
    }

    pub fn advance_time(&mut self, ms: u64) {
        self.test_mode = true;
        self.elapsed_ms += ms;
    }

    pub fn has_frames(&self) -> bool {
        !self.frames.is_empty()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingExport {
    pub path: PathBuf,
    pub fps: u32,
    pub duration_ms: u64,
    pub frame_count: usize,
    pub has_overlay: bool,
    pub input_events: Vec<InputEvent>,
}

/// Compute the cell dimensions (width, height) from the font metrics at the given font size.
fn cell_size_at(font_px: f32) -> (u32, u32) {
    let metrics = FONT.metrics('M', font_px);
    let width = metrics.advance_width.ceil() as u32;
    let height = FONT
        .horizontal_line_metrics(font_px)
        .map(|lm| (lm.ascent - lm.descent + lm.line_gap).ceil() as u32)
        .unwrap_or(font_px.ceil() as u32);
    (width.max(1), height.max(1))
}

/// Duration in milliseconds that input events remain visible.
const OVERLAY_FADE_MS: u64 = 2000;

/// Footer background — slightly lifted from the terminal.
const FOOTER_BG: [u8; 3] = [0x18, 0x18, 0x27];

/// Token fill, border, and text colors inspired by shadcn dark kbd styling.
const TOKEN_BG: [u8; 3] = [0x22, 0x22, 0x34];
const TOKEN_BORDER: [u8; 3] = [0x3a, 0x3a, 0x52];
const TOKEN_TEXT: [u8; 3] = [0xcd, 0xd6, 0xf4];

/// Color for click indicator circle.
const CLICK_COLOR: [u8; 3] = [0xf3, 0x8b, 0xa8];

/// Very muted color for metric labels ("CPU", "MEM").
const METRIC_LABEL: [u8; 3] = [0x58, 0x5b, 0x70];

/// Slightly brighter color for metric values ("2.3%", "45 MB").
const METRIC_VALUE: [u8; 3] = [0x6c, 0x70, 0x86];

/// Catppuccin Yellow for wait spinner.
const WAIT_SPINNER_COLOR: [u8; 3] = [0xf9, 0xe2, 0xaf];

/// Catppuccin Green for wait-found checkmark.
const WAIT_FOUND_COLOR: [u8; 3] = [0xa6, 0xe3, 0xa1];

/// Braille spinner frames for wait-for animation.
const SPINNER_FRAMES: &[&str] = &[
    "\u{25CB}", // ○
    "\u{25CF}", // ●
];

/// Duration in milliseconds to show the "found" checkmark.
const WAIT_FOUND_DISPLAY_MS: u64 = 2000;

/// Catppuccin Teal for CPU sparkline.
const SPARK_CPU_LINE: [u8; 3] = [0x94, 0xe2, 0xd5];
const SPARK_CPU_FILL: [u8; 3] = [0x94, 0xe2, 0xd5];

/// Catppuccin Mauve for MEM sparkline.
const SPARK_MEM_LINE: [u8; 3] = [0xcb, 0xa6, 0xf7];
const SPARK_MEM_FILL: [u8; 3] = [0xcb, 0xa6, 0xf7];

/// Overlay layout metrics.
struct OverlayMetrics {
    overlay_height: u32,
    overlay_font_px: f32,
    metric_font_px: f32,
    click_radius: i32,
    footer_pad_x: i32,
    token_pad_x: i32,
    token_pad_y: i32,
    token_gap: i32,
    token_radius: i32,
    border_width: i32,
    sparkline_width: i32,
    sparkline_height: i32,
    metric_gap: i32,
    canvas_margin: u32,
    section_gap: u32,
    section_radius: u32,
}

impl OverlayMetrics {
    fn for_height(terminal_height: u32) -> Self {
        let h = (terminal_height as f32 * 0.10).max(36.0) as u32;
        let fpx = (h as f32 * 0.22).max(9.0);
        let metric_fpx = fpx;
        let row_h = h as f32 / 2.0;
        Self {
            overlay_height: h,
            overlay_font_px: fpx,
            metric_font_px: metric_fpx,
            click_radius: (terminal_height as f32 * 0.008).max(4.0) as i32,
            footer_pad_x: (h as f32 * 0.25).max(8.0) as i32,
            token_pad_x: (h as f32 * 0.18).max(7.0) as i32,
            token_pad_y: (h as f32 * 0.04).max(2.0) as i32,
            token_gap: (h as f32 * 0.06).max(3.0) as i32,
            token_radius: (h as f32 * 0.06).max(3.0) as i32,
            border_width: 1,
            sparkline_width: (h as f32 * 1.0).max(35.0) as i32,
            sparkline_height: (row_h * 0.40).max(6.0) as i32,
            metric_gap: (h as f32 * 0.14).max(6.0) as i32,
            canvas_margin: (terminal_height as f32 * 0.028).max(12.0) as u32,
            section_gap: (terminal_height as f32 * 0.014).max(6.0) as u32,
            section_radius: (terminal_height as f32 * 0.016).max(8.0) as u32,
        }
    }
}

/// Render frames to an MP4 file using ffmpeg.
///
/// Frames are interpolated to the target FPS based on their timestamps:
/// for each output frame at time T, the latest captured frame with
/// `timestamp_ms <= T` is rendered. Idle periods produce repeated frames.
///
/// When `overlay` is true and `input_events` is non-empty, a KeyCastr-style
/// overlay is burned into each frame showing recent keystrokes, clicks, and
/// scroll events.
///
/// When `target_width` is specified, the font size is computed so the terminal
/// fills that width. The footer is appended below the terminal at the same
/// width.
pub fn export_mp4(
    frames: &[Frame],
    fps: u32,
    output_path: &std::path::Path,
    overlay: bool,
    input_events: &[InputEvent],
    target_width: Option<u32>,
) -> std::io::Result<()> {
    if frames.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "no frames to export",
        ));
    }

    let last_frame = frames.last().unwrap();
    let cols = last_frame.cols.max(1) as u32;
    let rows = last_frame.rows.max(1) as u32;

    let (font_px, cw, ch, pixel_width, pixel_height, pad_left, terminal_height) =
        compute_layout(cols, rows, overlay, target_width);

    let om = OverlayMetrics::for_height(terminal_height);

    let size_arg = format!("{pixel_width}x{pixel_height}");
    let fps_val = fps.max(1);
    let fps_arg = fps_val.to_string();

    let mut child = Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgb24",
            "-video_size",
            &size_arg,
            "-framerate",
            &fps_arg,
            "-i",
            "pipe:0",
            "-c:v",
            "libx264",
            "-pix_fmt",
            "yuv420p",
            "-preset",
            "ultrafast",
            "-tune",
            "stillimage",
        ])
        .arg(output_path.as_os_str())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    let stdin = child.stdin.as_mut().unwrap();
    let frame_bytes = (pixel_width * pixel_height * 3) as usize;

    let first_ts = frames.first().unwrap().timestamp_ms;
    let last_ts = frames.last().unwrap().timestamp_ms;
    let duration_ms = (last_ts - first_ts).max(100);
    let frame_interval_ms = 1000 / fps_val as u64;
    let total_output_frames = (duration_ms / frame_interval_ms).max(1) as usize;

    let mut frame_idx = 0;
    for i in 0..total_output_frames {
        let t = first_ts + i as u64 * frame_interval_ms;
        while frame_idx + 1 < frames.len() && frames[frame_idx + 1].timestamp_ms <= t {
            frame_idx += 1;
        }
        let mut rgb = render_frame_rgb(
            &frames[frame_idx],
            pixel_width,
            pixel_height,
            cw,
            ch,
            font_px,
            pad_left,
            &om,
        );
        if overlay {
            let mut pb = PixelBufMut::wrap(&mut rgb, pixel_width, pixel_height);
            render_overlay(
                &mut pb,
                t,
                input_events,
                cw,
                ch,
                pad_left,
                &om,
                &frames[frame_idx],
            );
        }
        debug_assert_eq!(rgb.len(), frame_bytes);
        stdin.write_all(&rgb)?;
    }

    drop(child.stdin.take());
    let status = child.wait()?;
    if !status.success() {
        return Err(std::io::Error::other("ffmpeg exited with non-zero status"));
    }

    Ok(())
}

/// Compute the video layout: font size, cell size, pixel dimensions, and horizontal padding.
///
/// When `target_width` is given, font size is scaled so the terminal content
/// (excluding margins) fills that width. The total frame includes canvas
/// margins around all edges, with the terminal and footer in bordered sections.
///
/// Returns `(font_px, cell_width, cell_height, pixel_width, pixel_height, pad_left, terminal_height)`.
fn compute_layout(
    cols: u32,
    rows: u32,
    overlay: bool,
    target_width: Option<u32>,
) -> (f32, u32, u32, u32, u32, u32, u32) {
    match target_width {
        Some(tw) => {
            // Derive font size from target width, accounting for margins.
            // We need: tw = margin*2 + terminal_width, so terminal_width = tw - margin*2.
            // But margin depends on terminal_height which depends on font_px.
            // Bootstrap: estimate font_px from full width, compute margin, then adjust.
            let initial_font_px = tw as f32 / cols as f32;
            let (_, initial_ch) = cell_size_at(initial_font_px);
            let initial_terminal_height = rows * initial_ch;
            let initial_om = OverlayMetrics::for_height(initial_terminal_height);
            let content_width = tw.saturating_sub(initial_om.canvas_margin * 2);
            let font_px = content_width as f32 / cols as f32;
            let (cw, ch) = cell_size_at(font_px);
            let terminal_width = cols * cw;
            let terminal_height = rows * ch;
            let om = OverlayMetrics::for_height(terminal_height);
            let content_area_width = tw.saturating_sub(om.canvas_margin * 2).max(terminal_width);
            let total_height = if overlay {
                om.canvas_margin * 2 + terminal_height + om.section_gap + om.overlay_height
            } else {
                om.canvas_margin * 2 + terminal_height
            };
            let mut pixel_width = om.canvas_margin * 2 + content_area_width;
            pixel_width = pixel_width.max(tw);
            pixel_width += pixel_width % 2;
            let mut pixel_height = total_height;
            pixel_height += pixel_height % 2;
            let pad_left =
                om.canvas_margin + (content_area_width.saturating_sub(terminal_width)) / 2;
            (
                font_px,
                cw,
                ch,
                pixel_width,
                pixel_height,
                pad_left,
                terminal_height,
            )
        }
        None => {
            let (cw, ch) = cell_size_at(DEFAULT_FONT_PX);
            let terminal_width = cols * cw;
            let terminal_height = rows * ch;
            let om = OverlayMetrics::for_height(terminal_height);
            let total_height = if overlay {
                om.canvas_margin * 2 + terminal_height + om.section_gap + om.overlay_height
            } else {
                om.canvas_margin * 2 + terminal_height
            };
            let mut pixel_width = om.canvas_margin * 2 + terminal_width;
            pixel_width += pixel_width % 2;
            let mut pixel_height = total_height;
            pixel_height += pixel_height % 2;
            (
                DEFAULT_FONT_PX,
                cw,
                ch,
                pixel_width,
                pixel_height,
                om.canvas_margin,
                terminal_height,
            )
        }
    }
}

/// Group consecutive single-character key events that are close in time into
/// combined badges, while giving special keys (Enter, Ctrl+C, etc.) their own badge.
fn group_keycaps(events: &[InputEvent], window_start: u64, t: u64) -> Vec<String> {
    let mut badges: Vec<String> = Vec::new();
    let mut current_group = String::new();
    let mut last_ts: u64 = 0;

    for event in events {
        if event.timestamp_ms <= window_start || event.timestamp_ms > t {
            continue;
        }
        match &event.kind {
            InputEventKind::Key(k) => {
                let is_single_char = k.display.len() == 1;
                let close_in_time = event.timestamp_ms - last_ts < 200;
                if is_single_char && (current_group.is_empty() || close_in_time) {
                    current_group.push_str(&k.display);
                } else {
                    if !current_group.is_empty() {
                        badges.push(current_group.clone());
                        current_group.clear();
                    }
                    if is_single_char {
                        current_group.push_str(&k.display);
                    } else {
                        badges.push(k.display.clone());
                    }
                }
                last_ts = event.timestamp_ms;
            }
            InputEventKind::Scroll(s) => {
                if !current_group.is_empty() {
                    badges.push(current_group.clone());
                    current_group.clear();
                }
                let arrow = if matches!(s.direction, ScrollDirection::Up) {
                    "↑"
                } else {
                    "↓"
                };
                badges.push(format!("{arrow}{}", s.lines));
                last_ts = event.timestamp_ms;
            }
            InputEventKind::Click(_) => {}
        }
    }
    if !current_group.is_empty() {
        badges.push(current_group);
    }
    badges
}

fn format_memory(bytes: u64) -> String {
    const GB: u64 = 1_073_741_824;
    const MB: u64 = 1_048_576;
    if bytes >= GB {
        format!("{:.1}GB", bytes as f64 / GB as f64)
    } else {
        format!("{:.1}MB", bytes as f64 / MB as f64)
    }
}

fn format_cpu_value(cpu_percent: f32) -> String {
    format!("{:.1}%", cpu_percent)
}

fn format_mem_value(memory_bytes: u64) -> String {
    format_memory(memory_bytes)
}

/// Convert memory history (timestamp, bytes) to (timestamp, f32) for sparkline rendering.
fn mem_history_as_f32(history: &[(u64, u64)]) -> Vec<(u64, f32)> {
    history.iter().map(|(ts, b)| (*ts, *b as f32)).collect()
}

/// Render a filled-area sparkline chart.
///
/// The chart fills the rectangle `(x, y, width, height)` with a soft gradient
/// area from the line down to the bottom. A 1px line traces the top of
/// the area. The leftmost 20% fades from transparent to full opacity to hide
/// the hard left edge where data ends.
#[allow(clippy::too_many_arguments)]
fn render_sparkline(
    pb: &mut PixelBufMut<'_>,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    history: &[(u64, f32)],
    frame_time: u64,
    line_color: [u8; 3],
    fill_color: [u8; 3],
    max_value: Option<f32>,
) {
    if width < 2 || height < 2 || history.is_empty() {
        return;
    }

    let window_ms = HISTORY_WINDOW_MS as f64;
    let window_start = frame_time.saturating_sub(HISTORY_WINDOW_MS) as f64;

    let peak = match max_value {
        Some(mv) => mv,
        None => {
            let raw = history
                .iter()
                .map(|(_, v)| *v)
                .fold(0.0f32, f32::max)
                .max(1.0);
            raw * 1.3
        }
    };

    let fade_end = (width as f64 * 0.40) as i32;

    for col in 0..width {
        let t_at_col = window_start + (col as f64 / width as f64) * window_ms;

        let val = interpolate_history(history, t_at_col);
        let normalized = (val / peak).clamp(0.0, 1.0);
        let line_y_offset = ((1.0 - normalized as f64) * (height - 1) as f64).round() as i32;

        let fade_factor = if col < fade_end && fade_end > 0 {
            col as f64 / fade_end as f64
        } else {
            1.0
        };

        let fill_alpha = (255.0 * 0.30 * fade_factor).round() as u16;
        let line_alpha = (255.0 * 0.60 * fade_factor).round() as u16;

        for row in line_y_offset..height {
            let px = x + col;
            let py = y + row;
            if px < 0 || py < 0 || px as u32 >= pb.width || py as u32 >= pb.height {
                continue;
            }
            let off = ((py as u32 * pb.width + px as u32) * 3) as usize;
            let (a, color) = if row == line_y_offset {
                (line_alpha, line_color)
            } else {
                (fill_alpha, fill_color)
            };
            pb.data[off] = alpha_blend(pb.data[off], color[0], a);
            pb.data[off + 1] = alpha_blend(pb.data[off + 1], color[1], a);
            pb.data[off + 2] = alpha_blend(pb.data[off + 2], color[2], a);
        }
    }
}

/// Linearly interpolate a value at a given timestamp from history samples.
fn interpolate_history(history: &[(u64, f32)], t: f64) -> f32 {
    if history.is_empty() {
        return 0.0;
    }
    if history.len() == 1 {
        return history[0].1;
    }
    if t <= history[0].0 as f64 {
        return history[0].1;
    }
    let last = history.last().unwrap();
    if t >= last.0 as f64 {
        return last.1;
    }
    for window in history.windows(2) {
        let (t0, v0) = (window[0].0 as f64, window[0].1);
        let (t1, v1) = (window[1].0 as f64, window[1].1);
        if t >= t0 && t <= t1 {
            let frac = if (t1 - t0).abs() < 1e-9 {
                0.0
            } else {
                (t - t0) / (t1 - t0)
            };
            return v0 + (v1 - v0) * frac as f32;
        }
    }
    last.1
}

/// Render the wait-for status as a badge (rounded rect) in the given row.
///
/// While waiting, shows a braille spinner; once found, shows a green checkmark.
/// The badge size is computed from the waiting text and stays constant to prevent
/// layout shift. Disappears 2 seconds after the pattern is found.
fn render_wait_badge(
    pb: &mut PixelBufMut<'_>,
    status: &WaitStatus,
    frame_time: u64,
    row_top: i32,
    row_height: i32,
    om: &OverlayMetrics,
) {
    let (wait_text, found_ms) = match status {
        WaitStatus::Waiting { text, .. } => (text.as_str(), None),
        WaitStatus::Found { text, found_ms } => {
            if frame_time > found_ms + WAIT_FOUND_DISPLAY_MS {
                return;
            }
            (text.as_str(), Some(*found_ms))
        }
    };

    let canonical_text = format!("{} waiting for \"{wait_text}\"", SPINNER_FRAMES[0]);
    let fpx = om.overlay_font_px;

    let font = &*FONT;
    let lm = font.horizontal_line_metrics(fpx);
    let ascent = lm.map(|m| m.ascent).unwrap_or(fpx * 0.8);
    let descent = lm.map(|m| m.descent).unwrap_or(-fpx * 0.2);
    let line_h = (ascent - descent).ceil() as i32;

    let text_w = measure_text_width(&canonical_text, fpx);
    let token_h = line_h + om.token_pad_y * 2;
    let token_w = text_w + om.token_pad_x * 2;
    let row_margin = (row_height as f32 * 0.1).max(2.0) as i32;
    let usable_h = row_height - row_margin * 2;
    let token_y = row_top + row_margin + ((usable_h - token_h).max(0) / 2);
    let baseline_y = token_y + om.token_pad_y + ascent.ceil() as i32;
    let token_x = om.canvas_margin as i32 + om.footer_pad_x;

    let is_found = found_ms.is_some();
    let border_color = if is_found {
        WAIT_FOUND_COLOR
    } else {
        TOKEN_BORDER
    };

    draw_token_rect(pb, token_x, token_y, token_w, token_h, om, border_color);

    let (prefix_char, prefix_color) = if is_found {
        ("✓", WAIT_FOUND_COLOR)
    } else {
        let idx = ((frame_time / 500) % SPINNER_FRAMES.len() as u64) as usize;
        (SPINNER_FRAMES[idx], WAIT_SPINNER_COLOR)
    };

    let text_x = token_x + om.token_pad_x;
    let prefix_w = measure_text_width(prefix_char, fpx);

    draw_text(pb, prefix_char, text_x, baseline_y, fpx, prefix_color);

    let suffix = format!(" waiting for \"{wait_text}\"");
    draw_text(pb, &suffix, text_x + prefix_w, baseline_y, fpx, TOKEN_TEXT);
}

/// Render the dual-row metrics panel (CPU + MEM) on the right side of the footer.
fn render_metrics_panel(
    pb: &mut PixelBufMut<'_>,
    frame: &Frame,
    frame_time: u64,
    om: &OverlayMetrics,
    footer_y: u32,
) {
    let bar_top = footer_y as i32;
    let row_h = om.overlay_height as i32 / 2;

    let font = &*FONT;
    let lm = font.horizontal_line_metrics(om.metric_font_px);
    let ascent = lm.map(|m| m.ascent).unwrap_or(om.metric_font_px * 0.8);
    let descent = lm.map(|m| m.descent).unwrap_or(-om.metric_font_px * 0.2);
    let text_h = (ascent - descent).ceil() as i32;

    let cpu_val_text = format_cpu_value(frame.cpu_percent);
    let mem_val_text = format_mem_value(frame.memory_bytes);

    let cpu_label_w = measure_text_width("CPU", om.metric_font_px);
    let mem_label_w = measure_text_width("MEM", om.metric_font_px);
    let cpu_val_w = measure_text_width(&cpu_val_text, om.metric_font_px);
    let mem_val_w = measure_text_width(&mem_val_text, om.metric_font_px);

    let label_w = cpu_label_w.max(mem_label_w);
    let val_w = cpu_val_w.max(mem_val_w);
    let gap = om.metric_gap;
    let spark_w = om.sparkline_width;
    let spark_h = om.sparkline_height;

    let inset = om.canvas_margin as i32;
    let panel_w = spark_w + gap + val_w + gap + label_w;
    let panel_right = pb.width as i32 - inset - om.footer_pad_x;
    let panel_left = panel_right - panel_w;

    if panel_left < inset + om.footer_pad_x {
        return;
    }

    let mem_history_f32 = mem_history_as_f32(&frame.mem_history);

    let render_metric_row = |pb: &mut PixelBufMut<'_>,
                             row_top: i32,
                             label: &str,
                             value: &str,
                             history: &[(u64, f32)],
                             line_color: [u8; 3],
                             fill_color: [u8; 3],
                             max_val: Option<f32>| {
        let baseline_y = row_top + (row_h - text_h) / 2 + ascent.ceil() as i32;

        let spark_x = panel_left;
        let spark_y = row_top + (row_h - spark_h) / 2;
        render_sparkline(
            pb, spark_x, spark_y, spark_w, spark_h, history, frame_time, line_color, fill_color,
            max_val,
        );

        // Value after sparkline.
        let val_x = panel_left + spark_w + gap;
        draw_text(
            pb,
            value,
            val_x,
            baseline_y,
            om.metric_font_px,
            METRIC_VALUE,
        );

        // Label on the far right.
        let label_x = panel_left + spark_w + gap + val_w + gap;
        draw_text(
            pb,
            label,
            label_x,
            baseline_y,
            om.metric_font_px,
            METRIC_LABEL,
        );
    };

    render_metric_row(
        pb,
        bar_top,
        "CPU",
        &cpu_val_text,
        &frame.cpu_history,
        SPARK_CPU_LINE,
        SPARK_CPU_FILL,
        Some(100.0),
    );
    render_metric_row(
        pb,
        bar_top + row_h,
        "MEM",
        &mem_val_text,
        &mem_history_f32,
        SPARK_MEM_LINE,
        SPARK_MEM_FILL,
        None,
    );
}

/// Render the overlay footer. Always fills the footer area.
/// Text fades based on age — newest keystrokes are brightest.
#[allow(clippy::too_many_arguments)]
fn render_overlay(
    pb: &mut PixelBufMut<'_>,
    t: u64,
    events: &[InputEvent],
    cw: u32,
    ch: u32,
    pad_left: u32,
    om: &OverlayMetrics,
    frame: &Frame,
) {
    let margin = om.canvas_margin;
    let terminal_height = frame.rows.max(1) as u32 * ch;
    let content_width = pb.width.saturating_sub(margin * 2);
    let footer_x = margin;
    let footer_y = margin + terminal_height + om.section_gap;

    for py in footer_y..footer_y + om.overlay_height {
        if py >= pb.height {
            break;
        }
        for dx in 0..content_width {
            let px = footer_x + dx;
            if px >= pb.width {
                break;
            }
            if !is_inside_rounded_rect(
                dx as i32,
                (py - footer_y) as i32,
                content_width as i32,
                om.overlay_height as i32,
                om.section_radius as i32,
            ) {
                continue;
            }
            let off = ((py * pb.width + px) * 3) as usize;
            pb.data[off] = FOOTER_BG[0];
            pb.data[off + 1] = FOOTER_BG[1];
            pb.data[off + 2] = FOOTER_BG[2];
        }
    }

    let window_start = t.saturating_sub(OVERLAY_FADE_MS);

    for event in events {
        if event.timestamp_ms <= window_start || event.timestamp_ms > t {
            continue;
        }
        if let InputEventKind::Click(c) = &event.kind {
            render_click_indicator(pb, c.row, c.col, cw, ch, pad_left, margin, om);
        }
    }

    let row_h = om.overlay_height as i32 / 2;
    let bar_top_i = footer_y as i32;
    let row1_top = bar_top_i;
    let row2_top = bar_top_i + row_h;

    let keys = group_keycaps(events, window_start, t);
    if !keys.is_empty() {
        render_footer_text(pb, &keys, row1_top, row_h, om);
    }

    if let Some(ref ws) = frame.wait_status {
        render_wait_badge(pb, ws, t, row2_top, row_h, om);
    }

    render_metrics_panel(pb, frame, t, om, footer_y);
}

/// Alpha-blend a single channel.
fn alpha_blend(base: u8, overlay: u8, alpha: u16) -> u8 {
    let inv = 255 - alpha;
    ((overlay as u16 * alpha + base as u16 * inv) / 255) as u8
}

fn measure_text_width(text: &str, font_px: f32) -> i32 {
    let font = &*FONT;
    let mut w = 0.0f32;
    let chars: Vec<char> = text.chars().collect();
    for (i, ch) in chars.iter().enumerate() {
        let m = font.metrics(*ch, font_px);
        if i + 1 < chars.len() {
            w += m.advance_width;
        } else {
            // Last char: use the wider of advance or actual pixel extent.
            let pixel_extent = m.xmin as f32 + m.width as f32;
            w += m.advance_width.max(pixel_extent);
        }
    }
    w.ceil() as i32
}

fn is_inside_rounded_rect(x: i32, y: i32, w: i32, h: i32, r: i32) -> bool {
    let r = r.min(w / 2).min(h / 2).max(0);
    if x < 0 || y < 0 || x >= w || y >= h {
        return false;
    }
    if (x >= r && x < w - r) || (y >= r && y < h - r) {
        return true;
    }
    let cx = if x < r { r } else { w - r - 1 };
    let cy = if y < r { r } else { h - r - 1 };
    let dx = x - cx;
    let dy = y - cy;
    dx * dx + dy * dy <= r * r
}

fn draw_token_rect(
    pb: &mut PixelBufMut<'_>,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    om: &OverlayMetrics,
    border_color: [u8; 3],
) {
    for row in 0..h {
        let py = y + row;
        if py < 0 || py as u32 >= pb.height {
            continue;
        }
        for col in 0..w {
            let px = x + col;
            if px < 0 || px as u32 >= pb.width {
                continue;
            }
            if !is_inside_rounded_rect(col, row, w, h, om.token_radius) {
                continue;
            }
            let inner = is_inside_rounded_rect(
                col - om.border_width,
                row - om.border_width,
                w - om.border_width * 2,
                h - om.border_width * 2,
                (om.token_radius - om.border_width).max(0),
            );
            let color = if inner { TOKEN_BG } else { border_color };
            let off = ((py as u32 * pb.width + px as u32) * 3) as usize;
            pb.data[off] = color[0];
            pb.data[off + 1] = color[1];
            pb.data[off + 2] = color[2];
        }
    }
}

fn draw_text(
    pb: &mut PixelBufMut<'_>,
    text: &str,
    x: i32,
    baseline_y: i32,
    font_px: f32,
    color: [u8; 3],
) {
    let mut glyph_x = x;
    for ch in text.chars() {
        let font = match font_for_char(ch) {
            Some(f) => f,
            None => {
                let m = FONT.metrics('?', font_px);
                glyph_x += m.advance_width.ceil() as i32;
                continue;
            }
        };
        let (metrics, bitmap) = font.rasterize(ch, font_px);
        if bitmap.is_empty() || metrics.width == 0 || metrics.height == 0 {
            glyph_x += metrics.advance_width.ceil() as i32;
            continue;
        }
        let dx = glyph_x + metrics.xmin;
        let dy = baseline_y - (metrics.height as i32 + metrics.ymin);
        for row in 0..metrics.height {
            let py = dy + row as i32;
            if py < 0 || py as u32 >= pb.height {
                continue;
            }
            for col in 0..metrics.width {
                let px = dx + col as i32;
                if px < 0 || px as u32 >= pb.width {
                    continue;
                }
                let glyph_alpha = bitmap[row * metrics.width + col];
                if glyph_alpha == 0 {
                    continue;
                }
                let off = ((py as u32 * pb.width + px as u32) * 3) as usize;
                if glyph_alpha == 255 {
                    pb.data[off] = color[0];
                    pb.data[off + 1] = color[1];
                    pb.data[off + 2] = color[2];
                } else {
                    let a = glyph_alpha as u16;
                    let inv = 255 - a;
                    pb.data[off] = ((color[0] as u16 * a + pb.data[off] as u16 * inv) / 255) as u8;
                    pb.data[off + 1] =
                        ((color[1] as u16 * a + pb.data[off + 1] as u16 * inv) / 255) as u8;
                    pb.data[off + 2] =
                        ((color[2] as u16 * a + pb.data[off + 2] as u16 * inv) / 255) as u8;
                }
            }
        }
        glyph_x += metrics.advance_width.ceil() as i32;
    }
}

/// Render grouped keys as tasteful, left-anchored shadcn-like kbd tokens in the given row.
fn render_footer_text(
    pb: &mut PixelBufMut<'_>,
    keys: &[String],
    row_top: i32,
    row_height: i32,
    om: &OverlayMetrics,
) {
    let font = &*FONT;
    let fpx = om.overlay_font_px;
    let line_metrics = font.horizontal_line_metrics(fpx);
    let ascent = line_metrics.map(|lm| lm.ascent).unwrap_or(fpx * 0.8);
    let descent = line_metrics.map(|lm| lm.descent).unwrap_or(-fpx * 0.2);
    let line_h = (ascent - descent).ceil() as i32;

    let token_h = line_h + om.token_pad_y * 2;
    let margin = (row_height as f32 * 0.1).max(2.0) as i32;
    let usable_h = row_height - margin * 2;
    let token_y = row_top + margin + ((usable_h - token_h).max(0) / 2);
    let baseline_y = token_y + om.token_pad_y + ascent.ceil() as i32;

    let inset = om.canvas_margin as i32;
    let max_x = pb.width as i32 / 2;
    let start_x = inset + om.footer_pad_x;
    let mut x = start_x;
    let mut visible: Vec<&String> = Vec::new();
    let mut test_x = start_x;
    for key in keys.iter().rev() {
        let text_w = measure_text_width(key, fpx);
        let token_w = text_w + om.token_pad_x * 2 + om.token_gap;
        if test_x + token_w > max_x && !visible.is_empty() {
            break;
        }
        test_x += token_w;
        visible.push(key);
    }
    visible.reverse();

    for key in &visible {
        let text_w = measure_text_width(key, fpx);
        let token_w = text_w + om.token_pad_x * 2;
        if x + token_w > max_x {
            break;
        }
        draw_token_rect(pb, x, token_y, token_w, token_h, om, TOKEN_BORDER);
        draw_text(pb, key, x + om.token_pad_x, baseline_y, fpx, TOKEN_TEXT);
        x += token_w + om.token_gap;
    }
}

/// Draw a filled circle at the click position (in terminal cell coordinates).
#[allow(clippy::too_many_arguments)]
fn render_click_indicator(
    pb: &mut PixelBufMut<'_>,
    row: u16,
    col: u16,
    cw: u32,
    ch: u32,
    pad_left: u32,
    canvas_margin: u32,
    om: &OverlayMetrics,
) {
    let cx = pad_left as i32 + (col as i32 * cw as i32) + (cw as i32 / 2);
    let cy = canvas_margin as i32 + (row as i32 * ch as i32) + (ch as i32 / 2);
    let click_radius = om.click_radius;
    let r2 = click_radius * click_radius;

    for dy in -click_radius..=click_radius {
        for dx in -click_radius..=click_radius {
            if dx * dx + dy * dy > r2 {
                continue;
            }
            let px = cx + dx;
            let py = cy + dy;
            if px < 0 || py < 0 || px as u32 >= pb.width || py as u32 >= pb.height {
                continue;
            }
            let off = ((py as u32 * pb.width + px as u32) * 3) as usize;
            pb.data[off] = alpha_blend(pb.data[off], CLICK_COLOR[0], 180);
            pb.data[off + 1] = alpha_blend(pb.data[off + 1], CLICK_COLOR[1], 180);
            pb.data[off + 2] = alpha_blend(pb.data[off + 2], CLICK_COLOR[2], 180);
        }
    }
}

/// Mutable borrowed view over an existing RGB pixel buffer.
struct PixelBufMut<'a> {
    data: &'a mut [u8],
    width: u32,
    height: u32,
}

impl<'a> PixelBufMut<'a> {
    fn wrap(data: &'a mut [u8], width: u32, height: u32) -> Self {
        Self {
            data,
            width,
            height,
        }
    }
}

struct PixelBuf {
    data: Vec<u8>,
    width: u32,
    height: u32,
}

impl PixelBuf {
    fn new_with_bg(width: u32, height: u32, bg: [u8; 3]) -> Self {
        let pixel_count = (width * height) as usize;
        let mut data = vec![0u8; pixel_count * 3];
        for i in 0..pixel_count {
            let off = i * 3;
            data[off] = bg[0];
            data[off + 1] = bg[1];
            data[off + 2] = bg[2];
        }
        Self {
            data,
            width,
            height,
        }
    }

    fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: [u8; 3]) {
        for dy in 0..h {
            let py = y + dy;
            if py >= self.height {
                break;
            }
            for dx in 0..w {
                let px = x + dx;
                if px >= self.width {
                    break;
                }
                let off = ((py * self.width + px) * 3) as usize;
                self.data[off] = color[0];
                self.data[off + 1] = color[1];
                self.data[off + 2] = color[2];
            }
        }
    }

    fn blend_glyph(&mut self, glyph: &GlyphPlacement, fg: [u8; 3], bg: [u8; 3]) {
        for dy in 0..glyph.height {
            let py = glyph.y + dy as i32;
            if py < 0 {
                continue;
            }
            let py = py as u32;
            if py >= self.height {
                break;
            }
            for dx in 0..glyph.width {
                let px = glyph.x + dx as i32;
                if px < 0 {
                    continue;
                }
                let px = px as u32;
                if px >= self.width {
                    break;
                }
                let alpha = glyph.bitmap[(dy * glyph.width + dx) as usize];
                if alpha == 0 {
                    continue;
                }
                let off = ((py * self.width + px) * 3) as usize;
                if alpha == 255 {
                    self.data[off] = fg[0];
                    self.data[off + 1] = fg[1];
                    self.data[off + 2] = fg[2];
                } else {
                    let a = alpha as u16;
                    let inv = 255 - a;
                    self.data[off] = ((fg[0] as u16 * a + bg[0] as u16 * inv) / 255) as u8;
                    self.data[off + 1] = ((fg[1] as u16 * a + bg[1] as u16 * inv) / 255) as u8;
                    self.data[off + 2] = ((fg[2] as u16 * a + bg[2] as u16 * inv) / 255) as u8;
                }
            }
        }
    }
}

struct GlyphPlacement<'a> {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    bitmap: &'a [u8],
}

#[allow(clippy::too_many_arguments)]
fn render_frame_rgb(
    frame: &Frame,
    width: u32,
    height: u32,
    cw: u32,
    ch: u32,
    font_px: f32,
    pad_left: u32,
    om: &OverlayMetrics,
) -> Vec<u8> {
    let mut pb = PixelBuf::new_with_bg(width, height, CANVAS_BG);

    let terminal_width = frame.cols.max(1) as u32 * cw;
    let terminal_height = frame.rows.max(1) as u32 * ch;
    let margin = om.canvas_margin;
    let term_y = margin;

    let font = &*FONT;
    let line_metrics = font.horizontal_line_metrics(font_px);
    let ascent = line_metrics.map(|lm| lm.ascent).unwrap_or(font_px * 0.8);

    for (row_idx, row) in frame.cells.iter().enumerate() {
        if row_idx as u32 >= frame.rows as u32 {
            break;
        }
        let mut skip_next = false;
        for (col_idx, cell) in row.iter().enumerate() {
            if col_idx as u32 >= frame.cols as u32 {
                break;
            }
            let px_x = pad_left + col_idx as u32 * cw;
            let px_y = term_y + row_idx as u32 * ch;

            if skip_next {
                skip_next = false;
                continue;
            }

            pb.fill_rect(px_x, px_y, cw, ch, cell.bg);

            if cell.ch <= ' ' {
                continue;
            }

            let cp = cell.ch as u32;
            if matches!(cp,
                0xFE00..=0xFE0F
                | 0xE0100..=0xE01EF
                | 0x200B..=0x200F
                | 0x2028..=0x202F
                | 0x2060..=0x2069
                | 0xFEFF
                | 0x00AD
            ) {
                continue;
            }

            let is_wide = cp > 0x1F000
                || (0x2E80..=0x9FFF).contains(&cp)
                || (0xF900..=0xFAFF).contains(&cp)
                || (0xFE30..=0xFE4F).contains(&cp)
                || (0x1F300..=0x1FAFF).contains(&cp);

            if is_wide {
                let next_col = col_idx + 1;
                if (next_col as u32) < frame.cols as u32 {
                    let next_px_x = pad_left + next_col as u32 * cw;
                    pb.fill_rect(next_px_x, px_y, cw, ch, cell.bg);
                    skip_next = true;
                }
            }

            let glyph_font = match font_for_char(cell.ch) {
                Some(f) => f,
                None => continue,
            };
            let (metrics, bitmap) = glyph_font.rasterize(cell.ch, font_px);
            if bitmap.is_empty() || metrics.width == 0 || metrics.height == 0 {
                continue;
            }

            let cell_span = if is_wide { cw * 2 } else { cw };
            let glyph = GlyphPlacement {
                x: px_x as i32
                    + metrics.xmin
                    + ((cell_span as i32 - metrics.advance_width as i32) / 2),
                y: px_y as i32 + ascent as i32 - (metrics.height as i32 + metrics.ymin),
                width: metrics.width as u32,
                height: metrics.height as u32,
                bitmap: &bitmap,
            };

            pb.blend_glyph(&glyph, cell.fg, cell.bg);
        }
    }

    // Clip terminal corners to rounded rect.
    let r = om.section_radius as i32;
    for dy in 0..terminal_height {
        for dx in 0..terminal_width {
            if !is_inside_rounded_rect(
                dx as i32,
                dy as i32,
                terminal_width as i32,
                terminal_height as i32,
                r,
            ) {
                let px = margin + dx;
                let py = term_y + dy;
                let off = ((py * pb.width + px) * 3) as usize;
                pb.data[off] = CANVAS_BG[0];
                pb.data[off + 1] = CANVAS_BG[1];
                pb.data[off + 2] = CANVAS_BG[2];
            }
        }
    }

    pb.data
}
