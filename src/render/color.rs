//! 24-bit True Color representation, linear interpolation, and multi-stop palettes.

use crate::ParseColorError;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 24-bit RGB Color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// Creates a new RGB color.
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parses an RGB color from a hex string (e.g. `"#ff3b00"` or `"ff3b00"`).
    pub fn from_hex(hex: &str) -> Result<Self, ParseColorError> {
        let clean_hex = hex.trim().trim_start_matches('#');
        if clean_hex.len() != 6 || !clean_hex.is_ascii() {
            return Err(ParseColorError(hex.to_string()));
        }

        let r = u8::from_str_radix(&clean_hex[0..2], 16)
            .map_err(|_| ParseColorError(hex.to_string()))?;
        let g = u8::from_str_radix(&clean_hex[2..4], 16)
            .map_err(|_| ParseColorError(hex.to_string()))?;
        let b = u8::from_str_radix(&clean_hex[4..6], 16)
            .map_err(|_| ParseColorError(hex.to_string()))?;

        Ok(Self::new(r, g, b))
    }

    /// Linearly interpolates between color `a` and `b` by factor `t` in $[0.0, 1.0]$.
    #[inline]
    pub fn lerp(a: Self, b: Self, t: f32) -> Self {
        let t_clamped = t.clamp(0.0, 1.0);
        let r = (a.r as f32 + (b.r as f32 - a.r as f32) * t_clamped).round() as u8;
        let g = (a.g as f32 + (b.g as f32 - a.g as f32) * t_clamped).round() as u8;
        let b_val = (a.b as f32 + (b.b as f32 - a.b as f32) * t_clamped).round() as u8;
        Self::new(r, g, b_val)
    }

    /// Formats the color as a hex string `#rrggbb`.
    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl Serialize for Rgb {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for Rgb {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Rgb::from_hex(&s).map_err(serde::de::Error::custom)
    }
}

/// Palette holding gradient anchor colors.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ColorPalette {
    /// Heat source bottom color (hot).
    pub bottom: Rgb,
    /// Convective middle color.
    pub middle: Rgb,
    /// Cooled top chamber color (cold).
    pub top: Rgb,
    /// Fluid chamber background color.
    pub background: Rgb,
}

impl Default for ColorPalette {
    fn default() -> Self {
        Self {
            bottom: Rgb::new(0xFF, 0x3B, 0x00),     // #ff3b00
            middle: Rgb::new(0xFF, 0x7A, 0x00),     // #ff7a00
            top: Rgb::new(0x7B, 0x2C, 0xFF),        // #7b2cff
            background: Rgb::new(0x0D, 0x0D, 0x15), // #0d0d15
        }
    }
}

impl ColorPalette {
    /// Interpolates the 3-stop thermal ramp (`top -> middle -> bottom`).
    #[inline]
    fn sample_thermal_color(&self, temp: f32) -> Rgb {
        let t = temp.clamp(0.0, 1.0);
        if t < 0.5 {
            Rgb::lerp(self.top, self.middle, t * 2.0)
        } else {
            Rgb::lerp(self.middle, self.bottom, (t - 0.5) * 2.0)
        }
    }

    /// Samples lava color with optional ambient furnace background glow and soft outer halo.
    ///
    /// When `ambient_glow` is `false` (required for 1-bit Braille dot thresholding) or `gradient`
    /// is `false` (fast stepped mode), pixels below `threshold` return `self.background` exactly.
    #[inline]
    pub fn sample_lava_surface(
        &self,
        temp: f32,
        field_intensity: f32,
        threshold: f32,
        sim_y: f32,
        gradient: bool,
        ambient_glow: bool,
    ) -> Rgb {
        if !gradient || !ambient_glow {
            return self.sample_lava_stepped(temp, field_intensity, threshold, gradient);
        }

        // Subtle furnace light cast upward from the heated lamp base.
        let bg = if sim_y < 0.36 {
            let h = ((0.36 - sim_y) / 0.36).clamp(0.0, 1.0);
            Rgb::lerp(self.background, self.bottom, h * h * 0.18)
        } else {
            self.background
        };

        let halo_start = threshold * 0.55;
        if field_intensity < halo_start {
            return bg;
        }

        let base_color = self.sample_thermal_color(temp);

        if field_intensity < threshold {
            let denom = (threshold - halo_start).max(1e-4);
            let halo = ((field_intensity - halo_start) / denom).clamp(0.0, 1.0);
            return Rgb::lerp(bg, base_color, halo * halo * 0.48);
        }

        let edge_factor = ((field_intensity - threshold) / 0.35).clamp(0.0, 1.0);
        Rgb::lerp(
            Rgb::new(255, 255, 255),
            base_color,
            0.55 + 0.45 * edge_factor,
        )
    }

