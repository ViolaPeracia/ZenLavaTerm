//! DEC Sixel (DCS) high-resolution terminal graphics renderer.
//!
//! Supported by Windows Terminal, WezTerm, Foot, Konsole, iTerm2, and XTerm.
//! Uses zero-dependency 5-bit RGB popularity quantization (≤ 255 color registers)
//! and Sixel RLE compression to render smooth volumetric lava gradients cleanly.

use super::color::Rgb;
use super::framebuffer::VirtualFramebuffer;
use super::{CursorMode, Renderer};
use std::io::{self, Write};

const BUCKET_COUNT: usize = 32 * 32 * 32; // 5-bit per channel RGB LUT
const MAX_SIXEL_COLORS: usize = 255;

#[inline]
fn rgb_bucket(c: Rgb) -> usize {
    (((c.r >> 3) as usize) << 10) | (((c.g >> 3) as usize) << 5) | ((c.b >> 3) as usize)
}

#[inline]
fn bucket_to_rgb(bucket: usize) -> Rgb {
    let r5 = ((bucket >> 10) & 0x1F) as u8;
    let g5 = ((bucket >> 5) & 0x1F) as u8;
    let b5 = (bucket & 0x1F) as u8;
    Rgb::new(
        (r5 << 3) | (r5 >> 2),
        (g5 << 3) | (g5 >> 2),
        (b5 << 3) | (b5 >> 2),
    )
}

#[inline]
fn to_sixel_pct(channel: u8) -> u8 {
    ((channel as u16 * 100 + 127) / 255) as u8
}

#[inline]
fn color_dist_sq(a: Rgb, b: Rgb) -> u32 {
    let dr = a.r as i32 - b.r as i32;
    let dg = a.g as i32 - b.g as i32;
    let db = a.b as i32 - b.b as i32;
    (dr * dr + dg * dg + db * db) as u32
}

fn write_rle_run(writer: &mut dyn Write, count: usize, ch: u8) -> io::Result<()> {
    if count >= 4 {
        write!(writer, "!{}{}", count, ch as char)
    } else {
        for _ in 0..count {
            writer.write_all(&[ch])?;
        }
        Ok(())
    }
}

/// Terminal renderer using DEC Sixel Graphics (`\x1bP0;0;0q...\x1b\\`).
#[derive(Debug, Default)]
pub struct SixelRenderer {
    cursor_mode: CursorMode,
    inline_anchored: bool,
    bucket_counts: Vec<u32>,
    bucket_to_reg: Vec<u16>,
    active_buckets: Vec<u16>,
    palette: Vec<Rgb>,
    pixel_regs: Vec<u8>,
    band_buf: Vec<u8>,
}

impl SixelRenderer {
    /// Creates a new `SixelRenderer` in `CursorMode::Absolute`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `SixelRenderer` with the specified cursor mode.
    pub fn with_cursor_mode(cursor_mode: CursorMode) -> Self {
        Self {
            cursor_mode,
            ..Self::default()
        }
    }

    fn quantize_frame(&mut self, pixels: &[Rgb]) {
        if self.bucket_counts.len() != BUCKET_COUNT {
            self.bucket_counts.resize(BUCKET_COUNT, 0);
            self.bucket_to_reg.resize(BUCKET_COUNT, 0);
        }

        self.active_buckets.clear();
        for &px in pixels {
            let b = rgb_bucket(px);
            if self.bucket_counts[b] == 0 {
                self.active_buckets.push(b as u16);
            }
            self.bucket_counts[b] = self.bucket_counts[b].saturating_add(1);
        }

        if self.active_buckets.len() > MAX_SIXEL_COLORS {
            let counts = &self.bucket_counts;
            self.active_buckets
                .sort_unstable_by(|&a, &b| counts[b as usize].cmp(&counts[a as usize]));
        }

        self.palette.clear();
        let palette_len = self.active_buckets.len().min(MAX_SIXEL_COLORS);
        for (reg, &bucket) in self.active_buckets[..palette_len].iter().enumerate() {
            self.bucket_to_reg[bucket as usize] = reg as u16;
            self.palette.push(bucket_to_rgb(bucket as usize));
        }

        if self.active_buckets.len() > MAX_SIXEL_COLORS {
            for &bucket in &self.active_buckets[MAX_SIXEL_COLORS..] {
                let target = bucket_to_rgb(bucket as usize);
                let mut best_reg = 0usize;
                let mut best_dist = u32::MAX;
                for (reg, &cand) in self.palette.iter().enumerate() {
                    let d = color_dist_sq(target, cand);
                    if d < best_dist {
                        best_dist = d;
                        best_reg = reg;
                    }
                }
                self.bucket_to_reg[bucket as usize] = best_reg as u16;
            }
        }

        self.pixel_regs.clear();
        self.pixel_regs.reserve(pixels.len());
        for &px in pixels {
            let b = rgb_bucket(px);
            self.pixel_regs.push(self.bucket_to_reg[b] as u8);
        }

        // Reset only touched bucket counts so subsequent frames stay O(unique_colors).
        for &bucket in &self.active_buckets {
            self.bucket_counts[bucket as usize] = 0;
        }
    }
}

