//! Keyboard event mapping and domain actions.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

/// High-level domain action triggered by user input.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Action {
    /// Exit the application cleanly.
    Quit,
    /// Pause or resume simulation.
    TogglePause,
    /// Toggle the single-line bottom status HUD (`Ctrl+i` or `Tab`).
    ToggleHud,
    /// Increase simulation speed / buoyancy.
    SpeedUp,
    /// Decrease simulation speed / buoyancy.
    SlowDown,
    /// Reset simulation state.
    Reset,
    /// Acoustic ripple / perturbation injected into the lava.
    Ripple(f32),
    /// No actionable command.
    None,
}

/// Translates a crossterm `KeyEvent` into a domain `Action` with optional keyboard ripple.
pub fn map_key_event_with_ripple(key: KeyEvent, enable_ripple: bool) -> Action {
    if key.kind == KeyEventKind::Release {
        return Action::None;
    }

    // 1. Control modifier checks
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        return match key.code {
            KeyCode::Char('c') | KeyCode::Char('C') => Action::Quit,
            KeyCode::Char('i') | KeyCode::Char('I') => Action::ToggleHud,
            _ => Action::None,
        };
    }

    // 2. Alt modifier checks
    if key.modifiers.contains(KeyModifiers::ALT) {
        return Action::None;
    }

    // 3. Command keys and non-command ripple triggers
    // Note: in standard terminals without Kitty keyboard enhancement, Ctrl+i sends ASCII 0x09 (Tab).
    match key.code {
        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => Action::Quit,
        KeyCode::Char(' ') | KeyCode::Char('p') | KeyCode::Char('P') => Action::TogglePause,
        KeyCode::Tab => Action::ToggleHud,
        KeyCode::Char('+') | KeyCode::Char('=') | KeyCode::Up | KeyCode::Right => Action::SpeedUp,
        KeyCode::Char('-') | KeyCode::Char('_') | KeyCode::Down | KeyCode::Left => Action::SlowDown,
        KeyCode::Char('r') | KeyCode::Char('R') => Action::Reset,
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
            map_key_event(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE)),
            Action::Quit
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Action::Quit
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE)),
            Action::TogglePause
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE)),
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
    fn test_ctrl_i_and_tab_toggle_hud_while_plain_i_ripples() {
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
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE)),
            Action::Ripple(1.0)
        );
    }

    #[test]
    fn test_keyboard_ripple_action() {
        let a_event = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(map_key_event(a_event), Action::Ripple(1.0));
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('7'), KeyModifiers::NONE)),
            Action::Ripple(1.0)
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('!'), KeyModifiers::NONE)),
            Action::Ripple(1.0)
        );
        assert_eq!(map_key_event_with_ripple(a_event, false), Action::None);
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
        let mut q_release = KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE);
        q_release.kind = KeyEventKind::Release;
        assert_eq!(map_key_event(q_release), Action::None);

        let mut tab_release = KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE);
        tab_release.kind = KeyEventKind::Release;
        assert_eq!(map_key_event(tab_release), Action::None);
    }
}
