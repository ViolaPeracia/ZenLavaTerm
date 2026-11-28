//! Full-block (`█`) fallback terminal renderer.

use super::color::Rgb;
use super::framebuffer::VirtualFramebuffer;
use super::{CursorMode, Renderer};
use std::io::{self, Write};

/// Terminal renderer using full block characters (`█`).
#[derive(Debug, Default)]
pub struct BlockRenderer {
    last_fg: Option<Rgb>,
    cursor_mode: CursorMode,
    last_rendered_rows: Option<usize>,
}

impl BlockRenderer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `BlockRenderer` with the specified cursor positioning mode.
    pub fn with_cursor_mode(cursor_mode: CursorMode) -> Self {
        Self {
            cursor_mode,
            ..Self::default()
        }
    }
}

impl Renderer for BlockRenderer {
    fn render(&mut self, buffer: &VirtualFramebuffer, writer: &mut dyn Write) -> io::Result<()> {
        let term_rows = buffer.height;
        let term_cols = buffer.width;
        let slice = buffer.as_slice();

        self.last_fg = None;

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
            let row_start = row * term_cols;
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
                let color = slice[row_start + col];

                if self.last_fg != Some(color) {
                    write!(writer, "\x1b[38;2;{};{};{}m", color.r, color.g, color.b)?;
                    self.last_fg = Some(color);
                }

                writer.write_all("█".as_bytes())?;
            }

            if self.cursor_mode == CursorMode::Inline {
                writer.write_all(b"\x1b[0m\x1b[K")?;
                self.last_fg = None;
            }
        }

        if self.cursor_mode == CursorMode::Inline && prev_rows.is_some_and(|prev| term_rows < prev)
        {
            writer.write_all(b"\x1b[J")?;
        }

        write!(writer, "\x1b[0m")?;
        self.last_rendered_rows = Some(term_rows);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_render_output() {
        let mut renderer = BlockRenderer::new();
        let fb = VirtualFramebuffer::new(2, 2, Rgb::new(0, 0, 255));

        let mut output = Vec::new();
        renderer.render(&fb, &mut output).expect("Render succeeds");

        let text = String::from_utf8_lossy(&output);
        assert!(text.contains("█"));
        assert!(text.contains("\x1b[0m"));
    }

    #[test]
    fn test_inline_mode_tracks_previous_height_across_resizes() {
        let mut renderer = BlockRenderer::with_cursor_mode(CursorMode::Inline);
        let fb4 = VirtualFramebuffer::new(2, 4, Rgb::new(0, 0, 0));
        let fb2 = VirtualFramebuffer::new(2, 2, Rgb::new(0, 0, 0));
        let fb5 = VirtualFramebuffer::new(2, 5, Rgb::new(0, 0, 0));

        let mut out1 = Vec::new();
        renderer.render(&fb4, &mut out1).unwrap();
        assert!(!String::from_utf8_lossy(&out1).contains("\x1b[1;1H"));

        let mut out2 = Vec::new();
        renderer.render(&fb2, &mut out2).unwrap();
        let t2 = String::from_utf8_lossy(&out2);
        assert!(t2.starts_with("\r\x1b[3A"));
        assert!(t2.contains("\x1b[K"));
        assert!(t2.contains("\x1b[J"));

        let mut out3 = Vec::new();
        renderer.render(&fb5, &mut out3).unwrap();
        let t3 = String::from_utf8_lossy(&out3);
        assert!(t3.starts_with("\r\x1b[1A"));
        assert!(!t3.contains("\x1b[J"));
    }
}
