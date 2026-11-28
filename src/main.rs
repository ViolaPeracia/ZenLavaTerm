//! LavaTerm — CLI Entry point & Terminal Event Loop.

use clap::Parser;
use crossterm::{
    cursor,
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use lavaterm::{
    audio::{create_audio_provider, list_audio_devices, AudioSourceKind},
    config::{default_config_path, load_config, run_setup_wizard, AudioConfig, WizardOutcome},
    core::{Interaction, PhysicsParams, Simulation},
    input::{map_key_event_with_ripple, ripple_origin_for_char, Action, MouseTracker},
    reactive::default_system_provider,
    render::{
        rasterize_simulation, BlockRenderer, BrailleRenderer, ColorPalette, CursorMode,
        HalfBlockRenderer, RasterizeOptions, Renderer, Rgb, VirtualFramebuffer,
    },
    theme::{load_custom_theme_file, resolve_theme},
    widget::{
        render_snapshot_with_options, resolve_policy, should_compact, CompactScaler, ExecutionMode,
        PolicyInput,
    },
    LavaError, Result,
};

use std::{
    io::{stdin, stdout, BufWriter, IsTerminal, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    sync::Arc,
    time::{Duration, Instant},
};

const INTERACTIVE_CONFIG_SENTINEL: &str = "__LAVATERM_INTERACTIVE_CONFIG__";

/// LavaTerm: Terminal-native ambient lava lamp visualizer.
#[derive(Parser, Debug)]
#[command(name = "lavaterm", author, version, about, long_about = None)]
struct Cli {
    /// Open interactive default-config setup menu (without path) or load a custom TOML file (with path)
    #[arg(
        short,
        long,
        value_name = "PATH",
        num_args = 0..=1,
        default_missing_value = INTERACTIVE_CONFIG_SENTINEL
    )]
    config: Option<PathBuf>,

    /// Open interactive keyboard setup menu to edit and save default settings
    #[arg(long)]
    setup: bool,

    /// Skip the automatic first-run setup wizard
    #[arg(long)]
    no_setup: bool,

    /// Renderer backend: halfblock | block | braille
    #[arg(short, long, value_name = "TYPE")]
    renderer: Option<String>,

    /// Target frames per second
    #[arg(long, value_name = "FPS")]
    fps: Option<u32>,

    /// Number of metaball blobs
    #[arg(long, value_name = "COUNT")]
    blobs: Option<usize>,

    /// Theme preset, auto-detect, or theme file path (e.g. ocean, cyberpunk, synthwave, auto, pywal, wallust)
    #[arg(short, long, value_name = "THEME")]
    theme: Option<String>,

    /// Force compact geometry & profile scaling
    #[arg(long)]
    compact: bool,

    /// Run as low-overhead ambient widget (default 15 FPS, compact physics)
    #[arg(long)]
    widget: bool,

    /// Run inline in terminal without entering alternate screen
    #[arg(long)]
    inline: bool,

    /// Render a single ANSI frame to stdout and exit
    #[arg(long)]
    snapshot: bool,

    /// Explicit viewport width (columns)
    #[arg(long, value_name = "COLS")]
    width: Option<u16>,

    /// Explicit viewport height (rows)
    #[arg(long, value_name = "ROWS")]
    height: Option<u16>,

    /// Enable ambient system-reactive visualizer mode (CPU/RAM/Battery)
    #[arg(long)]
    system: bool,

    /// Enable audio-reactive visualizer mode (FFT bass/mid/treble)
    #[arg(long)]
    audio: bool,

    /// Target audio input or capture device name (implicitly enables --audio)
    #[arg(long, value_name = "DEVICE")]
    audio_device: Option<String>,

    /// Capture system audio (loopback) instead of microphone on Windows WASAPI (implicitly enables --audio)
    #[arg(long)]
    audio_loopback: bool,

    /// Exit with an error instead of falling back to synthetic audio when live capture is unavailable
    #[arg(long)]
    no_audio_fallback: bool,

    /// List available audio capture devices and exit
    #[arg(long)]
    list_audio_devices: bool,

    /// Disable mouse click shockwaves, dragging, and scroll pressure
    #[arg(long)]
    no_mouse: bool,

    /// Disable keyboard ripples on character keypresses
    #[arg(long)]
    no_ripple: bool,

    /// Multiplier for mouse click shockwave force (default: 1.0)
    #[arg(long, value_name = "FORCE")]
    shockwave_force: Option<f32>,

    /// Multiplier for mouse drag stirring force (default: 1.0)
    #[arg(long, value_name = "FORCE")]
    stir_force: Option<f32>,

    /// Run headless simulation without taking over TTY (useful for testing/CI)
    #[arg(long)]
    headless: bool,

    /// Number of frames to step when in headless mode
    #[arg(long, default_value_t = 60)]
    frames: usize,
}

