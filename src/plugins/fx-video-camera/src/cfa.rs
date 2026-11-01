//! The colour filter arrays (design step 6): one table per layout, the codes below, and the packing
//! into four u32 that both the CPU passes and the WGSL read (D3), so the shaders carry no layout code.

/// The colour of a sensor pixel.
pub const R: u32 = 0;
pub const G: u32 = 1;
pub const B: u32 = 2;
/// Panchromatic (no dye): RGBW.
pub const W: u32 = 3;
/// An infrared pixel (a visible-blocking, NIR-passing filter): RGB-IR.
pub const IR: u32 = 4;

/// The layouts in the order of the `cfa` choice row.
pub const LAYOUTS: &[&str] =
    &["rggb", "bggr", "grbg", "gbrg", "quad_bayer", "xtrans", "rgbw", "rgbir"];

/// A layout's period and its cells, row by row (`cells[y * period + x]`).
pub struct Layout {
    pub period: u32,
    pub cells: &'static [u32],
}

const RGGB: [u32; 4] = [R, G, G, B];
const BGGR: [u32; 4] = [B, G, G, R];
const GRBG: [u32; 4] = [G, R, B, G];
const GBRG: [u32; 4] = [G, B, R, G];
/// Quad Bayer: each Bayer cell is a 2×2 block of one colour.
#[rustfmt::skip]
const QUAD: [u32; 16] = [
    R, R, G, G,
    R, R, G, G,
    G, G, B, B,
    G, G, B, B,
];
/// Fujifilm X-Trans, the 6×6 of dcraw (`xtrans[6][6]`): 20 G, 8 R, 8 B; every row and column holds
/// all three colours.
#[rustfmt::skip]
const XTRANS: [u32; 36] = [
    G, G, R, G, G, B,
    G, G, B, G, G, R,
    B, R, G, R, B, G,
    G, G, B, G, G, R,
    G, G, R, G, G, B,
    R, B, G, B, R, G,
];
/// RGBW, the Kodak CFA2.0 checkerboard: W on half the sites, the colours on the other half.
#[rustfmt::skip]
const RGBW: [u32; 16] = [
    W, B, W, G,
    B, W, G, W,
    W, G, W, R,
    G, W, R, W,
];
/// RGB-IR, the 4×4 of OmniVision's RGB-IR sensors: IR at odd/odd, G on the other odd sites.
#[rustfmt::skip]
const RGBIR: [u32; 16] = [
    B, G, R, G,
    G, IR, G, IR,
    R, G, B, G,
    G, IR, G, IR,
];

/// The layout of choice index `i` (clamped).
pub fn layout(i: usize) -> Layout {
    match i.min(LAYOUTS.len() - 1) {
        0 => Layout { period: 2, cells: &RGGB },
        1 => Layout { period: 2, cells: &BGGR },
        2 => Layout { period: 2, cells: &GRBG },
        3 => Layout { period: 2, cells: &GBRG },
        4 => Layout { period: 4, cells: &QUAD },
        5 => Layout { period: 6, cells: &XTRANS },
        6 => Layout { period: 4, cells: &RGBW },
        _ => Layout { period: 4, cells: &RGBIR },
    }
}

/// The table packed into four u32, 3 bits a cell, ten cells a word; `(dx, dy)` shifts the phase
/// (the ISP's assumption: the cell at (x, y) is the sensor's cell at (x+dx, y+dy)).
pub fn pack(l: &Layout, dx: u32, dy: u32) -> [u32; 4] {
    let p = l.period;
    let mut out = [0u32; 4];
    for y in 0..p {
        for x in 0..p {
            let c = l.cells[(((y + dy) % p) * p + (x + dx) % p) as usize];
            let i = (y * p + x) as usize;
            out[i / 10] |= c << ((i % 10) * 3);
        }
    }
    out
}

/// The code at sensor pixel (x, y) of a packed table — the ONE text, mirrored by `cfa_code` in
/// `common.wgsl`.
#[inline(always)]
pub fn code(tab: &[u32; 4], period: u32, x: i32, y: i32) -> u32 {
    let p = period as i32;
    let i = (y.rem_euclid(p) * p + x.rem_euclid(p)) as usize;
    (tab[i / 10] >> ((i % 10) * 3)) & 7
}

/// Whether a packed table is a 2×2 Bayer (the Malvar–He–Cutler kernels apply exactly).
pub fn is_bayer(l: &Layout) -> bool {
    l.period == 2
}

/// Whether a layout has infrared pixels.
pub fn has_ir(l: &Layout) -> bool {
    l.cells.contains(&IR)
}
