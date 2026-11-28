//! High-resolution Half-Block (`▀` / `▄`) terminal renderer.

use super::color::Rgb;
use super::framebuffer::VirtualFramebuffer;
use super::{CursorMode, Renderer};
use std::io::{self, Write};

/// Terminal renderer that packs two vertical virtual pixels into a single character cell using `▀`.
#[derive(Debug, Default)]
pub struct HalfBlockRenderer {
    last_fg: Option<Rgb>,
    last_bg: Option<Rgb>,
    cursor_mode: CursorMode,
    last_rendered_rows: Option<usize>,
}

impl HalfBlockRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `HalfBlockRenderer` with the specified cursor positioning mode.
    pub fn with_cursor_mode(cursor_mode: CursorMode) -> Self {
        Self {
            cursor_mode,
            ..Self::default()
        }
    }
}

impl Renderer for HalfBlockRenderer {
    fn render(&mut self, buffer: &VirtualFramebuffer, writer: &mut dyn Write) -> io::Result<()> {
        let term_rows = buffer.height / 2;
        let term_cols = buffer.width;
        let slice = buffer.as_slice();

        self.last_fg = None;
        self.last_bg = None;

        let prev_rows = self.last_rendered_rows;
        if self.cursor_mode == CursorMode::Inline {
            if let Some(prev) = prev_rows {
                if prev > 1 {
                    write!(writer, "\r\x1b[{}A", prev - 1)?;
                } else if prev == 1 {
                    writer.write_all(b"\r")?;
                }
            }
        }

        for row in 0..term_rows {
            let top_row_start = (row * 2) * term_cols;
            let btm_row_start = (row * 2 + 1) * term_cols;

            match self.cursor_mode {
                CursorMode::Absolute => write!(writer, "\x1b[{};1H", row + 1)?,
                CursorMode::Inline => {
                    if row > 0 {
                        writer.write_all(b"\r\n")?;
                    }
                }
                CursorMode::Stream => {
                    if row > 0 {
                        writer.write_all(b"\n")?;
                    }
                }
            }

            for col in 0..term_cols {
                let top_col = slice[top_row_start + col];
                let btm_col = slice[btm_row_start + col];

                if self.last_fg != Some(top_col) {
                    write!(
                        writer,
                        "\x1b[38;2;{};{};{}m",
                        top_col.r, top_col.g, top_col.b
                    )?;
                    self.last_fg = Some(top_col);
                }

                if self.last_bg != Some(btm_col) {
                    write!(
                        writer,
                        "\x1b[48;2;{};{};{}m",
                        btm_col.r, btm_col.g, btm_col.b
                    )?;
                    self.last_bg = Some(btm_col);
                }

                writer.write_all("▀".as_bytes())?;
            }

            if self.cursor_mode == CursorMode::Inline {
                writer.write_all(b"\x1b[0m\x1b[K")?;
                self.last_fg = None;
                self.last_bg = None;
            }
        }

        if self.cursor_mode == CursorMode::Inline && prev_rows.is_some_and(|prev| term_rows < prev)
        {
            writer.write_all(b"\x1b[J")?;
        }

        // Reset styling at the end of the frame
        write!(writer, "\x1b[0m")?;
        self.last_rendered_rows = Some(term_rows);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_halfblock_render_output() {
        let mut renderer = HalfBlockRenderer::new();
        let mut fb = VirtualFramebuffer::new(2, 2, Rgb::new(255, 0, 0));
        fb.set_pixel(0, 1, Rgb::new(0, 255, 0));

        let mut output = Vec::new();
        renderer.render(&fb, &mut output).expect("Render succeeds");

        let text = String::from_utf8_lossy(&output);
        assert!(text.contains("▀"));
        assert!(text.contains("\x1b[0m"));
    }

    #[test]
    fn test_inline_mode_tracks_previous_height_across_resizes() {
        let mut renderer = HalfBlockRenderer::with_cursor_mode(CursorMode::Inline);
        let fb4 = VirtualFramebuffer::new(2, 8, Rgb::new(0, 0, 0)); // 4 rows
        let fb2 = VirtualFramebuffer::new(2, 4, Rgb::new(0, 0, 0)); // 2 rows
        let fb5 = VirtualFramebuffer::new(2, 10, Rgb::new(0, 0, 0)); // 5 rows

        let mut out1 = Vec::new();
        renderer.render(&fb4, &mut out1).unwrap();
        let t1 = String::from_utf8_lossy(&out1);
        assert!(!t1.contains("\x1b[1;1H"));
        assert!(t1.contains("\x1b[K"));

        // Shrink 4 -> 2 rows: must rewind by prev_rows-1 = 3 and clear below with \x1b[J
        let mut out2 = Vec::new();
        renderer.render(&fb2, &mut out2).unwrap();
        let t2 = String::from_utf8_lossy(&out2);
        assert!(t2.starts_with("\r\x1b[3A"));
        assert!(t2.contains("\x1b[J"));

        // Grow 2 -> 5 rows: must rewind by prev_rows-1 = 1 and NOT emit \x1b[J
        let mut out3 = Vec::new();
        renderer.render(&fb5, &mut out3).unwrap();
        let t3 = String::from_utf8_lossy(&out3);
        assert!(t3.starts_with("\r\x1b[1A"));
        assert!(!t3.contains("\x1b[J"));
    }
}
