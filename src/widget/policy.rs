//! Widget execution policy, mode resolution, and conflict validation.

use crate::{LavaError, Result};

/// Execution mode for the LavaTerm application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Standard full-screen interactive loop in alternate screen.
    Interactive,
    /// Low-overhead ambient widget loop with reduced default FPS and compact scaling.
    Widget,
    /// In-place interactive loop without entering the alternate screen.
    Inline,
    /// Single-shot frame serialization directly to stdout for status bars.
    Snapshot,
    /// Headless non-interactive simulation stepping for testing and CI.
    Headless,
}

/// Raw input parameters from CLI flags and TOML configuration.
#[derive(Debug, Clone, Default)]
pub struct PolicyInput {
    pub cli_fps: Option<u32>,
    pub cli_compact: bool,
    pub cli_widget: bool,
    pub cli_inline: bool,
    pub cli_snapshot: bool,
    pub cli_headless: bool,
    pub cli_width: Option<u16>,
    pub cli_height: Option<u16>,

    pub toml_render_fps: u32,
    pub toml_widget_fps: u32,
    pub toml_widget_compact: bool,
    pub toml_widget_inline: bool,
    pub toml_widget_width: Option<u16>,
    pub toml_widget_height: Option<u16>,
    pub toml_widget_adapt_blobs: bool,
}

/// Resolved runtime policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPolicy {
    pub mode: ExecutionMode,
    pub target_fps: u32,
    pub force_compact: bool,
    pub explicit_dimensions: Option<(u16, u16)>,
    pub adapt_blobs: bool,
}

