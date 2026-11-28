//! Interactive keyboard-driven setup wizard for configuring and saving default `config.toml`.

use crate::config::{default_config_write_path, save_config, Config};
use crate::theme::{list_presets, resolve_theme};
use crate::{LavaError, Result};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io::{stdout, Write};
use std::path::PathBuf;

/// Result returned when exiting the interactive setup wizard.
#[derive(Debug, Clone)]
pub enum WizardOutcome {
    /// User chose "Save as Default & Launch": config was persisted to `path`.
    SavedAndLaunch { config: Config, path: PathBuf },
    /// User chose "Launch Once (Don't Save)" or pressed `Esc`.
    LaunchOnce(Config),
    /// User pressed `Ctrl+c` / `Ctrl+q` to abort launch.
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReactiveChoice {
    Ambient,
    AudioLoopback,
    AudioMic,
    System,
}

impl ReactiveChoice {
    fn options() -> &'static [Self] {
        #[cfg(target_os = "windows")]
        {
            &[
                Self::Ambient,
                Self::AudioLoopback,
                Self::AudioMic,
                Self::System,
            ]
        }
        #[cfg(not(target_os = "windows"))]
        {
            &[Self::Ambient, Self::AudioMic, Self::System]
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Ambient => "Ambient (pure lava)",
            Self::AudioLoopback => "Audio - Desktop Loopback (WASAPI)",
            Self::AudioMic => "Audio - Microphone / Input",
            Self::System => "System Telemetry (CPU / RAM / Battery)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisualFxChoice {
    GlowOnly,
    GlowAndPool,
    ClassicFlat,
}

impl VisualFxChoice {
    const ALL: [Self; 3] = [Self::GlowOnly, Self::GlowAndPool, Self::ClassicFlat];

    fn label(self) -> &'static str {
        match self {
            Self::GlowOnly => "Soft Glow (recommended)",
            Self::GlowAndPool => "Glow + Bottom Pool",
            Self::ClassicFlat => "Classic Flat (no glow)",
        }
    }
}

const RENDERERS: [&str; 3] = ["halfblock", "block", "braille"];
const FPS_OPTIONS: [u32; 4] = [24, 30, 45, 60];
const BLOB_OPTIONS: [usize; 5] = [10, 14, 18, 24, 32];
const TOTAL_ROWS: usize = 9; // 7 settings + Save button + Launch-once button

/// Pure keyboard-driven state machine for the interactive configuration wizard.
#[derive(Debug, Clone)]
pub struct WizardState {
    base_config: Config,
    themes: Vec<String>,
    pub selected_row: usize,
    theme_idx: usize,
    renderer_idx: usize,
    mode_idx: usize,
    fps_idx: usize,
    blob_idx: usize,
    fx_idx: usize,
    interactive_fx: bool,
}

impl WizardState {
    /// Initializes wizard fields from an existing `Config` (or built-in defaults).
    pub fn from_config(config: &Config) -> Self {
        let mut themes: Vec<String> = list_presets().into_iter().map(String::from).collect();
        themes.push("auto".to_string());

        let desired_theme = config
            .theme
            .name
            .as_deref()
            .unwrap_or("lava")
            .to_ascii_lowercase();
        let theme_idx = themes.iter().position(|t| t == &desired_theme).unwrap_or(0);

        let renderer_idx = RENDERERS
            .iter()
            .position(|r| *r == config.render.renderer)
            .unwrap_or(0);

        let mode_choice = if config.audio.enabled {
            if config.audio.loopback && cfg!(target_os = "windows") {
                ReactiveChoice::AudioLoopback
            } else {
                ReactiveChoice::AudioMic
            }
        } else if config.reactive.enabled {
            ReactiveChoice::System
        } else {
            ReactiveChoice::Ambient
        };
        let modes = ReactiveChoice::options();
        let mode_idx = modes.iter().position(|m| *m == mode_choice).unwrap_or(0);

        let fps_idx = FPS_OPTIONS
            .iter()
            .enumerate()
            .min_by_key(|(_, &f)| (f as i32 - config.render.fps as i32).abs())
            .map(|(i, _)| i)
            .unwrap_or(1);

        let blob_idx = BLOB_OPTIONS
            .iter()
            .enumerate()
            .min_by_key(|(_, &b)| (b as isize - config.simulation.blobs as isize).abs())
            .map(|(i, _)| i)
            .unwrap_or(3);

        let fx_choice = match (config.render.glow, config.render.molten_pool) {
            (true, false) => VisualFxChoice::GlowOnly,
            (true, true) | (false, true) => VisualFxChoice::GlowAndPool,
            (false, false) => VisualFxChoice::ClassicFlat,
        };
        let fx_idx = VisualFxChoice::ALL
            .iter()
            .position(|f| *f == fx_choice)
            .unwrap_or(0);

        Self {
            base_config: config.clone(),
            themes,
            selected_row: 0,
            theme_idx,
            renderer_idx,
            mode_idx,
            fps_idx,
            blob_idx,
            fx_idx,
            interactive_fx: config.interaction.mouse || config.interaction.keyboard_ripple,
        }
    }