const DEFAULT_SEED: u64 = 1337;

struct TerminalGuard {
    raw_mode: bool,
    alternate_screen: bool,
    mouse_enabled: bool,
    cursor_hidden: bool,
    line_wrap_disabled: bool,
    osc_bg_set: bool,
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let mut out = stdout();
        if self.line_wrap_disabled {
            let _ = out.write_all(b"\x1b[?7h");
        }
        if self.osc_bg_set {
            let _ = out.write_all(b"\x1b]111\x07");
        }
        if self.mouse_enabled {
            let _ = execute!(out, DisableMouseCapture);
        }
        if self.alternate_screen && self.cursor_hidden {
            let _ = execute!(out, LeaveAlternateScreen, cursor::Show);
        } else if self.alternate_screen {
            let _ = execute!(out, LeaveAlternateScreen);
        } else if self.cursor_hidden {
            let _ = execute!(out, cursor::Show);
        }
        let _ = out.flush();
        if self.raw_mode {
            let _ = terminal::disable_raw_mode();
        }
    }
}

fn restore_terminal(alternate_screen: bool, mouse_enabled: bool) {
    let mut out = stdout();
    let _ = out.write_all(b"\x1b[?7h\x1b]111\x07");
    let _ = out.flush();
    let _ = terminal::disable_raw_mode();
    if mouse_enabled {
        let _ = execute!(stdout(), DisableMouseCapture);
    }
    if alternate_screen {
        let _ = execute!(stdout(), LeaveAlternateScreen, cursor::Show);
    } else {
        let _ = execute!(stdout(), cursor::Show);
    }
}

fn setup_panic_hook(alternate_screen: bool, mouse_enabled: bool) {
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        restore_terminal(alternate_screen, mouse_enabled);
        original_hook(panic_info);
    }));
}

#[cfg(unix)]
fn setup_signal_handler(term_flag: Arc<AtomicBool>) {
    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&term_flag));
    let _ = signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&term_flag));
}

#[cfg(windows)]
fn setup_signal_handler(term_flag: Arc<AtomicBool>) {
    use std::sync::OnceLock;
    static GLOBAL_TERM_FLAG: OnceLock<Arc<AtomicBool>> = OnceLock::new();
    let _ = GLOBAL_TERM_FLAG.get_or_init(|| term_flag);

    unsafe extern "system" fn ctrl_handler(_ctrl_type: u32) -> i32 {
        if let Some(flag) = GLOBAL_TERM_FLAG.get() {
            flag.store(true, Ordering::SeqCst);
        }
        1
    }

    extern "system" {
        fn SetConsoleCtrlHandler(
            HandlerRoutine: Option<unsafe extern "system" fn(u32) -> i32>,
            Add: i32,
        ) -> i32;
    }

    unsafe {
        let ret = SetConsoleCtrlHandler(Some(ctrl_handler), 1);
        if ret == 0 {
            eprintln!("Warning: Failed to register Windows console control handler");
        }
    }
}

#[cfg(not(any(unix, windows)))]
fn setup_signal_handler(_term_flag: Arc<AtomicBool>) {}

struct RuntimeOptions {
    renderer_type: String,
    theme_label: String,
    fps: u32,
    raster_opts: RasterizeOptions,
    base_blob_count: usize,
    force_compact: bool,
    adapt_blobs: bool,
    system_reactive: bool,
    audio_reactive: bool,
    allow_audio_fallback: bool,
    audio_config: AudioConfig,
    poll_interval_ms: u64,
    mouse_enabled: bool,
    keyboard_ripple: bool,
    shockwave_force: f32,
    stir_force: f32,
    initial_hud_note: Option<String>,
}

struct HudState {
    shown_at: Option<Instant>,
    note: Option<String>,
}

impl HudState {
    const HOLD_SECS: f32 = 2.4;
    const FADE_SECS: f32 = 1.2;

    fn new(initial_note: Option<String>) -> Self {
        if let Some(note) = initial_note {
            Self {
                shown_at: Some(Instant::now()),
                note: Some(note),
            }
        } else {
            Self {
                shown_at: None,
                note: None,
            }
        }
    }

    fn trigger(&mut self, note: impl Into<String>) {
        self.shown_at = Some(Instant::now());
        self.note = Some(note.into());
    }

    fn current_alpha(&mut self, now: Instant) -> f32 {
        let Some(shown_at) = self.shown_at else {
            return 0.0;
        };
        let elapsed = now.duration_since(shown_at).as_secs_f32();
        if elapsed < Self::HOLD_SECS {
            1.0
        } else if elapsed < Self::HOLD_SECS + Self::FADE_SECS {
            let t = (elapsed - Self::HOLD_SECS) / Self::FADE_SECS;
            let rem = (1.0 - t).clamp(0.0, 1.0);
            rem * rem
        } else {
            self.shown_at = None;
            self.note = None;
            0.0
        }
    }
}

