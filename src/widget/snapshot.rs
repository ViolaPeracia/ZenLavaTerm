//! Single-shot ANSI True Color frame serializer for status bars and external scripts.

use crate::core::Simulation;
use crate::render::{
    rasterize_simulation_options, BlockRenderer, BrailleRenderer, ColorPalette, CursorMode,
    HalfBlockRenderer, Renderer, VirtualFramebuffer,
};
use crate::{LavaError, Result};

/// Serializes a single frame of the simulation into a standalone ANSI True Color string with explicit gradient control.
#[allow(clippy::too_many_arguments)]
pub fn render_snapshot_options(
    sim: &mut Simulation,
    palette: &ColorPalette,
    cols: u16,
    rows: u16,
    renderer_type: &str,
    threshold: f32,
    warmup_steps: usize,
    gradient: bool,
) -> Result<String> {
    if cols == 0 || rows == 0 {
        return Err(LavaError::Render(
            "Snapshot dimensions must be non-zero".to_string(),
        ));
    }

    // 1. Advance simulation warmup steps to form natural lava contours
    let dt = 1.0 / 30.0;
    for _ in 0..warmup_steps {
        sim.step(dt);
    }

    // 2. Compute virtual framebuffer dimensions
    let (v_width, v_height) = match renderer_type {
        "block" => (cols as usize, rows as usize),
        "braille" => (cols as usize * 2, rows as usize * 4),
        _ => (cols as usize, rows as usize * 2),
    };

    let mut fb = VirtualFramebuffer::new(v_width, v_height, palette.background);
    rasterize_simulation_options(sim, &mut fb, palette, threshold, gradient);

    // 3. Serialize into in-memory ANSI string without absolute cursor positioning
    let mut out = Vec::with_capacity(cols as usize * rows as usize * 30);
    let mut renderer: Box<dyn Renderer> = match renderer_type {
        "block" => Box::new(BlockRenderer::with_cursor_mode(CursorMode::Stream)),
        "braille" => Box::new(BrailleRenderer::with_cursor_mode(CursorMode::Stream)),
        _ => Box::new(HalfBlockRenderer::with_cursor_mode(CursorMode::Stream)),
    };
    renderer.render(&fb, &mut out)?;

    String::from_utf8(out).map_err(|e| LavaError::Render(e.to_string()))
}

/// Serializes a single frame of the simulation into a standalone ANSI True Color string with smooth gradients.
pub fn render_snapshot(
    sim: &mut Simulation,
    palette: &ColorPalette,
    cols: u16,
    rows: u16,
    renderer_type: &str,
    threshold: f32,
    warmup_steps: usize,
) -> Result<String> {
    render_snapshot_options(
        sim,
        palette,
        cols,
        rows,
        renderer_type,
        threshold,
        warmup_steps,
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::PhysicsParams;

    fn create_test_sim() -> Simulation {
        let params = PhysicsParams {
            gravity: 0.12,
            buoyancy: 0.80,
            viscosity: 0.93,
            noise: 0.15,
            thermal_transfer_rate: 0.40,
        };
        Simulation::new(params, 6, 42)
    }

    #[test]
    fn test_snapshot_zero_dimensions_error() {
        let mut sim = create_test_sim();
        let palette = ColorPalette::default();
        assert!(render_snapshot(&mut sim, &palette, 0, 10, "halfblock", 1.0, 5).is_err());
        assert!(render_snapshot(&mut sim, &palette, 10, 0, "halfblock", 1.0, 5).is_err());
    }

    #[test]
    fn test_snapshot_micro_geometries_all_renderers() {
        let palette = ColorPalette::default();
        let micro_geometries = [(20, 1), (20, 2), (20, 3)];
        let renderers = ["halfblock", "block", "braille"];

        for &(cols, rows) in &micro_geometries {
            for renderer in &renderers {
                let mut sim = create_test_sim();
                let output = render_snapshot(&mut sim, &palette, cols, rows, renderer, 1.0, 3)
                    .expect("Snapshot generation succeeds");

                assert!(output.ends_with("\x1b[0m"), "Must end with ANSI reset");
                assert!(
                    output.contains("\x1b[38;2;"),
                    "Must contain True Color SGR sequences"
                );

                if rows > 1 {
                    let newline_count = output.matches('\n').count();
                    assert_eq!(
                        newline_count,
                        (rows - 1) as usize,
                        "Must contain exact row newline separators"
                    );
                } else {
                    assert!(
                        !output.contains('\n'),
                        "Single-row snapshot must not contain newline"
                    );
                }
            }
        }
    }

    #[test]
    fn test_snapshot_standard_geometries() {
        let palette = ColorPalette::default();
        let mut sim = create_test_sim();
        let output = render_snapshot(&mut sim, &palette, 24, 8, "halfblock", 1.0, 5)
            .expect("Standard snapshot succeeds");

        assert!(output.contains('▀'));
        assert_eq!(output.matches('\n').count(), 7);
    }

    #[test]
    fn test_snapshot_gradient_option() {
        let palette = ColorPalette::default();
        let mut sim1 = create_test_sim();
        let mut sim2 = create_test_sim();

        let out_grad =
            render_snapshot_options(&mut sim1, &palette, 20, 6, "halfblock", 1.0, 3, true)
                .expect("Snapshot with gradient succeeds");
        let out_flat =
            render_snapshot_options(&mut sim2, &palette, 20, 6, "halfblock", 1.0, 3, false)
                .expect("Snapshot without gradient succeeds");

        assert!(out_grad.contains('▀'));
        assert!(out_flat.contains('▀'));
    }
}
