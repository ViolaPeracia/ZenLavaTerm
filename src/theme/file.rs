//! Custom theme file parser supporting JSON, TOML, and 16-color schemes.

use crate::render::{ColorPalette, Rgb};
use serde::Deserialize;
use std::{fs, path::Path};

#[derive(Debug, Deserialize)]
struct DirectPaletteSchema {
    bottom: Option<String>,
    middle: Option<String>,
    top: Option<String>,
    background: Option<String>,
}

impl DirectPaletteSchema {
    fn to_palette(&self) -> Result<Option<ColorPalette>, String> {
        if self.bottom.is_some() || self.top.is_some() {
            let default_pal = ColorPalette::default();
            let parse_opt = |opt: Option<&str>, fallback: Rgb| -> Result<Rgb, String> {
                match opt {
                    Some(h) => Rgb::from_hex(h).map_err(|e| e.to_string()),
                    None => Ok(fallback),
                }
            };
            Ok(Some(ColorPalette {
                bottom: parse_opt(self.bottom.as_deref(), default_pal.bottom)?,
                middle: parse_opt(self.middle.as_deref(), default_pal.middle)?,
                top: parse_opt(self.top.as_deref(), default_pal.top)?,
                background: parse_opt(self.background.as_deref(), default_pal.background)?,
            }))
        } else {
            Ok(None)
        }
    }
}

/// Loads and parses a custom theme file from any arbitrary path.
pub fn load_custom_theme_file(path: &Path) -> Result<ColorPalette, String> {
    let content = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read theme file at '{}': {e}", path.display()))?;

    let trimmed = content.trim();

    // 1. If it looks like JSON, try JSON direct palette -> Pywal JSON -> Wallust JSON
    if trimmed.starts_with('{') {
        if let Ok(json_palette) = serde_json::from_str::<DirectPaletteSchema>(trimmed) {
            if let Some(pal) = json_palette.to_palette()? {
                return Ok(pal);
            }
        }
        if let Ok(pal) = super::pywal::parse_pywal_json(trimmed) {
            return Ok(pal);
        }
        return super::wallust::parse_wallust_json(trimmed);
    }

    // 2. Try parsing as TOML direct palette schema
    if let Ok(toml_palette) = toml::from_str::<DirectPaletteSchema>(trimmed) {
        if let Some(pal) = toml_palette.to_palette()? {
            return Ok(pal);
        }
    }

    // 3. Try parsing as flat 16-color hex text
    super::pywal::parse_pywal_flat(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_direct_json() {
        let json = r##"{
            "bottom": "#00ffcc",
            "middle": "#0077ff",
            "top": "#ff0077",
            "background": "#000011"
        }"##;
        let pal = serde_json::from_str::<DirectPaletteSchema>(json).unwrap();
        let b = Rgb::from_hex(pal.bottom.as_deref().unwrap()).unwrap();
        assert_eq!(b, Rgb::new(0x00, 0xFF, 0xCC));
    }

    #[test]
    fn test_broken_json_theme_returns_error() {
        let dir = std::env::temp_dir();
        let path = dir.join("lavaterm_test_broken_json_theme.json");
        fs::write(&path, "{ invalid json").unwrap();
        let res = load_custom_theme_file(&path);
        let _ = fs::remove_file(&path);
        assert!(res.is_err());
    }

    #[test]
    fn test_invalid_hex_in_direct_theme_returns_error() {
        let dir = std::env::temp_dir();
        let json_path = dir.join("lavaterm_test_invalid_hex_direct.json");
        fs::write(&json_path, r##"{"bottom": "not-a-hex", "top": "#ff0077"}"##).unwrap();
        let json_res = load_custom_theme_file(&json_path);
        let _ = fs::remove_file(&json_path);
        assert!(json_res.is_err());

        let toml_path = dir.join("lavaterm_test_invalid_hex_direct.toml");
        fs::write(&toml_path, "bottom = \"not-a-hex\"\ntop = \"#ff0077\"\n").unwrap();
        let toml_res = load_custom_theme_file(&toml_path);
        let _ = fs::remove_file(&toml_path);
        assert!(toml_res.is_err());
    }
}
