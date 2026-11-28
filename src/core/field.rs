//! 2D Scalar field evaluation and isosurface potential calculation.

use super::metaball::Blob;

/// Small epsilon to prevent division by zero in field potential calculations.
const EPSILON: f32 = 1e-4;
/// Compact-support cutoff multiplier (`(2.2 * r)^2 = 4.84 * r^2`), eliminating far-field glue.
const CUT_SQ_MULT: f32 = 4.84;
const TAIL: f32 = 1.0 / CUT_SQ_MULT;
const NORM: f32 = 1.0 / (1.0 - TAIL);

/// Evaluates a single blob's compact-support metaball contribution (`0.0` for `d >= 2.2 * r`,
/// `1.0` at `d == r`). Shared across all field query APIs so `evaluate_field`, `is_inside`,
/// and `evaluate_with_temperature_aspect` always agree on exact isosurface boundaries.
#[inline]
fn blob_contribution(r_sq: f32, d_sq: f32) -> f32 {
    if d_sq < r_sq * CUT_SQ_MULT {
        ((r_sq / (d_sq + EPSILON) - TAIL) * NORM).max(0.0)
    } else {
        0.0
    }
}

/// Evaluates the total scalar field intensity at coordinate `(px, py)` using the compact-support
/// metaball kernel (square `1:1` viewport).
#[inline]
pub fn evaluate_field(blobs: &[Blob], px: f32, py: f32) -> f32 {
    let mut sum = 0.0;
    for blob in blobs {
        let dx = px - blob.x;
        let dy = py - blob.y;
        let d_sq = dx * dx + dy * dy;
        let r_sq = blob.radius * blob.radius;
        sum += blob_contribution(r_sq, d_sq);
    }
    sum
}

/// Evaluates both the field intensity and the field-weighted temperature at `(px, py)`
/// assuming a square `1:1` viewport.
///
/// Returns `(field_intensity, weighted_temperature)`.
#[inline]
pub fn evaluate_with_temperature(blobs: &[Blob], px: f32, py: f32) -> (f32, f32) {
    evaluate_with_temperature_aspect(blobs, px, py, 1.0)
}

/// Evaluates field intensity and weighted temperature with area-preserving isotropic
/// aspect-ratio correction and compact-support metaball falloff.
///
/// Using compact support (`d < 2.2 * r`, normalized so `d == r -> 1.0`) eliminates infinite
/// far-field summation tails so 10–32 blobs stay distinct and only form organic gooey necks
/// when they genuinely approach each other.
#[inline]
pub fn evaluate_with_temperature_aspect(
    blobs: &[Blob],
    px: f32,
    py: f32,
    aspect_ratio: f32,
) -> (f32, f32) {
    let aspect = aspect_ratio.clamp(0.25, 4.0);
    let scale_x = aspect.sqrt();
    let scale_y = 1.0 / scale_x;

    let mut total_field = 0.0;
    let mut weighted_temp = 0.0;

    for blob in blobs {
        let dx = (px - blob.x) * scale_x;
        let dy = (py - blob.y) * scale_y;
        let d_sq = dx * dx + dy * dy;
        let r_sq = blob.radius * blob.radius;
        let contribution = blob_contribution(r_sq, d_sq);
        if contribution > 0.0 {
            total_field += contribution;
            weighted_temp += contribution * blob.temperature;
        }
    }

    let avg_temp = if total_field > 0.0 {
        weighted_temp / total_field
    } else {
        0.5
    };

    (total_field, avg_temp)
}

/// Determines if a given point exceeds the isosurface threshold.
#[inline]
pub fn is_inside(blobs: &[Blob], px: f32, py: f32, threshold: f32) -> bool {
    evaluate_field(blobs, px, py) >= threshold
}

/// Computes continuous scalar field potential and temperature distribution from blobs.
#[derive(Debug, Clone, Copy, Default)]
pub struct ScalarField;

impl ScalarField {
    /// Evaluates the total scalar field intensity at coordinate `(px, py)`.
    #[inline]
    pub fn evaluate_field(&self, blobs: &[Blob], px: f32, py: f32) -> f32 {
        evaluate_field(blobs, px, py)
    }

    /// Evaluates both the field intensity and the field-weighted temperature at `(px, py)`.
    #[inline]
    pub fn evaluate_with_temperature(&self, blobs: &[Blob], px: f32, py: f32) -> (f32, f32) {
        evaluate_with_temperature(blobs, px, py)
    }

    /// Determines if a given point exceeds the isosurface threshold.
    #[inline]
    pub fn is_inside(&self, blobs: &[Blob], px: f32, py: f32, threshold: f32) -> bool {
        is_inside(blobs, px, py, threshold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_decreases_with_distance() {
        let field = ScalarField;
        let blobs = vec![Blob::new(0.5, 0.5, 0.1, 1.0)];

        let near_val = field.evaluate_field(&blobs, 0.5, 0.55);
        let far_val = field.evaluate_field(&blobs, 0.5, 0.90);

        assert!(
            near_val > far_val,
            "Field must decrease as distance increases"
        );
    }

    #[test]
    fn test_field_superposition() {
        let field = ScalarField;
        let blob1 = Blob::new(0.4, 0.5, 0.1, 1.0);
        let blob2 = Blob::new(0.6, 0.5, 0.1, 1.0);

        let single_val = field.evaluate_field(std::slice::from_ref(&blob1), 0.5, 0.5);
        let dual_val = field.evaluate_field(&[blob1, blob2], 0.5, 0.5);

        assert!(
            dual_val > single_val * 1.5,
            "Two nearby blobs must superimpose their scalar fields"
        );
    }

    #[test]
    fn test_evaluate_field_and_is_inside_match_rendered_compact_kernel() {
        let blobs = vec![
            Blob::new(0.3, 0.5, 0.08, 0.9),
            Blob::new(0.7, 0.5, 0.08, 0.2),
        ];
        for &(px, py) in &[(0.3, 0.5), (0.38, 0.5), (0.5, 0.5), (0.95, 0.95)] {
            let f_scalar = evaluate_field(&blobs, px, py);
            let (f_temp, _) = evaluate_with_temperature(&blobs, px, py);
            let (f_aspect, _) = evaluate_with_temperature_aspect(&blobs, px, py, 1.0);
            assert!((f_scalar - f_temp).abs() < 1e-6);
            assert!((f_scalar - f_aspect).abs() < 1e-6);
            assert_eq!(is_inside(&blobs, px, py, 1.0), f_aspect >= 1.0);
        }
        // Outside 2.2 * r cutoff must be strictly zero in both APIs
        assert_eq!(evaluate_field(&blobs, 0.95, 0.95), 0.0);
    }
}