    pub fn move_up(&mut self) {
        self.selected_row = if self.selected_row == 0 {
            TOTAL_ROWS - 1
        } else {
            self.selected_row - 1
        };
    }

    pub fn move_down(&mut self) {
        self.selected_row = (self.selected_row + 1) % TOTAL_ROWS;
    }

    pub fn cycle(&mut self, dir: isize) {
        let step = |idx: usize, len: usize| -> usize {
            ((idx as isize + dir).rem_euclid(len as isize)) as usize
        };
        match self.selected_row {
            0 => self.theme_idx = step(self.theme_idx, self.themes.len()),
            1 => self.renderer_idx = step(self.renderer_idx, RENDERERS.len()),
            2 => self.mode_idx = step(self.mode_idx, ReactiveChoice::options().len()),
            3 => self.fps_idx = step(self.fps_idx, FPS_OPTIONS.len()),
            4 => self.blob_idx = step(self.blob_idx, BLOB_OPTIONS.len()),
            5 => self.fx_idx = step(self.fx_idx, VisualFxChoice::ALL.len()),
            6 => self.interactive_fx = !self.interactive_fx,
            _ => {}
        }
    }

    pub fn current_theme(&self) -> &str {
        &self.themes[self.theme_idx]
    }

    /// Plain-English description of the currently highlighted wizard row.
    pub fn selected_description(&self) -> &'static str {
        match self.selected_row {
            0 => "Color palette for hot base, mid flow, cool top, and background.",
            1 => "halfblock (smooth 2x vertical), block (chunky), braille (crisp 2x4 dots).",
            2 => "Ambient = calm lava; Audio = reacts to music; System = CPU/RAM/battery.",
            3 => "Refresh rate: 30 fps is balanced; 60 fps is ultra-smooth.",
            4 => "Number of metaball droplets floating and merging in the lamp.",
            5 => "Glow adds soft aura & furnace light; Molten Pool adds heated base wave.",
            6 => "Enable mouse click/drag stirring and visible ripples when typing.",
            7 => "Write settings to config.toml and start LavaTerm immediately.",
            _ => "Start LavaTerm with these settings without overwriting config.toml.",
        }
    }

