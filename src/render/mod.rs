//! Terminal rendering, color interpolation, and framebuffer abstractions.

pub mod block;
pub mod braille;
pub mod color;
pub mod framebuffer;
pub mod halfblock;

pub use block::BlockRenderer;
pub use braille::BrailleRenderer;
pub use color::{ColorPalette, Rgb};
pub use framebuffer::VirtualFramebuffer;
pub use halfblock::HalfBlockRenderer;

use crate::core::Simulation;
use std::io::Write;

/// Controls how a renderer positions the terminal cursor across rows and frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorMode {
    #[default]
    Absolute,
    Inline,
    /// Plain LF (`\n`) row separation for cooked stdout, pipes, and status-bar snapshots.
    Stream,
}

/// Visual options controlling scalar field rasterization into a `VirtualFramebuffer`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RasterizeOptions {
    /// Isosurface density threshold for metaball boundaries.
    pub threshold: f32,
    /// Enable smooth 3-stop gradient shading (false uses fast stepped stops).
    pub gradient: bool,
    /// Enable soft edge glow aura and bottom furnace light (disabled for 1-bit Braille).
    pub ambient_glow: bool,
    /// Enable animated molten bottom pool that blobs organically merge with and detach from.
    pub molten_pool: bool,
    /// Physical viewport aspect ratio (`width / height`) for isotropic circular metaballs.
    pub aspect_ratio: f32,
}

impl Default for RasterizeOptions {
    fn default() -> Self {
        Self {
            threshold: 1.0,
            gradient: true,
            ambient_glow: true,
            molten_pool: true,
            aspect_ratio: 1.0,
        }
    }
}

impl RasterizeOptions {
    /// Builds rasterization options matched to a renderer backend.
    ///
    /// Automatically disables `ambient_glow` for `braille` because Braille's 1-bit dot
    /// classifier treats any non-background pixel as an active dot.
    pub fn for_renderer(
        renderer_type: &str,
        threshold: f32,
        gradient: bool,
        glow: bool,
        molten_pool: bool,
    ) -> Self {
        Self {
            threshold,
            gradient,
            ambient_glow: glow && gradient && renderer_type != "braille",
            molten_pool,
            aspect_ratio: 1.0,
        }
    }

    /// Computes physical `width / height` aspect ratio for a terminal grid
    /// assuming standard monospace character cells (`cell_width ≈ 0.5 * cell_height`).
    pub fn physical_aspect_ratio(cols: u16, rows: u16) -> f32 {
        if cols == 0 || rows == 0 {
            return 1.0;
        }
        ((cols as f32 * 0.5) / rows as f32).clamp(0.35, 3.5)
    }

    /// Returns a copy of these options with an explicit physical aspect ratio.
    pub fn with_aspect_ratio(mut self, aspect_ratio: f32) -> Self {
        self.aspect_ratio = aspect_ratio.clamp(0.25, 4.0);
        self
    }
}

/// Common interface implemented by terminal renderers.
pub trait Renderer {
    /// Renders the contents of the virtual framebuffer to the provided output writer.
    fn render(
        &mut self,
        buffer: &VirtualFramebuffer,
        writer: &mut dyn Write,
    ) -> std::io::Result<()>;
}

