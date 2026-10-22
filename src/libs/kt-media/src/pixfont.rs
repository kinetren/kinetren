//! Built-in 3×5 pixel font and a canvas for procedural frames.
//!
//! Factored out of placeholder.rs (R.52): the same primitives are needed by generators
//! (the timecode draws digits). The font depends neither on system fonts nor on the
//! egui renderer — the frame is deterministic and the same in preview, export and
//! tests (t208). Characters outside the table are shown as “?”. This is NOT the
//! rasterization of user title text — that lives in kt-text with its own
//! determinism discipline.

pub(crate) const GLYPH_W: usize = 3;
pub(crate) const GLYPH_H: usize = 5;

/// 3×5 font: glyph rows are separated by '/', '#' — a pixel. Lowercase letters
/// are converted to uppercase on drawing.
static FONT: &[(char, &str)] = &[
    (' ', ".../.../.../.../..."),
    ('0', "###/#.#/#.#/#.#/###"),
    ('1', ".#./##./.#./.#./###"),
    ('2', "###/..#/###/#../###"),
    ('3', "###/..#/###/..#/###"),
    ('4', "#.#/#.#/###/..#/..#"),
    ('5', "###/#../###/..#/###"),
    ('6', "###/#../###/#.#/###"),
    ('7', "###/..#/..#/..#/..#"),
    ('8', "###/#.#/###/#.#/###"),
    ('9', "###/#.#/###/..#/###"),
    ('A', "###/#.#/###/#.#/#.#"),
    ('B', "##./#.#/##./#.#/##."),
    ('C', "###/#../#../#../###"),
    ('D', "##./#.#/#.#/#.#/##."),
    ('E', "###/#../###/#../###"),
    ('F', "###/#../###/#../#.."),
    ('G', "###/#../#.#/#.#/###"),
    ('H', "#.#/#.#/###/#.#/#.#"),
    ('I', "###/.#./.#./.#./###"),
    ('J', "..#/..#/..#/#.#/###"),
    ('K', "#.#/#.#/##./#.#/#.#"),
    ('L', "#../#../#../#../###"),
    ('M', "#.#/###/###/#.#/#.#"),
    ('N', "#.#/##./###/.##/#.#"),
    ('O', "###/#.#/#.#/#.#/###"),
    ('P', "###/#.#/###/#../#.."),
    ('Q', "###/#.#/#.#/###/..#"),
    ('R', "###/#.#/###/##./#.#"),
    ('S', "###/#../###/..#/###"),
    ('T', "###/.#./.#./.#./.#."),
    ('U', "#.#/#.#/#.#/#.#/###"),
    ('V', "#.#/#.#/#.#/#.#/.#."),
    ('W', "#.#/#.#/###/###/#.#"),
    ('X', "#.#/#.#/.#./#.#/#.#"),
    ('Y', "#.#/#.#/.#./.#./.#."),
    ('Z', "###/..#/.#./#../###"),
    ('.', ".../.../.../.../.#."),
    (',', ".../.../.../.#./#.."),
    ('-', ".../.../###/.../..."),
    ('_', ".../.../.../.../###"),
    (':', ".../.#./.../.#./..."),
    (';', ".../.#./.../.#./#.."),
    ('\'', ".#./.#./.../.../..."),
    ('"', "#.#/#.#/.../.../..."),
    ('(', "..#/.#./.#./.#./..#"),
    (')', "#../.#./.#./.#./#.."),
    ('[', ".##/.#./.#./.#./.##"),
    (']', "##./.#./.#./.#./##."),
    ('{', ".##/.#./##./.#./.##"),
    ('}', "##./.#./.##/.#./##."),
    ('/', "..#/..#/.#./#../#.."),
    ('\\', "#../#../.#./..#/..#"),
    ('+', ".../.#./###/.#./..."),
    ('=', ".../###/.../###/..."),
    ('!', ".#./.#./.#./.../.#."),
    ('?', "###/..#/.#./.../.#."),
    ('#', "#.#/###/#.#/###/#.#"),
    ('@', "###/#.#/###/#../.##"),
    ('%', "#.#/..#/.#./#../#.#"),
    ('&', "##./##./###/#.#/.##"),
    ('*', "#.#/.#./#.#/.../..."),
    ('~', ".../.##/##./.../..."),
    ('$', ".#./###/##./.##/.#."),
    ('<', "..#/.#./#../.#./..#"),
    ('>', "#../.#./..#/.#./#.."),
    ('|', ".#./.#./.#./.#./.#."),
    ('^', ".#./#.#/.../.../..."),
    ('`', "#../.#./.../.../..."),
];

fn glyph(c: char) -> &'static str {
    let up = c.to_ascii_uppercase();
    FONT.iter()
        .find(|(g, _)| *g == up)
        .or_else(|| FONT.iter().find(|(g, _)| *g == '?'))
        .map(|(_, rows)| *rows)
        .unwrap_or(".../.../.../.../...")
}

/// Frame canvas: an RGBA buffer and its dimensions. A separate type instead of a
/// bundle of parameters in each drawing routine — otherwise they would take eight arguments each.
pub(crate) struct Canvas {
    pub(crate) buf: Vec<u8>,
    pub(crate) w: i64,
    pub(crate) h: i64,
}

impl Canvas {
    pub(crate) fn new(w: u32, h: u32, bg: [u8; 4]) -> Canvas {
        let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
        for px in buf.chunks_exact_mut(4) {
            px.copy_from_slice(&bg);
        }
        Canvas { buf, w: w as i64, h: h as i64 }
    }

    pub(crate) fn rect(&mut self, x0: i64, y0: i64, rw: i64, rh: i64, c: [u8; 4]) {
        for y in y0.max(0)..(y0 + rh).min(self.h) {
            for x in x0.max(0)..(x0 + rw).min(self.w) {
                let i = ((y * self.w + x) * 4) as usize;
                self.buf[i..i + 4].copy_from_slice(&c);
            }
        }
    }

    /// A string in the built-in font at scale `s`, top-left corner (x, y).
    pub(crate) fn text(&mut self, x: i64, y: i64, s: i64, text: &str, c: [u8; 4]) {
        let mut cx = x;
        for ch in text.chars() {
            for (ry, row) in glyph(ch).split('/').enumerate() {
                for (rx, px) in row.chars().enumerate() {
                    if px == '#' {
                        self.rect(cx + rx as i64 * s, y + ry as i64 * s, s, s, c);
                    }
                }
            }
            cx += (GLYPH_W as i64 + 1) * s;
        }
    }

    /// Film icon: a frame border with perforations along the edges.
    pub(crate) fn film(&mut self, x: i64, y: i64, s: i64, c: [u8; 4]) {
        let (fw, fh) = (14 * s, 10 * s);
        self.rect(x, y, fw, s, c);
        self.rect(x, y + fh - s, fw, s, c);
        self.rect(x, y, s, fh, c);
        self.rect(x + fw - s, y, s, fh, c);
        for i in 0..4 {
            let px = x + s * 2 + i * 3 * s;
            self.rect(px, y + s * 2, s, s, c);
            self.rect(px, y + fh - s * 3, s, s, c);
        }
    }
}

pub(crate) fn text_width(len: usize, s: i64) -> i64 {
    if len == 0 { 0 } else { (len as i64 * (GLYPH_W as i64 + 1) - 1) * s }
}