    /// Produces a validated `Config` reflecting all wizard selections.
    pub fn to_config(&self) -> Config {
        let mut cfg = self.base_config.clone();
        cfg.theme.name = Some(self.themes[self.theme_idx].clone());
        cfg.theme.path = None;
        cfg.render.renderer = RENDERERS[self.renderer_idx].to_string();
        cfg.render.fps = FPS_OPTIONS[self.fps_idx];
        cfg.simulation.blobs = BLOB_OPTIONS[self.blob_idx];

        match VisualFxChoice::ALL[self.fx_idx] {
            VisualFxChoice::GlowOnly => {
                cfg.render.glow = true;
                cfg.render.molten_pool = false;
            }
            VisualFxChoice::GlowAndPool => {
                cfg.render.glow = true;
                cfg.render.molten_pool = true;
            }
            VisualFxChoice::ClassicFlat => {
                cfg.render.glow = false;
                cfg.render.molten_pool = false;
            }
        }

        match ReactiveChoice::options()[self.mode_idx] {
            ReactiveChoice::Ambient => {
                cfg.audio.enabled = false;
                cfg.audio.loopback = false;
                cfg.reactive.enabled = false;
            }
            ReactiveChoice::AudioLoopback => {
                cfg.audio.enabled = true;
                cfg.audio.loopback = true;
                cfg.reactive.enabled = false;
            }
            ReactiveChoice::AudioMic => {
                cfg.audio.enabled = true;
                cfg.audio.loopback = false;
                cfg.reactive.enabled = false;
            }
            ReactiveChoice::System => {
                cfg.audio.enabled = false;
                cfg.audio.loopback = false;
                cfg.reactive.enabled = true;
            }
        }

        cfg.interaction.mouse = self.interactive_fx;
        cfg.interaction.keyboard_ripple = self.interactive_fx;
        cfg
    }
}

struct WizardTerminalGuard;

impl Drop for WizardTerminalGuard {
    fn drop(&mut self) {
        let mut out = stdout();
        let _ = execute!(out, LeaveAlternateScreen, cursor::Show);
        let _ = out.flush();
        let _ = terminal::disable_raw_mode();
    }
}

