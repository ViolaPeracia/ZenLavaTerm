//! Kitty Graphics Protocol (APC) high-resolution 24-bit RGB terminal renderer.
//!
//! Supported natively by Kitty, Ghostty, and WezTerm. Renders the `VirtualFramebuffer`
//! directly as an in-place 24-bit RGB image (`i=1, p=1`) behind text cells (`z=-1`) so
//! LavaTerm's top-left fading HUD renders cleanly on top without flicker.

use super::framebuffer::VirtualFramebuffer;
use super::{CursorMode, Renderer};
use std::io::{self, Write};

const B64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const CHUNK_SIZE: usize = 4096;

/// Encodes `input` bytes into RFC 4648 Base64, appending to `out`.
fn encode_base64_into(input: &[u8], out: &mut Vec<u8>) {
    out.clear();
    let encoded_len = input.len().div_ceil(3) * 4;
    out.reserve(encoded_len);

    let mut chunks = input.chunks_exact(3);
    for chunk in &mut chunks {
        let n = ((chunk[0] as u32) << 16) | ((chunk[1] as u32) << 8) | (chunk[2] as u32);
        out.push(B64_TABLE[((n >> 18) & 0x3F) as usize]);
        out.push(B64_TABLE[((n >> 12) & 0x3F) as usize]);
        out.push(B64_TABLE[((n >> 6) & 0x3F) as usize]);
        out.push(B64_TABLE[(n & 0x3F) as usize]);
    }

    match chunks.remainder() {
        [a] => {
            let n = (*a as u32) << 16;
            out.push(B64_TABLE[((n >> 18) & 0x3F) as usize]);
            out.push(B64_TABLE[((n >> 12) & 0x3F) as usize]);
            out.push(b'=');
            out.push(b'=');
        }
        [a, b] => {
            let n = ((*a as u32) << 16) | ((*b as u32) << 8);
            out.push(B64_TABLE[((n >> 18) & 0x3F) as usize]);
            out.push(B64_TABLE[((n >> 12) & 0x3F) as usize]);
            out.push(B64_TABLE[((n >> 6) & 0x3F) as usize]);
            out.push(b'=');
        }
        _ => {}
    }
}

/// Terminal renderer using the Kitty Graphics Protocol (`\x1b_G...\x1b\\`).
#[derive(Debug, Default)]
pub struct KittyRenderer {
    cursor_mode: CursorMode,
    viewport_cols: u16,
    viewport_rows: u16,
    last_rendered_rows: Option<usize>,
    rgb_buf: Vec<u8>,
    b64_buf: Vec<u8>,
}

impl KittyRenderer {
    /// Creates a new `KittyRenderer` in `CursorMode::Absolute`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new `KittyRenderer` with the given cursor mode and target terminal cell size.
    pub fn with_cursor_mode(cursor_mode: CursorMode, cols: u16, rows: u16) -> Self {
        Self {
            cursor_mode,
            viewport_cols: cols,
            viewport_rows: rows,
            ..Self::default()
        }
    }

    /// Updates the target terminal cell dimensions used for terminal-side upscaling (`c=`, `r=`).
    pub fn set_viewport(&mut self, cols: u16, rows: u16) {
        self.viewport_cols = cols;
        self.viewport_rows = rows;
    }

    /// Returns the Kitty Graphics escape sequence that deletes LavaTerm's persistent image ID (`i=1`).
    pub const fn clear_sequence() -> &'static [u8] {
        b"\x1b_Ga=d,d=I,i=1,q=2\x1b\\"
    }
}