impl Renderer for SixelRenderer {
    fn render(&mut self, buffer: &VirtualFramebuffer, writer: &mut dyn Write) -> io::Result<()> {
        if buffer.width == 0 || buffer.height == 0 {
            return Ok(());
        }

        let width = buffer.width;
        let height = buffer.height;
        self.quantize_frame(buffer.as_slice());

        match self.cursor_mode {
            CursorMode::Absolute => {
                writer.write_all(b"\x1b[1;1H")?;
            }
            CursorMode::Inline => {
                if self.inline_anchored {
                    writer.write_all(b"\x1b8")?;
                } else {
                    writer.write_all(b"\x1b7")?;
                    self.inline_anchored = true;
                }
            }
            CursorMode::Stream => {}
        }

        // P1=0 (1:1 aspect), P2=0 (fill background cleanly), P3=0 + raster attribute "1;1;W;H
        write!(writer, "\x1bP0;0;0q\"1;1;{width};{height}")?;

        for (reg, &rgb) in self.palette.iter().enumerate() {
            write!(
                writer,
                "#{};2;{};{};{}",
                reg,
                to_sixel_pct(rgb.r),
                to_sixel_pct(rgb.g),
                to_sixel_pct(rgb.b)
            )?;
        }

        self.band_buf.resize(width, 0);
        let mut used_flags = [false; 256];
        let mut band_regs = Vec::with_capacity(64);
        let num_bands = height.div_ceil(6);

        for band in 0..num_bands {
            let y0 = band * 6;
            let band_h = (height - y0).min(6);
            band_regs.clear();

            for dy in 0..band_h {
                let row_start = (y0 + dy) * width;
                for &reg in &self.pixel_regs[row_start..row_start + width] {
                    let idx = reg as usize;
                    if !used_flags[idx] {
                        used_flags[idx] = true;
                        band_regs.push(reg);
                    }
                }
            }

            for (reg_pos, &reg) in band_regs.iter().enumerate() {
                let mut last_nonzero = 0usize;
                for x in 0..width {
                    let mut mask = 0u8;
                    for dy in 0..band_h {
                        if self.pixel_regs[(y0 + dy) * width + x] == reg {
                            mask |= 1u8 << dy;
                        }
                    }
                    self.band_buf[x] = mask;
                    if mask != 0 {
                        last_nonzero = x + 1;
                    }
                }

                if last_nonzero > 0 {
                    write!(writer, "#{reg}")?;
                    let mut run_ch = b'?' + self.band_buf[0];
                    let mut run_len = 1usize;
                    for &mask in &self.band_buf[1..last_nonzero] {
                        let ch = b'?' + mask;
                        if ch == run_ch {
                            run_len += 1;
                        } else {
                            write_rle_run(writer, run_len, run_ch)?;
                            run_ch = ch;
                            run_len = 1;
                        }
                    }
                    write_rle_run(writer, run_len, run_ch)?;
                    if reg_pos + 1 < band_regs.len() {
                        writer.write_all(b"$")?;
                    }
                }
            }

            for &reg in &band_regs {
                used_flags[reg as usize] = false;
            }

            if band + 1 < num_bands {
                writer.write_all(b"-")?;
            }
        }

        writer.write_all(b"\x1b\\")?;

        if self.cursor_mode == CursorMode::Stream {
            writer.write_all(b"\n")?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sixel_basic_frame_and_rle() {
        let mut renderer = SixelRenderer::with_cursor_mode(CursorMode::Absolute);
        let fb = VirtualFramebuffer::new(10, 6, Rgb::new(255, 0, 0));
        let mut out = Vec::new();
        renderer.render(&fb, &mut out).expect("sixel render");
        let s = String::from_utf8(out).expect("valid utf8");

        assert!(s.starts_with("\x1b[1;1H\x1bP0;0;0q\"1;1;10;6"));
        assert!(s.contains("#0;2;100;0;0"));
        // All 6 rows filled across 10 cols -> mask 0b111111 = 63 -> '?' + 63 = '~', RLE "!10~"
        assert!(s.contains("#0!10~"));
        assert!(s.ends_with("\x1b\\"));
    }

    #[test]
    fn test_sixel_caps_palette_at_255_registers() {
        let mut renderer = SixelRenderer::with_cursor_mode(CursorMode::Stream);
        let mut fb = VirtualFramebuffer::new(32, 32, Rgb::new(0, 0, 0));
        // Fill with 1024 distinct 5-bit RGB colors (> 255)
        for y in 0..32 {
            for x in 0..32 {
                let r = ((x * 8) & 0xFF) as u8;
                let g = ((y * 8) & 0xFF) as u8;
                let b = (((x + y) * 4) & 0xFF) as u8;
                fb.set_pixel(x, y, Rgb::new(r, g, b));
            }
        }

        let mut out = Vec::new();
        renderer.render(&fb, &mut out).expect("sixel render");
        let s = String::from_utf8(out).expect("valid utf8");

        assert!(renderer.palette.len() <= MAX_SIXEL_COLORS);
        assert!(s.starts_with("\x1bP0;0;0q\"1;1;32;32"));
        assert!(s.ends_with("\x1b\\\n"));
        // Register index 255 or higher must never be emitted
        assert!(!s.contains("#255;2;"));
    }

    #[test]
    fn test_sixel_multiband_separator() {
        let mut renderer = SixelRenderer::new();
        let fb = VirtualFramebuffer::new(4, 12, Rgb::new(0, 128, 255));
        let mut out = Vec::new();
        renderer.render(&fb, &mut out).expect("render");
        let s = String::from_utf8(out).expect("utf8");
        // 12 rows = 2 bands -> exactly one '-' band separator
        assert_eq!(s.matches('-').count(), 1);
    }
}