/// Rasterizes the continuous simulation into the discrete virtual framebuffer.
pub fn rasterize_simulation(
    sim: &Simulation,
    buffer: &mut VirtualFramebuffer,
    palette: &ColorPalette,
    options: &RasterizeOptions,
) {
    let width = buffer.width;
    let height = buffer.height;

    if width == 0 || height == 0 {
        return;
    }

    let inv_w = 1.0 / width as f32;
    let inv_h = 1.0 / height as f32;
    let pixels = buffer.as_mut_slice();
    let use_pool = options.molten_pool && height >= 6;
    let t = sim.elapsed_time;
    let aspect = options.aspect_ratio.clamp(0.25, 4.0);
    let (scale_x, scale_y) = if aspect >= 1.0 {
        (aspect, 1.0)
    } else {
        (1.0, 1.0 / aspect)
    };
    let ripples = sim.ripple_waves();
    let beat_glow = sim.beat_envelope() * 0.25;

    for y in 0..height {
        // Map discrete y [0..height] to normalized space [0.0..1.0] (y=0 top in terminal, so invert for simulation bottom heat)
        let sim_y = 1.0 - (y as f32 + 0.5) * inv_h;
        let row_start = y * width;
        let check_pool = use_pool && sim_y < 0.22;

        for x in 0..width {
            let sim_x = (x as f32 + 0.5) * inv_w;

            let (mut field_val, mut temp) = sim.evaluate_at_aspect(sim_x, sim_y, aspect);

            if check_pool {
                let surface_y = 0.042
                    + 0.012 * (sim_x * 8.5 + t * 1.1).sin()
                    + 0.008 * (sim_x * 14.0 - t * 0.75).cos();
                let dy = (sim_y - surface_y).max(0.0);
                let pool_field = 0.0028 / (dy * dy + 0.0020);
                let combined = field_val + pool_field;
                if combined > 0.0 {
                    temp = (field_val * temp + pool_field * 1.0) / combined;
                }
                field_val = combined;
            }

            let mut ripple_glow = 0.0f32;
            if !ripples.is_empty() {
                const BAND: f32 = 0.068;
                for wave in ripples {
                    let dx = (sim_x - wave.cx) * scale_x;
                    let dy = (sim_y - wave.cy) * scale_y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let delta = (dist - wave.radius()).abs();
                    if delta < BAND {
                        let c = (delta / BAND * std::f32::consts::FRAC_PI_2).cos();
                        let ring = c * c * wave.envelope();
                        field_val += ring * 0.45;
                        ripple_glow = (ripple_glow + ring * 0.72).min(1.0);
                    }
                }
            }

            pixels[row_start + x] = palette.sample_lava_surface(
                temp,
                field_val,
                options.threshold,
                sim_y,
                options.gradient,
                options.ambient_glow,
                (beat_glow + ripple_glow).min(1.0),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::PhysicsParams;

    #[test]
    fn test_rasterize_options_disables_ambient_glow_for_braille() {
        let hb = RasterizeOptions::for_renderer("halfblock", 1.0, true, true, true);
        assert!(hb.ambient_glow);
        assert!(hb.molten_pool);

        let br = RasterizeOptions::for_renderer("braille", 1.0, true, true, true);
        assert!(!br.ambient_glow);
        assert!(br.molten_pool);

        let stepped = RasterizeOptions::for_renderer("halfblock", 1.0, false, true, true);
        assert!(!stepped.ambient_glow);
    }

    #[test]
    fn test_molten_pool_populates_bottom_row() {
        let sim = Simulation::new(PhysicsParams::default(), 4, 42);
        let palette = ColorPalette::default();
        let mut fb = VirtualFramebuffer::new(40, 20, palette.background);
        let opts = RasterizeOptions {
            threshold: 1.0,
            gradient: true,
            ambient_glow: false,
            molten_pool: true,
            aspect_ratio: 1.0,
        };
        rasterize_simulation(&sim, &mut fb, &palette, &opts);

        let bottom_row_active = (0..40)
            .filter(|&x| fb.get_pixel(x, 19) != Some(palette.background))
            .count();
        assert_eq!(
            bottom_row_active, 40,
            "Molten bottom pool must span the base row"
        );
    }

    #[test]
    fn test_braille_rasterization_keeps_upper_empty_pixels_exact_background() {
        let sim = Simulation::new(PhysicsParams::default(), 4, 42);
        let palette = ColorPalette::default();
        let mut fb = VirtualFramebuffer::new(40, 20, palette.background);
        let opts = RasterizeOptions::for_renderer("braille", 1.0, true, true, true);
        rasterize_simulation(&sim, &mut fb, &palette, &opts);

        // Top-left corner (far from blobs and bottom pool) stays exact background
        assert_eq!(fb.get_pixel(0, 0), Some(palette.background));
    }
}
