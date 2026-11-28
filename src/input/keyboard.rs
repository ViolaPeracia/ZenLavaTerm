//! Keyboard event mapping and domain actions.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// High-level domain action triggered by user input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    /// Exit the application cleanly (`Ctrl+q`, `Ctrl+c`, or `Esc`).
    Quit,
    /// Pause or resume simulation (`Ctrl+p`).
    TogglePause,
    /// Show the fading top-left status HUD (`Ctrl+i` or `Tab`).
    ToggleHud,
    /// Increase simulation speed (`+`, `=`, `Up`, `Right`).
    SpeedUp,
    /// Decrease simulation speed (`-`, `_`, `Down`, `Left`).
    SlowDown,
    /// Reset simulation state (`Ctrl+r`).
    Reset,
    /// Cycle to the next built-in theme preset with smooth color cross-fade (`Ctrl+t`).
    CycleTheme,
    /// Cycle to the next output renderer backend (`Ctrl+o`).
    CycleRenderer,
    /// Toggle ambient soft edge glow (`Ctrl+g`).
    ToggleGlow,
    /// Acoustic ripple / perturbation injected into the lava.
    Ripple(f32),
    /// No actionable command.
    None,
}

/// Maps a typed character to a normalized `(x, y)` chamber origin using a QWERTY spatial layout
/// so typing across the keyboard spawns ripples moving naturally across the lava lamp.
pub fn ripple_origin_for_char(ch: char) -> (f32, f32) {
    const ROWS: [&str; 4] = ["`1234567890", "qwertyuiop[]\\", "asdfghjkl;'", "zxcvbnm,./"];
    let lower = ch.to_ascii_lowercase();
    if lower == ' ' {
        return (0.50, 0.22);
    }
    for (row_idx, row) in ROWS.iter().enumerate() {
        if let Some(col_idx) = row.find(lower) {
            let x = 0.12 + 0.76 * (col_idx as f32 / (row.len().saturating_sub(1).max(1)) as f32);
            let y = 0.78 - 0.16 * (row_idx as f32);
            return (x, y);
        }
    }
    // Deterministic fallback spread for Unicode/symbol keys
    let code = ch as u32;
    let x = 0.15 + 0.70 * ((code.wrapping_mul(37) % 100) as f32 / 100.0);
    let y = 0.22 + 0.56 * ((code.wrapping_mul(73) % 100) as f32 / 100.0);
    (x, y)
}

/// Translates a crossterm `KeyEvent` into a domain `Action` with optional keyboard ripple.
///
/// Letter commands use `Ctrl+<key>` (`Ctrl+p` pause, `Ctrl+r` reset, `Ctrl+i` HUD,
/// `Ctrl+t` theme, `Ctrl+o` output renderer, `Ctrl+g` glow, `Ctrl+q`/`Ctrl+c` quit)
/// so typing any normal letter or spacebar creates ripples without accidentally pausing or quitting.
/// `Esc` remains an instant quit shortcut.
pub fn map_key_event_with_ripple(key: KeyEvent, enable_ripple: bool) -> Action {
    if key.kind == KeyEventKind::Release {
        return Action::None;
    }

    // 1. Control modifier checks (all letter commands live here)
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Char('q') | KeyCode::Char('Q') => {
                Action::Quit
            }
            KeyCode::Char('p') | KeyCode::Char('P') => Action::TogglePause,
            KeyCode::Char('r') | KeyCode::Char('R') => Action::Reset,
            KeyCode::Char('i') | KeyCode::Char('I') => Action::ToggleHud,
            KeyCode::Char('t') | KeyCode::Char('T') => Action::CycleTheme,
            KeyCode::Char('o') | KeyCode::Char('O') => Action::CycleRenderer,
            KeyCode::Char('g') | KeyCode::Char('G') => Action::ToggleGlow,
            _ => Action::None,
        };
    }

    // 2. Alt modifier checks
    if key.modifiers.contains(KeyModifiers::ALT) {
        return Action::None;
    }

    // 3. Non-letter command keys and full-keyboard typing ripples
    // Note: in standard terminals without Kitty keyboard enhancement, Ctrl+i sends ASCII 0x09 (Tab).
    match key.code {
        KeyCode::Esc => Action::Quit,
        KeyCode::Tab => Action::ToggleHud,
        KeyCode::Char('+') | KeyCode::Char('=') | KeyCode::Up | KeyCode::Right => Action::SpeedUp,
        KeyCode::Char('-') | KeyCode::Char('_') | KeyCode::Down | KeyCode::Left => Action::SlowDown,
        KeyCode::Char(_) if enable_ripple => Action::Ripple(1.0),
        _ => Action::None,
    }
}

/// Translates a crossterm `KeyEvent` into a domain `Action` (with keyboard ripple enabled).
pub fn map_key_event(key: KeyEvent) -> Action {
    map_key_event_with_ripple(key, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_mapping() {
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL)),
            Action::Quit
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Action::Quit
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL)),
            Action::TogglePause
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::CONTROL)),
            Action::Reset
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE)),
            Action::SpeedUp
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('-'), KeyModifiers::NONE)),
            Action::SlowDown
        );
    }

    #[test]
    fn test_plain_letters_and_space_trigger_ripple_instead_of_commands() {
        for ch in ['q', 'p', 'r', 'i', 't', 'o', 'g', ' ', 'a', '7', '!'] {
            assert_eq!(
                map_key_event(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE)),
                Action::Ripple(1.0),
                "Plain '{ch}' should trigger Ripple when ripple is enabled"
            );
            assert_eq!(
                map_key_event_with_ripple(
                    KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE),
                    false
                ),
                Action::None,
                "Plain '{ch}' should be ignored when ripple is disabled"
            );
        }
    }

    #[test]
    fn test_live_visual_hotkeys() {
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('t'), KeyModifiers::CONTROL)),
            Action::CycleTheme
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('T'), KeyModifiers::CONTROL)),
            Action::CycleTheme
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL)),
            Action::CycleRenderer
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::CONTROL)),
            Action::ToggleGlow
        );
    }

    #[test]
    fn test_ctrl_i_and_tab_toggle_hud() {
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::CONTROL)),
            Action::ToggleHud
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('I'), KeyModifiers::CONTROL)),
            Action::ToggleHud
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            Action::ToggleHud
        );
    }

    #[test]
    fn test_modifier_keys_do_not_produce_ripples() {
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Action::Quit
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::CONTROL)),
            Action::None
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::CONTROL)),
            Action::None
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('a'), KeyModifiers::ALT)),
            Action::None
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT)),
            Action::None
        );
    }

    #[test]
    fn test_key_release_events_are_ignored() {
        let mut q_release = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL);
        q_release.kind = KeyEventKind::Release;
        assert_eq!(map_key_event(q_release), Action::None);

        let mut tab_release = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        tab_release.kind = KeyEventKind::Release;
        assert_eq!(map_key_event(tab_release), Action::None);
    }

    #[test]
    fn test_ripple_origin_for_char_spreads_horizontally() {
        let (left_x, _) = ripple_origin_for_char('a');
        let (right_x, _) = ripple_origin_for_char('l');
        assert!(left_x < right_x);
        let (space_x, space_y) = ripple_origin_for_char(' ');
        assert!((space_x - 0.5).abs() < 1e-4 && space_y < 0.3);
    }
}