fn sample_under_cell(
    fb: &VirtualFramebuffer,
    cell_col: usize,
    cell_row: usize,
    renderer_type: &str,
    fallback: Rgb,
) -> Rgb {
    let (px, py) = match renderer_type {
        "block" => (cell_col, cell_row),
        "braille" => (cell_col * 2, cell_row * 4),
        _ => (cell_col, cell_row * 2),
    };
    fb.get_pixel(px, py).unwrap_or(fallback)
}

#[allow(clippy::too_many_arguments)]
fn render_status_overlay(
    out: &mut impl Write,
    fb: &VirtualFramebuffer,
    palette: &ColorPalette,
    cols: u16,
    rows: u16,
    is_fullscreen: bool,
    hud_alpha: f32,
    hud_note: Option<&str>,
    paused: bool,
    speed_scale: f32,
    opts: &RuntimeOptions,
    audio_source: Option<AudioSourceKind>,
) -> std::io::Result<()> {
    if rows < 3 || cols < 28 {
        return Ok(());
    }
    let max_cols = cols as usize;

    if hud_alpha > 0.01 && rows >= 3 {
        let mode_str = if let Some(src) = audio_source {
            format!("audio: {}", src.label())
        } else if opts.system_reactive {
            "system".to_string()
        } else {
            "ambient".to_string()
        };
        let state_tag = if paused { "PAUSED · " } else { "" };
        let header_line = format!(
            " LavaTerm · {} · {} · {}fps ",
            opts.theme_label, opts.renderer_type, opts.fps
        );
        let body_line = if let Some(note) = hud_note {
            format!(" {state_tag}{mode_str} · {speed_scale:.2}x · {note} ")
        } else {
            format!(" {state_tag}{mode_str} · {speed_scale:.2}x · Ctrl+p/r/i/q · +/- ")
        };

        let max_inner = max_cols.saturating_sub(4);
        let inner_w = header_line
            .chars()
            .count()
            .max(body_line.chars().count())
            .min(max_inner);

        if inner_w >= 12 {
            let start_col: usize = 2; // 1-based terminal column (top-left with 1-col margin)
            let top_label: String = header_line.chars().take(inner_w).collect();
            let top_fill = "─".repeat(inner_w.saturating_sub(top_label.chars().count()));
            let row0_chars: Vec<char> = format!("╭{top_label}{top_fill}╮").chars().collect();

            let body_trunc: String = body_line.chars().take(inner_w).collect();
            let body_pad = " ".repeat(inner_w.saturating_sub(body_trunc.chars().count()));
            let row1_chars: Vec<char> = format!("│{body_trunc}{body_pad}│").chars().collect();

            let row2_chars: Vec<char> = format!("╰{}╯", "─".repeat(inner_w)).chars().collect();

            let card_bg_target = Rgb::new(14, 14, 24);
            let border_target = palette.middle;
            let text_target = Rgb::new(235, 238, 252);
            let header_target = Rgb::new(255, 224, 175);

            if !is_fullscreen {
                // Save cursor at bottom row of inline frame, then move up to top row of inline frame
                out.write_all(b"\x1b7")?;
                if rows > 1 {
                    write!(out, "\x1b[{}A", rows - 1)?;
                }
            }

            for (r_idx, chars) in [row0_chars, row1_chars, row2_chars].iter().enumerate() {
                if is_fullscreen {
                    let term_row = r_idx + 1;
                    write!(out, "\x1b[{};{}H", term_row, start_col)?;
                } else {
                    if r_idx > 0 {
                        out.write_all(b"\r\n")?;
                    } else {
                        out.write_all(b"\r")?;
                    }
                    if start_col > 1 {
                        write!(out, "\x1b[{}C", start_col - 1)?;
                    }
                }
                for (c_off, &ch) in chars.iter().enumerate() {
                    let cell_col = (start_col - 1) + c_off;
                    let under = sample_under_cell(
                        fb,
                        cell_col,
                        r_idx,
                        &opts.renderer_type,
                        palette.background,
                    );
                    let is_border = matches!(ch, '╭' | '╮' | '╰' | '╯' | '─' | '│');
                    let fg_target = if is_border {
                        border_target
                    } else if r_idx == 0 {
                        header_target
                    } else {
                        text_target
                    };
                    let bg_col = Rgb::lerp(under, card_bg_target, hud_alpha * 0.88);
                    let fg_col = Rgb::lerp(under, fg_target, hud_alpha);
                    write!(
                        out,
                        "\x1b[48;2;{};{};{}m\x1b[38;2;{};{};{}m{ch}",
                        bg_col.r, bg_col.g, bg_col.b, fg_col.r, fg_col.g, fg_col.b
                    )?;
                }
                write!(out, "\x1b[0m")?;
            }

            if !is_fullscreen {
                // Restore cursor back to the exact bottom row so inline rewind remains accurate
                out.write_all(b"\x1b8")?;
            }
        }
    }

    if audio_source == Some(AudioSourceKind::Synthetic) {
        let badge = " [audio: synthetic fallback] ";
        if max_cols >= badge.len() {
            let start_col = max_cols - badge.len() + 1;
            if is_fullscreen {
                write!(
                    out,
                    "\x1b[{};{}H\x1b[48;2;42;24;16m\x1b[38;2;255;180;90m{badge}\x1b[0m",
                    rows, start_col
                )?;
            } else {
                write!(
                    out,
                    "\x1b7\r\x1b[{}C\x1b[48;2;42;24;16m\x1b[38;2;255;180;90m{badge}\x1b[0m\x1b8",
                    start_col.saturating_sub(1)
                )?;
            }
        }
    }

    Ok(())
}

