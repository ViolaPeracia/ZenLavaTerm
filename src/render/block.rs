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
    has_rendered: bool,
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

        if self.cursor_mode == CursorMode::Inline && self.has_rendered {
            if term_rows > 1 {
                write!(writer, "\r\x1b[{}A", term_rows - 1)?;
            } else if term_rows == 1 {
                writer.write_all(b"\r")?;
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
        }

        write!(writer, "\x1b[0m")?;
        self.has_rendered = true;
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
    fn test_inline_mode_omits_absolute_row1_move() {
        let mut renderer = BlockRenderer::with_cursor_mode(super::super::CursorMode::Inline);
        let fb = VirtualFramebuffer::new(2, 2, Rgb::new(0, 0, 0));

        let mut first = Vec::new();
        renderer
            .render(&fb, &mut first)
            .expect("First render succeeds");
        let first_text = String::from_utf8_lossy(&first);
        assert!(!first_text.contains("\x1b[1;1H"));
        assert!(!first_text.contains("\x1b[2;1H"));
        assert!(first_text.contains("\r\n"));

        let mut second = Vec::new();
        renderer
            .render(&fb, &mut second)
            .expect("Second render succeeds");
        let second_text = String::from_utf8_lossy(&second);
        assert!(second_text.starts_with("\r\x1b[1A"));
        assert!(!second_text.contains("\x1b[1;1H"));
    }
}