impl Renderer for KittyRenderer {
    fn render(&mut self, buffer: &VirtualFramebuffer, writer: &mut dyn Write) -> io::Result<()> {
        if buffer.width == 0 || buffer.height == 0 {
            return Ok(());
        }

        let slice = buffer.as_slice();
        self.rgb_buf.clear();
        self.rgb_buf.reserve(slice.len() * 3);
        for px in slice {
            self.rgb_buf.push(px.r);
            self.rgb_buf.push(px.g);
            self.rgb_buf.push(px.b);
        }

        encode_base64_into(&self.rgb_buf, &mut self.b64_buf);

        let target_rows = if self.viewport_rows > 0 {
            self.viewport_rows as usize
        } else {
            (buffer.height / 6).max(1)
        };

        match self.cursor_mode {
            CursorMode::Absolute => {
                writer.write_all(b"\x1b[1;1H")?;
            }
            CursorMode::Inline => {
                if let Some(prev) = self.last_rendered_rows {
                    if prev > 1 {
                        write!(writer, "\r\x1b[{}A", prev - 1)?;
                    } else if prev == 1 {
                        writer.write_all(b"\r")?;
                    }
                }
            }
            CursorMode::Stream => {}
        }

        let total_chunks = self.b64_buf.len().div_ceil(CHUNK_SIZE);
        for (idx, chunk) in self.b64_buf.chunks(CHUNK_SIZE).enumerate() {
            let more = u8::from(idx + 1 < total_chunks);
            if idx == 0 {
                let no_cursor_move = if self.cursor_mode == CursorMode::Stream {
                    ""
                } else {
                    ",C=1"
                };
                if self.viewport_cols > 0 && self.viewport_rows > 0 {
                    write!(
                        writer,
                        "\x1b_Ga=T,f=24,t=d,i=1,p=1,s={},v={},c={},r={},z=-1{},q=2,m={};",
                        buffer.width,
                        buffer.height,
                        self.viewport_cols,
                        self.viewport_rows,
                        no_cursor_move,
                        more
                    )?;
                } else {
                    write!(
                        writer,
                        "\x1b_Ga=T,f=24,t=d,i=1,p=1,s={},v={},z=-1{},q=2,m={};",
                        buffer.width, buffer.height, no_cursor_move, more
                    )?;
                }
            } else {
                write!(writer, "\x1b_Gm={more};")?;
            }
            writer.write_all(chunk)?;
            writer.write_all(b"\x1b\\")?;
        }

        match self.cursor_mode {
            CursorMode::Inline => {
                for _ in 1..target_rows {
                    writer.write_all(b"\r\n")?;
                }
                self.last_rendered_rows = Some(target_rows);
            }
            CursorMode::Stream => {
                writer.write_all(b"\n")?;
            }
            CursorMode::Absolute => {
                self.last_rendered_rows = Some(target_rows);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::Rgb;

    #[test]
    fn test_base64_rfc4648_vectors() {
        let mut out = Vec::new();
        encode_base64_into(b"", &mut out);
        assert_eq!(out, b"");

        encode_base64_into(b"f", &mut out);
        assert_eq!(out, b"Zg==");

        encode_base64_into(b"fo", &mut out);
        assert_eq!(out, b"Zm8=");

        encode_base64_into(b"foo", &mut out);
        assert_eq!(out, b"Zm9v");

        encode_base64_into(b"foobar", &mut out);
        assert_eq!(out, b"Zm9vYmFy");
    }

    #[test]
    fn test_kitty_single_chunk_absolute_and_viewport() {
        let mut renderer = KittyRenderer::with_cursor_mode(CursorMode::Absolute, 80, 24);
        let fb = VirtualFramebuffer::new(2, 2, Rgb::new(0xFF, 0x3B, 0x00));
        let mut out = Vec::new();
        renderer.render(&fb, &mut out).expect("render succeeds");
        let s = String::from_utf8(out).expect("valid utf8");

        assert!(s.starts_with(
            "\x1b[1;1H\x1b_Ga=T,f=24,t=d,i=1,p=1,s=2,v=2,c=80,r=24,z=-1,C=1,q=2,m=0;"
        ));
        assert!(s.ends_with("\x1b\\"));
    }

    #[test]
    fn test_kitty_multi_chunk_streaming() {
        let mut renderer = KittyRenderer::with_cursor_mode(CursorMode::Stream, 40, 20);
        // 60x40 * 3 = 7200 RGB bytes -> 9600 base64 bytes -> 3 chunks (4096 + 4096 + 1408)
        let fb = VirtualFramebuffer::new(60, 40, Rgb::new(10, 20, 30));
        let mut out = Vec::new();
        renderer.render(&fb, &mut out).expect("render succeeds");
        let s = String::from_utf8(out).expect("valid utf8");

        assert!(s.contains(",m=1;"));
        assert!(s.contains("\x1b_Gm=1;"));
        assert!(s.contains("\x1b_Gm=0;"));
        assert!(s.ends_with("\x1b\\\n"));
        assert!(!s.contains("C=1"));
    }

    #[test]
    fn test_kitty_inline_rewind_and_clear_sequence() {
        let mut renderer = KittyRenderer::with_cursor_mode(CursorMode::Inline, 20, 6);
        let fb = VirtualFramebuffer::new(4, 4, Rgb::new(1, 2, 3));
        let mut out1 = Vec::new();
        renderer.render(&fb, &mut out1).expect("first inline frame");
        let s1 = String::from_utf8(out1).expect("utf8");
        assert!(!s1.contains("\x1b[5A"));
        assert_eq!(s1.matches("\r\n").count(), 5);

        let mut out2 = Vec::new();
        renderer
            .render(&fb, &mut out2)
            .expect("second inline frame");
        let s2 = String::from_utf8(out2).expect("utf8");
        assert!(s2.starts_with("\r\x1b[5A"));

        assert_eq!(
            KittyRenderer::clear_sequence(),
            b"\x1b_Ga=d,d=I,i=1,q=2\x1b\\"
        );
    }
}