fn draw_wizard(
    out: &mut impl Write,
    frame: &mut Vec<u8>,
    state: &WizardState,
    save_path: Option<&PathBuf>,
    error_msg: Option<&str>,
    clear_screen: bool,
) -> std::io::Result<()> {
    let (cols, rows) = terminal::size().unwrap_or((80, 24));
    let box_w: usize = 70.min(cols.saturating_sub(2) as usize).max(46);
    let inner_w = box_w - 2;
    let box_h: usize = 20;
    let start_col = (cols as usize).saturating_sub(box_w) / 2 + 1;
    let right_col = start_col + box_w - 1;
    let start_row = (rows as usize).saturating_sub(box_h) / 2 + 1;

    let palette = resolve_theme(state.current_theme()).unwrap_or_default();
    let accent = palette.middle;
    let hot = palette.bottom;
    let cool = palette.top;

    frame.clear();
    // Synchronized Output (DEC 2026) prevents mid-frame tearing/flicker on modern terminals.
    frame.extend_from_slice(b"\x1b[?2026h");
    if clear_screen {
        frame.extend_from_slice(b"\x1b[2J\x1b[H");
    }

    let mut cur_row = start_row;
    let horiz = "─".repeat(inner_w);
    // Top border
    write!(
        frame,
        "\x1b[{};{}H\x1b[48;2;14;14;22m\x1b[38;2;{};{};{}m╭{}╮\x1b[0m",
        cur_row, start_col, accent.r, accent.g, accent.b, horiz
    )?;
    cur_row += 1;

    // Header
    let title = "ZenLavaTerm Setup - Default Configuration";
    let title_trunc: String = title.chars().take(inner_w.saturating_sub(2)).collect();
    let pad_title = inner_w.saturating_sub(title_trunc.chars().count() + 2);
    write!(
        frame,
        "\x1b[{};{}H\x1b[48;2;14;14;22m\x1b[38;2;{};{};{}m│\x1b[1m\x1b[38;2;245;245;255m  {}{}\x1b[0m\x1b[{};{}H\x1b[48;2;14;14;22m\x1b[38;2;{};{};{}m│\x1b[0m",
        cur_row,
        start_col,
        accent.r,
        accent.g,
        accent.b,
        title_trunc,
        " ".repeat(pad_title),
        cur_row,
        right_col,
        accent.r,
        accent.g,
        accent.b
    )?;
    cur_row += 1;

    // Divider
    write!(
        frame,
        "\x1b[{};{}H\x1b[48;2;14;14;22m\x1b[38;2;65;68;88m├{}┤\x1b[0m",
        cur_row, start_col, horiz
    )?;
    cur_row += 1;

    let labels_and_vals: [(&str, String); 7] = [
        ("Theme", state.current_theme().to_string()),
        ("Renderer", RENDERERS[state.renderer_idx].to_string()),
        (
            "Reactive Mode",
            ReactiveChoice::options()[state.mode_idx]
                .label()
                .to_string(),
        ),
        ("Target FPS", format!("{} fps", FPS_OPTIONS[state.fps_idx])),
        (
            "Blob Count",
            format!("{} blobs", BLOB_OPTIONS[state.blob_idx]),
        ),
        (
            "Visual Effects",
            VisualFxChoice::ALL[state.fx_idx].label().to_string(),
        ),
        (
            "Mouse & Typing Ripples",
            if state.interactive_fx {
                "Enabled".to_string()
            } else {
                "Disabled".to_string()
            },
        ),
    ];

    for (idx, (label, val)) in labels_and_vals.iter().enumerate() {
        let active = state.selected_row == idx;
        let bg = if active {
            "\x1b[48;2;32;34;52m"
        } else {
            "\x1b[48;2;14;14;22m"
        };
        let marker = if active { ">" } else { " " };
        let val_display = if active {
            format!("< {val} >")
        } else {
            format!("  {val}  ")
        };

        // Add live 3-block swatch on Theme row
        let swatch_plain_len = if idx == 0 { 5 } else { 0 };
        let line_plain = format!(" {marker} {:<22} {val_display}", label);
        let max_text = inner_w.saturating_sub(swatch_plain_len);
        let truncated: String = line_plain.chars().take(max_text).collect();
        let pad = inner_w.saturating_sub(truncated.chars().count() + swatch_plain_len);

        write!(
            frame,
            "\x1b[{};{}H\x1b[48;2;14;14;22m\x1b[38;2;{};{};{}m│{bg}",
            cur_row, start_col, accent.r, accent.g, accent.b
        )?;
        if active {
            write!(frame, "\x1b[1m\x1b[38;2;255;255;255m{truncated}\x1b[22m")?;
        } else {
            write!(frame, "\x1b[38;2;195;200;220m{truncated}")?;
        }
        if idx == 0 {
            write!(
                frame,
                " \x1b[38;2;{};{};{}m█\x1b[38;2;{};{};{}m█\x1b[38;2;{};{};{}m█ ",
                hot.r, hot.g, hot.b, accent.r, accent.g, accent.b, cool.r, cool.g, cool.b
            )?;
        }
        write!(
            frame,
            "{}\x1b[{};{}H\x1b[48;2;14;14;22m\x1b[38;2;{};{};{}m│\x1b[0m",
            " ".repeat(pad),
            cur_row,
            right_col,
            accent.r,
            accent.g,
            accent.b
        )?;
        cur_row += 1;
    }

    // Divider before description
    write!(
        frame,
        "\x1b[{};{}H\x1b[48;2;14;14;22m\x1b[38;2;65;68;88m├{}┤\x1b[0m",
        cur_row, start_col, horiz
    )?;
    cur_row += 1;

    // Live explanation line for the currently highlighted option
    let desc_line = format!("  {}", state.selected_description());
    let desc_trunc: String = desc_line.chars().take(inner_w).collect();
    let desc_pad = inner_w.saturating_sub(desc_trunc.chars().count());
    write!(
        frame,
        "\x1b[{};{}H\x1b[48;2;14;14;22m\x1b[38;2;{};{};{}m│\x1b[38;2;165;210;255m{desc_trunc}{}\x1b[{};{}H\x1b[48;2;14;14;22m\x1b[38;2;{};{};{}m│\x1b[0m",
        cur_row,
        start_col,
        accent.r,
        accent.g,
        accent.b,
        " ".repeat(desc_pad),
        cur_row,
        right_col,
        accent.r,
        accent.g,
        accent.b
    )?;
    cur_row += 1;

    // Blank spacer
    write!(
        frame,
        "\x1b[{};{}H\x1b[48;2;14;14;22m\x1b[38;2;{};{};{}m│{}│\x1b[0m",
        cur_row,
        start_col,
        accent.r,
        accent.g,
        accent.b,
        " ".repeat(inner_w)
    )?;
    cur_row += 1;

    // Action rows (7: Save as Default & Launch, 8: Launch Once)
    let action_rows = [
        (7usize, "Save as Default & Launch"),
        (8usize, "Launch Once (Don't Save)"),
    ];
    for (row_id, text) in action_rows {
        let active = state.selected_row == row_id;
        let btn = if active {
            format!("  > [ {text} ]")
        } else {
            format!("    [ {text} ]")
        };
        let btn_trunc: String = btn.chars().take(inner_w).collect();
        let pad = inner_w.saturating_sub(btn_trunc.chars().count());
        let style = if active && row_id == 7 {
            format!(
                "\x1b[48;2;{};{};{}m\x1b[1m\x1b[38;2;14;14;22m",
                accent.r, accent.g, accent.b
            )
        } else if active {
            "\x1b[48;2;46;49;72m\x1b[1m\x1b[38;2;255;255;255m".to_string()
        } else {
            "\x1b[48;2;14;14;22m\x1b[38;2;165;170;192m".to_string()
        };
        write!(
            frame,
            "\x1b[{};{}H\x1b[48;2;14;14;22m\x1b[38;2;{};{};{}m│{style}{btn_trunc}{}\x1b[0m\x1b[{};{}H\x1b[48;2;14;14;22m\x1b[38;2;{};{};{}m│\x1b[0m",
            cur_row,
            start_col,
            accent.r,
            accent.g,
            accent.b,
            " ".repeat(pad),
            cur_row,
            right_col,
            accent.r,
            accent.g,
            accent.b
        )?;
        cur_row += 1;
    }

    // Divider
    write!(
        frame,
        "\x1b[{};{}H\x1b[48;2;14;14;22m\x1b[38;2;65;68;88m├{}┤\x1b[0m",
        cur_row, start_col, horiz
    )?;
    cur_row += 1;

    // Footer lines: controls + --config reminder + path/error
    let path_str = save_path
        .map(|p| format!("File: {}", p.display()))
        .unwrap_or_else(|| "File: (no default config directory detected)".to_string());
    let footer_lines = [
        " Up/Down Select  Left/Right Change  Enter Next/Confirm  Esc Skip".to_string(),
        " Tip: Run 'lavaterm --config' anytime to edit defaults.".to_string(),
        if let Some(err) = error_msg {
            format!(" Error: {err}")
        } else {
            format!(" {path_str}")
        },
    ];
    for (i, line) in footer_lines.iter().enumerate() {
        let truncated: String = line.chars().take(inner_w).collect();
        let pad = inner_w.saturating_sub(truncated.chars().count());
        let color = if i == 2 && error_msg.is_some() {
            "\x1b[38;2;255;110;90m"
        } else if i == 1 {
            "\x1b[38;2;255;196;110m"
        } else {
            "\x1b[38;2;140;146;172m"
        };
        write!(
            frame,
            "\x1b[{};{}H\x1b[48;2;14;14;22m\x1b[38;2;{};{};{}m│{color}{truncated}{}\x1b[{};{}H\x1b[48;2;14;14;22m\x1b[38;2;{};{};{}m│\x1b[0m",
            cur_row,
            start_col,
            accent.r,
            accent.g,
            accent.b,
            " ".repeat(pad),
            cur_row,
            right_col,
            accent.r,
            accent.g,
            accent.b
        )?;
        cur_row += 1;
    }

    // Bottom border
    write!(
        frame,
        "\x1b[{};{}H\x1b[48;2;14;14;22m\x1b[38;2;{};{};{}m╰{}╯\x1b[0m",
        cur_row, start_col, accent.r, accent.g, accent.b, horiz
    )?;

    frame.extend_from_slice(b"\x1b[?2026l");
    out.write_all(frame)?;
    out.flush()
}