fn apply_viewport_compact_profile(
    sim: &mut Simulation,
    cols: u16,
    rows: u16,
    opts: &RuntimeOptions,
) {
    if opts.adapt_blobs && should_compact(cols, rows, opts.force_compact) {
        let profile = CompactScaler::calculate_profile(cols, rows, opts.base_blob_count);
        CompactScaler::adapt_simulation(&profile, sim);
    } else {
        sim.apply_compact_scales(opts.base_blob_count, 1.0, 1.0, 1.0);
    }
}

fn run_headless(
    sim: &mut Simulation,
    palette: &ColorPalette,
    frames: usize,
    opts: &RuntimeOptions,
) -> Result<()> {
    let mut sys_provider = if opts.system_reactive {
        Some(default_system_provider())
    } else {
        None
    };
    let mut audio_provider = if opts.audio_reactive {
        Some(create_audio_provider(
            &opts.audio_config,
            opts.allow_audio_fallback,
        )?)
    } else {
        None
    };

    if let Some(ref ap) = audio_provider {
        println!(
            "Starting LavaTerm headless simulation ({} frames, system={}, audio=true [{}])...",
            frames,
            opts.system_reactive,
            ap.source_kind().label()
        );
    } else {
        println!(
            "Starting LavaTerm headless simulation ({} frames, system={}, audio={})...",
            frames, opts.system_reactive, opts.audio_reactive
        );
    }

    let mut fb = VirtualFramebuffer::new(80, 48, palette.background);
    let dt = 1.0 / 30.0;

    for frame in 0..frames {
        if let Some(ref mut ap) = audio_provider {
            if !opts.allow_audio_fallback && ap.source_kind() == AudioSourceKind::Synthetic {
                return Err(LavaError::Audio(
                    "Audio capture disconnected and --no-audio-fallback is set".to_string(),
                ));
            }
            let audio_sig = ap.poll_signals();
            sim.step_audio(dt, &audio_sig);
        } else if let Some(ref mut sp) = sys_provider {
            let signals = sp.poll_signals();
            sim.step_reactive(dt, &signals);
        } else {
            sim.step(dt);
        }
        rasterize_simulation(sim, &mut fb, palette, &opts.raster_opts);
        if frame % 20 == 0 || frame == frames - 1 {
            println!(
                "  [Frame {:03}/{:03}] Sim Time: {:.2}s | Blobs: {} | Active pixels in canvas: {}",
                frame + 1,
                frames,
                sim.elapsed_time,
                sim.blobs.len(),
                fb.as_slice()
                    .iter()
                    .filter(|c| **c != palette.background)
                    .count()
            );
        }
    }
    println!("Headless simulation completed successfully.");
    Ok(())
}