    /// Samples the gradient based on normalized temperature, field intensity, and gradient smoothing flag.
    pub fn sample_lava_stepped(
        &self,
        temp: f32,
        field_intensity: f32,
        threshold: f32,
        gradient: bool,
    ) -> Rgb {
        if field_intensity < threshold {
            return self.background;
        }

        if !gradient {
            let t = temp.clamp(0.0, 1.0);
            return if t < 0.33 {
                self.top
            } else if t < 0.66 {
                self.middle
            } else {
                self.bottom
            };
        }

        let base_color = self.sample_thermal_color(temp);
        let edge_factor = ((field_intensity - threshold) / 0.3).clamp(0.0, 1.0);
        Rgb::lerp(Rgb::new(255, 255, 255), base_color, 0.4 + 0.6 * edge_factor)
    }

    /// Samples the gradient based on normalized temperature and field intensity using smooth linear interpolation.
    pub fn sample_lava(&self, temp: f32, field_intensity: f32, threshold: f32) -> Rgb {
        self.sample_lava_stepped(temp, field_intensity, threshold, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_parsing() {
        let col = Rgb::from_hex("#ff3b00").expect("Valid hex");
        assert_eq!(col, Rgb::new(0xFF, 0x3B, 0x00));

        let col_no_hash = Rgb::from_hex("7b2cff").expect("Valid hex without hash");
        assert_eq!(col_no_hash, Rgb::new(0x7B, 0x2C, 0xFF));

        assert!(Rgb::from_hex("invalid").is_err());
        assert!(Rgb::from_hex("#123").is_err());
    }

    #[test]
    fn test_from_hex_rejects_multibyte_utf8_without_panic() {
        assert!(Rgb::from_hex("#ééé").is_err());
        assert!(Rgb::from_hex("#🔥🔥").is_err());
    }

    #[test]
    fn test_color_lerp() {
        let black = Rgb::new(0, 0, 0);
        let white = Rgb::new(200, 200, 200);

        let mid = Rgb::lerp(black, white, 0.5);
        assert_eq!(mid, Rgb::new(100, 100, 100));

        assert_eq!(Rgb::lerp(black, white, 0.0), black);
        assert_eq!(Rgb::lerp(black, white, 1.0), white);
    }

    #[test]
    fn test_sample_lava_stepped_gradient_flag() {
        let palette = ColorPalette::default();
        let background = palette.sample_lava_stepped(0.5, 0.5, 1.0, false);
        assert_eq!(background, palette.background);

        // Gradient true interpolates smoothly
        let color_grad = palette.sample_lava_stepped(0.25, 2.0, 1.0, true);
        // Gradient false quantizes to discrete stops
        let color_flat = palette.sample_lava_stepped(0.25, 2.0, 1.0, false);
        assert_eq!(color_flat, palette.top);
        assert_ne!(color_grad, palette.top);
    }

    #[test]
    fn test_sample_lava_surface_glow_and_braille_isolation() {
        let palette = ColorPalette::default();

        // Ambient glow disabled (Braille/flat): pixels below threshold stay exact background
        let no_glow_halo = palette.sample_lava_surface(0.8, 0.85, 1.0, 0.05, true, false);
        assert_eq!(no_glow_halo, palette.background);

        // Ambient glow enabled: halo band blends smoothly above background
        let halo_color = palette.sample_lava_surface(0.8, 0.85, 1.0, 0.5, true, true);
        assert_ne!(halo_color, palette.background);

        // Furnace glow warms the bottom background while upper empty background stays pure
        let bottom_bg = palette.sample_lava_surface(0.5, 0.0, 1.0, 0.02, true, true);
        let top_bg = palette.sample_lava_surface(0.5, 0.0, 1.0, 0.90, true, true);
        assert!(bottom_bg.r > palette.background.r);
        assert_eq!(top_bg, palette.background);
    }

    #[test]
    fn test_palette_rejects_unknown_fields() {
        let toml_str = r##"
            bottom = "#ff0000"
            color1 = "#00ff00"
        "##;
        assert!(toml::from_str::<ColorPalette>(toml_str).is_err());
    }
}