/// Runs the interactive keyboard-driven setup wizard in the terminal and returns the chosen outcome.
pub fn run_setup_wizard(initial_config: &Config) -> Result<WizardOutcome> {
    terminal::enable_raw_mode()?;
    let _guard = WizardTerminalGuard;
    let mut out = stdout();
    execute!(
        out,
        EnterAlternateScreen,
        cursor::Hide,
        Clear(ClearType::All)
    )?;

    let mut state = WizardState::from_config(initial_config);
    let save_path = default_config_write_path();
    let mut error_msg: Option<String> = None;
    let mut frame_buf: Vec<u8> = Vec::with_capacity(8 * 1024);
    let mut needs_clear = true;
    let mut needs_redraw = true;

    loop {
        if needs_redraw {
            draw_wizard(
                &mut out,
                &mut frame_buf,
                &state,
                save_path.as_ref(),
                error_msg.as_deref(),
                needs_clear,
            )?;
            needs_clear = false;
            needs_redraw = false;
        }

        match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => {
                error_msg = None;
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(
                        key.code,
                        KeyCode::Char('c')
                            | KeyCode::Char('C')
                            | KeyCode::Char('q')
                            | KeyCode::Char('Q')
                    )
                {
                    return Ok(WizardOutcome::Cancelled);
                }

                match key.code {
                    KeyCode::Esc => return Ok(WizardOutcome::LaunchOnce(state.to_config())),
                    KeyCode::Up | KeyCode::Char('k') => {
                        state.move_up();
                        needs_redraw = true;
                    }
                    KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                        state.move_down();
                        needs_redraw = true;
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        state.cycle(-1);
                        needs_redraw = true;
                    }
                    KeyCode::Right | KeyCode::Char('l') | KeyCode::Char(' ') => {
                        state.cycle(1);
                        needs_redraw = true;
                    }
                    KeyCode::Enter => match state.selected_row {
                        7 => {
                            let cfg = state.to_config();
                            match save_path.clone() {
                                Some(path) => match save_config(&path, &cfg) {
                                    Ok(()) => {
                                        return Ok(WizardOutcome::SavedAndLaunch {
                                            config: cfg,
                                            path,
                                        });
                                    }
                                    Err(err) => {
                                        error_msg = Some(err.to_string());
                                        needs_redraw = true;
                                    }
                                },
                                None => {
                                    return Err(LavaError::Config(
                                        "Could not determine a default config directory on this system"
                                            .to_string(),
                                    ));
                                }
                            }
                        }
                        8 => return Ok(WizardOutcome::LaunchOnce(state.to_config())),
                        _ => {
                            // On setting rows (0..=6), Enter confirms the row and advances downward
                            // instead of unexpectedly saving and exiting the wizard.
                            state.move_down();
                            needs_redraw = true;
                        }
                    },
                    _ => {}
                }
            }
            Event::Resize(_, _) => {
                needs_clear = true;
                needs_redraw = true;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wizard_state_cycles_and_builds_valid_config() {
        let mut state = WizardState::from_config(&Config::default());
        assert_eq!(state.current_theme(), "lava");

        // Cycle theme -> ocean
        state.cycle(1);
        assert_eq!(state.current_theme(), "ocean");

        // Move to renderer -> braille
        state.move_down();
        state.cycle(-1);
        assert_eq!(state.to_config().render.renderer, "braille");

        // Move to reactive mode -> next option
        state.move_down();
        state.cycle(1);
        let cfg = state.to_config();
        assert!(cfg.validate().is_ok());
        assert!(cfg.audio.enabled || cfg.reactive.enabled);
    }

    #[test]
    fn test_wizard_visual_fx_and_interaction_toggles() {
        let mut state = WizardState::from_config(&Config::default());
        state.selected_row = 5; // Visual Effects (default: GlowOnly)
        let cfg_default = state.to_config();
        assert!(cfg_default.render.glow);
        assert!(!cfg_default.render.molten_pool);

        state.cycle(1); // GlowAndPool
        let cfg = state.to_config();
        assert!(cfg.render.glow);
        assert!(cfg.render.molten_pool);

        state.cycle(1); // ClassicFlat
        let cfg2 = state.to_config();
        assert!(!cfg2.render.glow);
        assert!(!cfg2.render.molten_pool);

        state.selected_row = 6; // Mouse & Typing Ripples
        state.cycle(1);
        let cfg3 = state.to_config();
        assert!(!cfg3.interaction.mouse);
        assert!(!cfg3.interaction.keyboard_ripple);
    }
}