fn framebuffer_dimensions(cols: u16, rows: u16, renderer_type: &str) -> (usize, usize) {
    match renderer_type {
        "block" => (cols as usize, rows as usize),
        "braille" => (cols as usize * 2, rows as usize * 4),
        _ => (cols as usize, (rows as usize) * 2),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InteractiveMode {
    Fullscreen,
    Inline { target_height: u16 },
}

fn run_event_loop(
    mut sim: Simulation,
    palette: ColorPalette,
    mut opts: RuntimeOptions,
    fixed_dim: Option<(u16, u16)>,
    mode: InteractiveMode,
) -> Result<()> {
    let is_fullscreen = matches!(mode, InteractiveMode::Fullscreen);
    setup_panic_hook(is_fullscreen, opts.mouse_enabled);

    let shutdown_flag = Arc::new(AtomicBool::new(false));
    setup_signal_handler(Arc::clone(&shutdown_flag));

    // Initialize providers before entering raw mode so any initialization errors
    // or stderr fallback warnings never corrupt raw terminal output.
    let mut sys_provider = if opts.system_reactive {
        Some(default_system_provider())
    } else {
        None
    };
    let mut audio_provider = if opts.audio_reactive {
        Some(create_audio_provider(
            &opts.audio_config,
            opts.allow_audio_fallback,
        )?)
    } else {
        None
    };
    let mut last_metric_poll = Instant::now();
    let poll_duration = Duration::from_millis(opts.poll_interval_ms.max(100));
    let mut current_sys_signals = sys_provider.as_mut().map(|p| p.poll_signals());

    let mut guard = TerminalGuard {
        raw_mode: false,
        alternate_screen: false,
        mouse_enabled: false,
        cursor_hidden: false,
        line_wrap_disabled: false,
        osc_bg_set: false,
    };

    terminal::enable_raw_mode()?;
    guard.raw_mode = true;

    let mut out = BufWriter::with_capacity(256 * 1024, stdout());

    if is_fullscreen {
        execute!(out, EnterAlternateScreen, cursor::Hide)?;
        guard.alternate_screen = true;
        guard.cursor_hidden = true;
        // Disable auto-wrap so bottom-right cell never scrolls ConPTY/PowerShell,
        // and sync native terminal background (OSC 11) so window padding blends seamlessly.
        let _ = out.write_all(b"\x1b[?7l");
        guard.line_wrap_disabled = true;
        let _ = write!(
            out,
            "\x1b]11;#{:02x}{:02x}{:02x}\x07",
            palette.background.r, palette.background.g, palette.background.b
        );
        guard.osc_bg_set = true;
    } else {
        execute!(out, cursor::Hide)?;
        guard.cursor_hidden = true;
    }

    if opts.mouse_enabled && execute!(out, EnableMouseCapture).is_ok() {
        guard.mouse_enabled = true;
    }

    let (term_cols, term_rows) = terminal::size().unwrap_or((80, 24));
    let (mut cols, mut rows) = match (fixed_dim, mode) {
        (Some((w, h)), _) => (w, h),
        (None, InteractiveMode::Fullscreen) => (term_cols, term_rows),
        (None, InteractiveMode::Inline { target_height }) => {
            (term_cols, target_height.min(term_rows))
        }
    };

    opts.raster_opts = opts
        .raster_opts
        .with_aspect_ratio(RasterizeOptions::physical_aspect_ratio(cols, rows));
    apply_viewport_compact_profile(&mut sim, cols, rows, &opts);

    let (v_width, v_height) = framebuffer_dimensions(cols, rows, &opts.renderer_type);
    let mut fb = VirtualFramebuffer::new(v_width, v_height, palette.background);

    let cursor_mode = if matches!(mode, InteractiveMode::Inline { .. }) {
        CursorMode::Inline
    } else {
        CursorMode::Absolute
    };

    let mut renderer: Box<dyn Renderer> = match opts.renderer_type.as_str() {
        "block" => Box::new(BlockRenderer::with_cursor_mode(cursor_mode)),
        "braille" => Box::new(BrailleRenderer::with_cursor_mode(cursor_mode)),
        _ => Box::new(HalfBlockRenderer::with_cursor_mode(cursor_mode)),
    };

    let mut mouse_tracker = MouseTracker::new();
    let target_frame_duration = Duration::from_secs_f32(1.0 / opts.fps as f32);
    let mut last_instant = Instant::now();
    let mut paused = false;
    let mut hud = HudState::new(opts.initial_hud_note.take());

    let loop_result = (|| -> Result<()> {
        loop {
            if shutdown_flag.load(Ordering::Relaxed) {
                return Ok(());
            }

            let now = Instant::now();
            let delta = now.duration_since(last_instant);
            last_instant = now;

            while event::poll(Duration::from_millis(0))? {
                match event::read()? {
                    Event::Key(key) => match map_key_event_with_ripple(key, opts.keyboard_ripple) {
                        Action::Quit => return Ok(()),
                        Action::TogglePause => {
                            paused = !paused;
                            hud.trigger(if paused {
                                "Paused (Ctrl+p resume)"
                            } else {
                                "Resumed"
                            });
                        }
                        Action::ToggleHud => {
                            hud.trigger("Ctrl+p pause · Ctrl+r reset · +/- speed · Esc quit");
                        }
                        Action::SpeedUp => {
                            sim.nudge_speed(0.25);
                            hud.trigger(format!("Speed {:.2}x", sim.speed_scale()));
                        }
                        Action::SlowDown => {
                            sim.nudge_speed(-0.25);
                            hud.trigger(format!("Speed {:.2}x", sim.speed_scale()));
                        }
                        Action::Reset => {
                            sim.reset(DEFAULT_SEED);
                            mouse_tracker.reset();
                            hud.trigger("Reset");
                        }
                        Action::Ripple(intensity) => {
                            if let KeyCode::Char(ch) = key.code {
                                let (x, y) = ripple_origin_for_char(ch);
                                sim.apply_interaction(&Interaction::RippleAt { x, y, intensity });
                            } else {
                                sim.apply_interaction(&Interaction::Ripple { intensity });
                            }
                        }
                        Action::None => {}
                    },
                    Event::Mouse(mouse_event) if opts.mouse_enabled => {
                        if let Some(interaction) = mouse_tracker.handle_event(
                            mouse_event,
                            cols,
                            rows,
                            opts.shockwave_force,
                            opts.stir_force,
                        ) {
                            sim.apply_interaction(&interaction);
                        }
                    }
                    Event::Resize(new_cols, new_rows) if fixed_dim.is_none() => {
                        cols = new_cols;
                        rows = match mode {
                            InteractiveMode::Fullscreen => new_rows,
                            InteractiveMode::Inline { target_height } => {
                                target_height.min(new_rows)
                            }
                        };
                        let (new_w, new_h) =
                            framebuffer_dimensions(cols, rows, &opts.renderer_type);
                        fb.resize(new_w, new_h, palette.background);
                        opts.raster_opts = opts
                            .raster_opts
                            .with_aspect_ratio(RasterizeOptions::physical_aspect_ratio(cols, rows));
                        apply_viewport_compact_profile(&mut sim, cols, rows, &opts);
                        mouse_tracker.reset();
                    }
                    _ => {}
                }
            }

            if let Some(ref mut sp) = sys_provider {
                if last_metric_poll.elapsed() >= poll_duration {
                    current_sys_signals = Some(sp.poll_signals());
                    last_metric_poll = Instant::now();
                }
            }

            let active_audio_source = audio_provider.as_ref().map(|ap| ap.source_kind());
            if !opts.allow_audio_fallback && active_audio_source == Some(AudioSourceKind::Synthetic)
            {
                return Err(LavaError::Audio(
                    "Audio capture disconnected and --no-audio-fallback is set".to_string(),
                ));
            }

            if !paused {
                if let Some(ref mut ap) = audio_provider {
                    let audio_sig = ap.poll_signals();
                    sim.step_audio(delta.as_secs_f32(), &audio_sig);
                } else if let Some(ref signals) = current_sys_signals {
                    sim.step_reactive(delta.as_secs_f32(), signals);
                } else {
                    sim.step(delta.as_secs_f32());
                }
            }

            rasterize_simulation(&sim, &mut fb, &palette, &opts.raster_opts);

            // Wrap lava frame + HUD overlay in Synchronized Output (DEC 2026) so the terminal
            // presents both in a single atomic swap with zero HUD strobe/flicker.
            out.write_all(b"\x1b[?2026h")?;
            renderer
                .render(&fb, &mut out)
                .map_err(|e| LavaError::Render(e.to_string()))?;

            let hud_alpha = hud.current_alpha(now);
            render_status_overlay(
                &mut out,
                &fb,
                &palette,
                cols,
                rows,
                is_fullscreen,
                hud_alpha,
                hud.note.as_deref(),
                paused,
                sim.speed_scale(),
                &opts,
                active_audio_source,
            )?;

            out.write_all(b"\x1b[?2026l")?;
            out.flush()?;

            let elapsed = now.elapsed();
            if elapsed < target_frame_duration {
                std::thread::sleep(target_frame_duration - elapsed);
            }
        }
    })();

    // Ensure shell prompt starts cleanly below inline output upon exit.
    if !is_fullscreen {
        let _ = out.write_all(b"\r\n");
    }
    let _ = out.flush();
    drop(out);
    drop(guard);

    loop_result
}

fn main() -> std::process::ExitCode {
    let cli = Cli::parse();

    let cli_audio_requested = cli.audio || cli.audio_loopback || cli.audio_device.is_some();
    if cli.system && cli_audio_requested {
        eprintln!(
            "Configuration/Policy error: Conflict: --system cannot be combined with audio flags (--audio, --audio-loopback, --audio-device)"
        );
        return std::process::ExitCode::FAILURE;
    }

    if cli.list_audio_devices {
        let devices = list_audio_devices();
        println!("Available Audio Capture Devices ({} found):", devices.len());
        for (i, dev) in devices.iter().enumerate() {
            let default_tag = if dev.is_default { " (default)" } else { "" };
            let direction_tag = format!("[{}]", dev.direction);
            println!(
                "  {}. {} {} {}",
                i + 1,
                direction_tag,
                dev.name,
                default_tag
            );
        }
        return std::process::ExitCode::SUCCESS;
    }

    let interactive_config_requested =
        cli.setup || cli.config.as_deref() == Some(Path::new(INTERACTIVE_CONFIG_SENTINEL));
    let custom_config_path = if interactive_config_requested {
        None
    } else {
        cli.config.as_deref()
    };

    let mut config = match load_config(custom_config_path) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("Error loading configuration: {err}");
            return std::process::ExitCode::FAILURE;
        }
    };

    let has_runtime_overrides = cli.renderer.is_some()
        || cli.fps.is_some()
        || cli.blobs.is_some()
        || cli.theme.is_some()
        || cli.compact
        || cli.widget
        || cli.inline
        || cli.snapshot
        || cli.headless
        || cli.width.is_some()
        || cli.height.is_some()
        || cli.system
        || cli_audio_requested;

    let should_open_wizard = interactive_config_requested
        || (!cli.no_setup
            && custom_config_path.is_none()
            && !has_runtime_overrides
            && default_config_path().is_none()
            && stdin().is_terminal()
            && stdout().is_terminal());

    let mut initial_hud_note: Option<String> = None;
    if should_open_wizard {
        if !stdin().is_terminal() || !stdout().is_terminal() {
            eprintln!(
                "Interactive setup requires a TTY. Pass --config <PATH> to load a TOML file."
            );
            return std::process::ExitCode::FAILURE;
        }
        match run_setup_wizard(&config) {
            Ok(WizardOutcome::SavedAndLaunch {
                config: new_cfg,
                path,
            }) => {
                config = new_cfg;
                initial_hud_note = Some(format!(
                    "Saved {} · run 'lavaterm --config' to edit",
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("config.toml")
                ));
            }
            Ok(WizardOutcome::LaunchOnce(new_cfg)) => {
                config = new_cfg;
                initial_hud_note = Some("Run 'lavaterm --config' anytime to save defaults".into());
            }
            Ok(WizardOutcome::Cancelled) => {
                return std::process::ExitCode::SUCCESS;
            }
            Err(err) => {
                eprintln!("Setup wizard error: {err}");
                return std::process::ExitCode::FAILURE;
            }
        }
    }

    let policy_input = PolicyInput {
        cli_fps: cli.fps,
        cli_compact: cli.compact,
        cli_widget: cli.widget,
        cli_inline: cli.inline,
        cli_snapshot: cli.snapshot,
        cli_headless: cli.headless,
        cli_width: cli.width,
        cli_height: cli.height,

        toml_render_fps: config.render.fps,
        toml_widget_fps: config.widget.fps,
        toml_widget_compact: config.widget.compact,
        toml_widget_inline: config.widget.inline,
        toml_widget_width: config.widget.width,
        toml_widget_height: config.widget.height,
        toml_widget_adapt_blobs: config.widget.adapt_blobs,
    };

    let policy = match resolve_policy(&policy_input) {
        Ok(p) => p,
        Err(err) => {
            eprintln!("Configuration/Policy error: {err}");
            return std::process::ExitCode::FAILURE;
        }
    };

    if let Some(blobs) = cli.blobs {
        config.simulation.blobs = blobs;
    }
    if let Some(renderer) = cli.renderer {
        config.render.renderer = renderer;
    }
    if let Some(shockwave_force) = cli.shockwave_force {
        config.interaction.shockwave_force = shockwave_force;
    }
    if let Some(stir_force) = cli.stir_force {
        config.interaction.stir_force = stir_force;
    }
    config.render.fps = policy.target_fps;
    if let Some((w, h)) = policy.explicit_dimensions {
        config.widget.width = Some(w);
        config.widget.height = Some(h);
    }

    if let Err(err) = config.validate() {
        eprintln!("Configuration/Policy error: {err}");
        return std::process::ExitCode::FAILURE;
    }

    let base_blob_count = config.simulation.blobs;
    let renderer_type = config.render.renderer.clone();
    let audio_reactive = cli_audio_requested || (!cli.system && config.audio.enabled);
    let system_reactive = !audio_reactive && (cli.system || config.reactive.enabled);
    let mut audio_config = config.audio.clone();
    audio_config.enabled = audio_reactive;
    if cli.audio_loopback {
        audio_config.loopback = true;
    }
    if let Some(ref dev) = cli.audio_device {
        audio_config.device = Some(dev.clone());
    }
    let poll_interval_ms = config.reactive.poll_interval_ms;
    let threshold = config.simulation.threshold;

    let mouse_enabled = if cli.no_mouse {
        false
    } else {
        config.interaction.mouse
    };
    let keyboard_ripple = if cli.no_ripple {
        false
    } else {
        config.interaction.keyboard_ripple
    };
    let shockwave_force = config.interaction.shockwave_force;
    let stir_force = config.interaction.stir_force;

    let base_physics = PhysicsParams {
        gravity: config.simulation.gravity,
        buoyancy: config.simulation.buoyancy,
        viscosity: config.simulation.viscosity,
        noise: config.simulation.noise,
        thermal_transfer_rate: config.simulation.thermal_transfer_rate,
    };

    let (palette, theme_label) = if let Some(ref theme_spec) = cli.theme {
        match resolve_theme(theme_spec) {
            Ok(p) => (p, theme_spec.clone()),
            Err(e) => {
                eprintln!("Error resolving theme '{theme_spec}': {e}");
                return std::process::ExitCode::FAILURE;
            }
        }
    } else if let Some(ref path) = config.theme.path {
        match load_custom_theme_file(path) {
            Ok(p) => (p, "custom".to_string()),
            Err(e) => {
                eprintln!("Error loading theme file '{}': {e}", path.display());
                return std::process::ExitCode::FAILURE;
            }
        }
    } else if let Some(ref theme_name) = config.theme.name {
        match resolve_theme(theme_name) {
            Ok(p) => (p, theme_name.clone()),
            Err(e) => {
                eprintln!("Error resolving theme '{theme_name}': {e}");
                return std::process::ExitCode::FAILURE;
            }
        }
    } else {
        (config.palette, "lava".to_string())
    };

    let (initial_cols, initial_rows) = match policy.explicit_dimensions {
        Some((w, h)) => (w, h),
        None => terminal::size().unwrap_or((80, 24)),
    };

    let raster_opts = RasterizeOptions::for_renderer(
        &renderer_type,
        threshold,
        config.render.gradient,
        config.render.glow,
        config.render.molten_pool,
    );

    let opts = RuntimeOptions {
        renderer_type: renderer_type.clone(),
        theme_label,
        fps: policy.target_fps,
        raster_opts,
        base_blob_count,
        force_compact: policy.force_compact,
        adapt_blobs: policy.adapt_blobs,
        system_reactive,
        audio_reactive,
        allow_audio_fallback: !cli.no_audio_fallback,
        audio_config,
        poll_interval_ms,
        mouse_enabled,
        keyboard_ripple,
        shockwave_force,
        stir_force,
        initial_hud_note,
    };

    let mut sim = Simulation::new(base_physics, base_blob_count, DEFAULT_SEED);
    apply_viewport_compact_profile(&mut sim, initial_cols, initial_rows, &opts);

    let result = match policy.mode {
        ExecutionMode::Snapshot => {
            match render_snapshot_with_options(
                &mut sim,
                &palette,
                initial_cols,
                initial_rows,
                &renderer_type,
                5,
                &opts.raster_opts,
            ) {
                Ok(snapshot_str) => {
                    print!("{snapshot_str}");
                    let _ = stdout().flush();
                    Ok(())
                }
                Err(e) => Err(e),
            }
        }
        ExecutionMode::Headless => run_headless(&mut sim, &palette, cli.frames, &opts),
        ExecutionMode::Inline => {
            let target_height = policy
                .explicit_dimensions
                .map(|(_, h)| h)
                .or(config.widget.height)
                .unwrap_or(10);
            run_event_loop(
                sim,
                palette,
                opts,
                policy.explicit_dimensions,
                InteractiveMode::Inline { target_height },
            )
        }
        ExecutionMode::Interactive | ExecutionMode::Widget => run_event_loop(
            sim,
            palette,
            opts,
            policy.explicit_dimensions,
            InteractiveMode::Fullscreen,
        ),
    };

    if let Err(e) = result {
        eprintln!("LavaTerm error: {e}");
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inline_hud_renders_with_relative_cursor_save_restore() {
        let palette = ColorPalette::default();
        let fb = VirtualFramebuffer::new(60, 12, palette.background);
        let opts = RuntimeOptions {
            renderer_type: "halfblock".to_string(),
            theme_label: "lava".to_string(),
            fps: 30,
            raster_opts: RasterizeOptions::default(),
            base_blob_count: 12,
            force_compact: false,
            adapt_blobs: true,
            system_reactive: false,
            audio_reactive: false,
            allow_audio_fallback: true,
            audio_config: AudioConfig::default(),
            poll_interval_ms: 500,
            mouse_enabled: true,
            keyboard_ripple: true,
            shockwave_force: 1.0,
            stir_force: 1.0,
            initial_hud_note: None,
        };
        let mut buf = Vec::new();
        render_status_overlay(
            &mut buf,
            &fb,
            &palette,
            60,
            6,
            false,
            1.0,
            Some("Speed 1.25x"),
            false,
            1.25,
            &opts,
            Some(AudioSourceKind::Synthetic),
        )
        .expect("inline overlay renders");
        let rendered = String::from_utf8_lossy(&buf);
        assert!(
            rendered.contains("\x1b7") && rendered.contains("\x1b8"),
            "Inline HUD must save and restore cursor relatively instead of jumping to screen top"
        );
        assert!(
            !rendered.contains("\x1b[1;2H"),
            "Inline HUD must not use absolute fullscreen cursor coordinates"
        );
        assert!(
            rendered.contains('╭') && rendered.contains('╰'),
            "Inline HUD must render rounded border card"
        );
        assert!(rendered.contains("synthetic fallback"));
    }
}