/// Resolves raw inputs into a validated `ResolvedPolicy` following strict precedence rules.
///
/// Precedence: CLI Arguments > Mode-scoped TOML Configuration > Built-in Defaults.
/// `[widget]` TOML settings only take effect in widget/inline/snapshot modes so configuring
/// widget defaults never hijacks a normal interactive `lavaterm` session or breaks resize.
pub fn resolve_policy(input: &PolicyInput) -> Result<ResolvedPolicy> {
    if input.cli_snapshot && (input.cli_inline || input.cli_headless) {
        return Err(LavaError::Config(
            "Conflict: --snapshot cannot be combined with --inline or --headless".to_string(),
        ));
    }
    if input.cli_inline && input.cli_headless {
        return Err(LavaError::Config(
            "Conflict: --inline cannot be combined with --headless".to_string(),
        ));
    }

    let in_widget_mode = input.cli_widget || input.cli_inline || input.cli_snapshot;

    let mode = if input.cli_snapshot {
        ExecutionMode::Snapshot
    } else if input.cli_headless {
        ExecutionMode::Headless
    } else if input.cli_inline || (input.cli_widget && input.toml_widget_inline) {
        ExecutionMode::Inline
    } else if input.cli_widget {
        ExecutionMode::Widget
    } else {
        ExecutionMode::Interactive
    };

    if let Some(0) = input.cli_fps {
        return Err(LavaError::Config(
            "Target FPS must be greater than zero".to_string(),
        ));
    }
    let target_fps = if let Some(fps) = input.cli_fps {
        fps
    } else if input.cli_widget {
        if input.toml_widget_fps > 0 {
            input.toml_widget_fps
        } else {
            15
        }
    } else if input.toml_render_fps > 0 {
        input.toml_render_fps
    } else {
        30
    };

    if target_fps == 0 {
        return Err(LavaError::Config(
            "Target FPS must be greater than zero".to_string(),
        ));
    }
    if target_fps > 240 {
        return Err(LavaError::Config(
            "Target FPS must be between 1 and 240".to_string(),
        ));
    }

    let force_compact =
        input.cli_compact || input.cli_widget || (in_widget_mode && input.toml_widget_compact);

    let (width, height) = if in_widget_mode {
        (
            input.cli_width.or(input.toml_widget_width),
            input.cli_height.or(input.toml_widget_height),
        )
    } else {
        (input.cli_width, input.cli_height)
    };

    let explicit_dimensions = match (width, height) {
        (Some(w), Some(h)) => {
            if w == 0 || h == 0 {
                return Err(LavaError::Config(
                    "Explicit dimensions (width/height) must be greater than zero".to_string(),
                ));
            }
            if w > 1000 || h > 1000 {
                return Err(LavaError::Config(
                    "Explicit dimensions (width/height) must be between 1 and 1000".to_string(),
                ));
            }
            Some((w, h))
        }
        (Some(_), None) | (None, Some(_)) => {
            return Err(LavaError::Config(
                "Must specify both width and height together".to_string(),
            ));
        }
        (None, None) => None,
    };

    let adapt_blobs = input.toml_widget_adapt_blobs;

    Ok(ResolvedPolicy {
        mode,
        target_fps,
        force_compact,
        explicit_dimensions,
        adapt_blobs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fps_zero_rejected() {
        let input_cli_zero = PolicyInput {
            cli_fps: Some(0),
            ..Default::default()
        };
        let err = resolve_policy(&input_cli_zero).expect_err("CLI FPS=0 must be rejected");
        assert!(err.to_string().contains("greater than zero"));

        let input_all_zero = PolicyInput {
            toml_render_fps: 0,
            toml_widget_fps: 0,
            ..Default::default()
        };
        let policy = resolve_policy(&input_all_zero).expect("Fallback default FPS must be > 0");
        assert_eq!(policy.target_fps, 30);
    }

    #[test]
    fn test_default_policy_resolution() {
        let input = PolicyInput {
            toml_render_fps: 30,
            toml_widget_fps: 15,
            toml_widget_adapt_blobs: true,
            ..Default::default()
        };

        let policy = resolve_policy(&input).expect("Valid default resolution");
        assert_eq!(policy.mode, ExecutionMode::Interactive);
        assert_eq!(policy.target_fps, 30);
        assert!(!policy.force_compact);
        assert_eq!(policy.explicit_dimensions, None);
        assert!(policy.adapt_blobs);
    }

    #[test]
    fn test_normal_mode_ignores_toml_widget_settings() {
        let input = PolicyInput {
            toml_render_fps: 60,
            toml_widget_fps: 15,
            toml_widget_compact: true,
            toml_widget_inline: true,
            toml_widget_width: Some(30),
            toml_widget_height: None, // Even partial widget dims must not break normal mode
            toml_widget_adapt_blobs: true,
            ..Default::default()
        };

        let policy = resolve_policy(&input).expect("Normal mode ignores [widget] TOML");
        assert_eq!(policy.mode, ExecutionMode::Interactive);
        assert_eq!(policy.target_fps, 60);
        assert!(!policy.force_compact);
        assert_eq!(policy.explicit_dimensions, None);
    }

    #[test]
    fn test_widget_mode_applies_toml_widget_settings() {
        let input = PolicyInput {
            cli_widget: true,
            toml_render_fps: 60,
            toml_widget_fps: 18,
            toml_widget_compact: true,
            toml_widget_inline: true,
            toml_widget_width: Some(36),
            toml_widget_height: Some(10),
            toml_widget_adapt_blobs: true,
            ..Default::default()
        };

        let policy = resolve_policy(&input).expect("Widget mode applies [widget] TOML");
        assert_eq!(policy.mode, ExecutionMode::Inline);
        assert_eq!(policy.target_fps, 18);
        assert!(policy.force_compact);
        assert_eq!(policy.explicit_dimensions, Some((36, 10)));
    }

    #[test]
    fn test_widget_mode_defaults_to_15_fps_and_compact() {
        let input = PolicyInput {
            cli_widget: true,
            toml_render_fps: 60,
            toml_widget_fps: 15,
            toml_widget_adapt_blobs: true,
            ..Default::default()
        };

        let policy = resolve_policy(&input).expect("Widget mode resolution");
        assert_eq!(policy.mode, ExecutionMode::Widget);
        assert_eq!(policy.target_fps, 15);
        assert!(policy.force_compact);
    }

    #[test]
    fn test_cli_fps_overrides_all() {
        let input = PolicyInput {
            cli_widget: true,
            cli_fps: Some(25),
            toml_render_fps: 60,
            toml_widget_fps: 15,
            ..Default::default()
        };

        let policy = resolve_policy(&input).expect("CLI fps override resolution");
        assert_eq!(policy.target_fps, 25);
    }

    #[test]
    fn test_snapshot_conflict_validation() {
        let input = PolicyInput {
            cli_snapshot: true,
            cli_inline: true,
            ..Default::default()
        };

        let err = resolve_policy(&input).expect_err("Snapshot and inline must conflict");
        assert!(err.to_string().contains("Conflict"));
    }

    #[test]
    fn test_dimension_validation() {
        let input_partial = PolicyInput {
            cli_width: Some(20),
            cli_height: None,
            ..Default::default()
        };
        assert!(resolve_policy(&input_partial).is_err());

        let input_zero = PolicyInput {
            cli_width: Some(0),
            cli_height: Some(10),
            ..Default::default()
        };
        assert!(resolve_policy(&input_zero).is_err());

        let input_valid = PolicyInput {
            cli_width: Some(20),
            cli_height: Some(5),
            ..Default::default()
        };
        let policy = resolve_policy(&input_valid).expect("Valid dimensions");
        assert_eq!(policy.explicit_dimensions, Some((20, 5)));
    }

    #[test]
    fn test_fps_above_240_rejected() {
        let input_max = PolicyInput {
            cli_fps: Some(240),
            ..Default::default()
        };
        assert!(resolve_policy(&input_max).is_ok());

        let input_over = PolicyInput {
            cli_fps: Some(241),
            ..Default::default()
        };
        let err = resolve_policy(&input_over).expect_err("FPS > 240 must be rejected");
        assert!(err.to_string().contains("between 1 and 240"));
    }

    #[test]
    fn test_dimensions_above_1000_rejected() {
        let input_max = PolicyInput {
            cli_width: Some(1000),
            cli_height: Some(1000),
            ..Default::default()
        };
        assert!(resolve_policy(&input_max).is_ok());

        let input_wide = PolicyInput {
            cli_width: Some(1001),
            cli_height: Some(50),
            ..Default::default()
        };
        let err_w = resolve_policy(&input_wide).expect_err("Width > 1000 must be rejected");
        assert!(err_w.to_string().contains("between 1 and 1000"));

        let input_tall = PolicyInput {
            cli_width: Some(50),
            cli_height: Some(1001),
            ..Default::default()
        };
        let err_h = resolve_policy(&input_tall).expect_err("Height > 1000 must be rejected");
        assert!(err_h.to_string().contains("between 1 and 1000"));
    }
}
