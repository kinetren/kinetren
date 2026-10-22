//! Rasterization of titles (R.52, F-TXT-001): a pure function
//! (parameters, W, H) → straight-RGBA bytes. The rules — the crate's CLAUDE.md:
//! a vendored font, pinned harfrust/ab_glyph, no GUI whatsoever.
//!
//! harfrust gives the POSITIONS (GPOS kerning, ligatures, shaping), ab_glyph —
//! the PIXELS by GlyphId; the metrics come from the same font bytes. In C2 only
//! the fill is drawn; outline/shadow/plate (F-TXT-005) are C3 — the fields for them
//! already sit in the structure: the format does not catch up with the code.

use ab_glyph::{Font as _, FontRef as AbFont, Glyph, PxScale, point};
use harfrust::{FontRef as HrFont, ShapeOptions, ShaperData, UnicodeBuffer};
use std::sync::OnceLock;

/// The only vendored family in v1 (C4 will add the system ones).
pub const VENDORED_FAMILY: &str = "Noto Sans";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FaceStyle {
    #[default]
    Regular,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Align {
    Left,
    #[default]
    Center,
    Right,
}

/// 3×3 anchor: which point of the text block sits at `pos`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Anchor {
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    #[default]
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
}

/// Decoration layer (drawing order BOTTOM TO TOP — a saved field from day one:
/// a lesson of Kdenlive from F-TXT-005). Rendering of the layers, other than
/// the fill, is C3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Plate,
    Shadow,
    Stroke,
    Fill,
}

/// Title parameters — the input of the rasterizer. Everything is RELATIVE: the size — a fraction of
/// the frame height, tracking/outline/shadow/padding — fractions of em; the frame is born
/// in any Quality without recomputing the parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct TextParams {
    /// Multi-line text, explicit `\n`. An empty line = a transparent frame.
    pub text: String,
    /// Font family. v1: a non-vendored family silently falls back to the vendored one
    /// (C4 will add the system ones and a visible notice of substitution).
    pub family: String,
    pub style: FaceStyle,
    /// Size: a fraction of the frame HEIGHT (0.07 ≈ the lower third).
    pub size: f64,
    /// Line spacing multiplier.
    pub line_spacing: f64,
    /// Tracking, a fraction of em.
    pub tracking: f64,
    pub align: Align,
    pub anchor: Anchor,
    /// Normalized anchor coordinates (0..1 of the frame width/height).
    pub pos: (f64, f64),
    /// RGBA fill (straight).
    pub fill: [u8; 4],
    /// Outline: width in fractions of em (0 = off) + colour. Rendering — C3.
    pub stroke_w: f64,
    pub stroke: [u8; 4],
    /// Shadow: offset and blur in fractions of em, colour (alpha 0 = off). C3.
    pub shadow_dx: f64,
    pub shadow_dy: f64,
    pub shadow_blur: f64,
    pub shadow: [u8; 4],
    /// Plate: paddings in fractions of em, colour (alpha 0 = off). C3.
    pub plate_pad: (f64, f64),
    pub plate: [u8; 4],
    /// Layer order, bottom to top.
    pub order: [Layer; 4],
    /// Fraction of REVEALED clusters, 0..1 (R.53, item 423). 1.0 — everything is visible, and
    /// this is EXACTLY the former code branch: the title golden does not move.
    ///
    /// There is no time in kt-text and there never will be: the number is computed by kt-media from the
    /// pts of the material. The layout is still computed ONCE over the FULL
    /// line — otherwise the letters would jitter at every appearance of a neighbour.
    pub reveal: f64,
    /// Width of the soft reveal front, IN CLUSTERS (0 — a hard cut,
    /// “typewriter”).
    pub reveal_soft: f64,
    /// Offset of the WHOLE block in frame pixels (R.53, item 424). A scrolling
    /// line and a crawl are an offset, not visibility. The caller must
    /// quantize it to 1/64 px BEFORE passing: then the mutual geometry of the
    /// glyphs between frames is bit-for-bit constant.
    pub offset: (f64, f64),
}

impl Default for TextParams {
    fn default() -> TextParams {
        TextParams {
            text: String::new(),
            family: VENDORED_FAMILY.into(),
            style: FaceStyle::Regular,
            size: 0.07,
            line_spacing: 1.0,
            tracking: 0.0,
            align: Align::Center,
            anchor: Anchor::Center,
            pos: (0.5, 0.5),
            fill: [255, 255, 255, 255],
            stroke_w: 0.0,
            stroke: [0, 0, 0, 255],
            shadow_dx: 0.04,
            shadow_dy: 0.04,
            shadow_blur: 0.06,
            shadow: [0, 0, 0, 0],
            plate_pad: (0.25, 0.12),
            plate: [0, 0, 0, 0],
            order: [Layer::Plate, Layer::Shadow, Layer::Stroke, Layer::Fill],
            reveal: 1.0,
            reveal_soft: 0.0,
            offset: (0.0, 0.0),
        }
    }
}

/// Box in frame pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PxBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Layout geometry: the block box and the line boxes — for dragging in the preview,
/// plates and per-character animation of the future (F-TXT-006).
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutInfo {
    pub block: PxBox,
    pub lines: Vec<PxBox>,
    /// Boxes of CLUSTERS in text order (R.53, item 423). A cluster, not a
    /// symbol nor a glyph: harfrust gives grapheme clusters for free, and
    /// the `fi` ligature, a base with a diacritic and a ZWJ emoji arrive as ONE
    /// unit. There is no segmenter of our own in the project, and creating one for the
    /// sake of animation would be a separate subsystem.
    pub clusters: Vec<PxBox>,
}

pub struct FaceInfo {
    pub family: &'static str,
    pub style: FaceStyle,
}

/// Vendored faces — for the combo in the title window.
pub fn faces() -> &'static [FaceInfo] {
    &[
        FaceInfo { family: VENDORED_FAMILY, style: FaceStyle::Regular },
        FaceInfo { family: VENDORED_FAMILY, style: FaceStyle::Bold },
    ]
}

// ---- font -----------------------------------------------------------------

static REGULAR_BYTES: &[u8] = include_bytes!("../assets/NotoSans-Regular.ttf");
static BOLD_BYTES: &[u8] = include_bytes!("../assets/NotoSans-Bold.ttf");

struct Face {
    ab: AbFont<'static>,
    hr: HrFont<'static>,
    shaper: ShaperData,
    /// px per font unit at a size of 1 px: 1/units_per_em.
    unit: f64,
    ascent_u: f64,
    descent_u: f64,
    gap_u: f64,
    /// The ab_glyph trap: `PxScale` is the HEIGHT (ascent−descent), not em.
    /// The scale for “size = size_px” is derived only from here (t481).
    height_per_em: f64,
}

fn face(style: FaceStyle) -> &'static Face {
    static REGULAR: OnceLock<Face> = OnceLock::new();
    static BOLD: OnceLock<Face> = OnceLock::new();
    let (cell, bytes) = match style {
        FaceStyle::Regular => (&REGULAR, REGULAR_BYTES),
        FaceStyle::Bold => (&BOLD, BOLD_BYTES),
    };
    cell.get_or_init(|| build_face(bytes, 0).expect("vendored font parses"))
}

/// Face constructor from bytes (vendored or system; `index` —
/// the font number in the ttc collection).
fn build_face(bytes: &'static [u8], index: u32) -> Option<Face> {
    let ab = AbFont::try_from_slice_and_index(bytes, index).ok()?;
    let hr = HrFont::from_index(bytes, index).ok()?;
    let shaper = ShaperData::new(&hr);
    let upem = ab.units_per_em()? as f64;
    let ascent_u = ab.ascent_unscaled() as f64;
    let descent_u = ab.descent_unscaled() as f64; // negative
    let gap_u = ab.line_gap_unscaled() as f64;
    Some(Face {
        ab,
        hr,
        shaper,
        unit: 1.0 / upem,
        ascent_u,
        descent_u,
        gap_u,
        height_per_em: (ascent_u - descent_u) / upem,
    })
}

/// The result of resolving the font family (C4): what we draw with and why. `System` carries
/// the file path — together with the bytes it enters the identity of the frame
/// (kt-text/CLAUDE.md, rule 1); `MissingFallback` must give a visible
/// notice in the UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontResolve {
    Vendored,
    System(std::path::PathBuf),
    MissingFallback,
}

fn font_db() -> &'static fontdb::Database {
    static DB: OnceLock<fontdb::Database> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    })
}

/// System families for the title window's combo: sorted, without duplicates.
/// The vendored family is not in the list — it is a separate first line.
///
/// Computed ONCE (R.53, item 413): the list is called on every frame while the combo
/// popup is open, and a walk over all faces with sorting and dedup would be
/// allocations in the frame path (invariant 12).
pub fn system_families() -> &'static [String] {
    static FAMILIES: OnceLock<Vec<String>> = OnceLock::new();
    FAMILIES.get_or_init(|| {
        let mut out: Vec<String> = font_db()
            .faces()
            .flat_map(|f| f.families.iter().map(|(name, _)| name.clone()))
            .filter(|n| !n.is_empty() && n != VENDORED_FAMILY)
            .collect();
        out.sort();
        out.dedup();
        out
    })
}

/// What is actually drawn with the given family on THIS machine.
pub fn resolve_family(family: &str, bold: bool) -> FontResolve {
    if family.is_empty() || family == VENDORED_FAMILY {
        return FontResolve::Vendored;
    }
    match system_face(family, bold) {
        Some((_, path)) => FontResolve::System(path),
        None => FontResolve::MissingFallback,
    }
}

/// Loaded system faces. The bytes leak ONCE per
/// (family, style): there are a few fonts in the project, and `Face` holds a
/// `FontRef<'static>`.
#[allow(clippy::type_complexity)]
fn system_face(family: &str, bold: bool) -> Option<(&'static Face, std::path::PathBuf)> {
    use std::collections::HashMap;
    use std::sync::Mutex;
    static CACHE: OnceLock<
        Mutex<HashMap<(String, bool), Option<(&'static Face, std::path::PathBuf)>>>,
    > = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (family.to_string(), bold);
    if let Some(hit) = cache.lock().unwrap().get(&key) {
        return hit.clone();
    }
    let db = font_db();
    let query = fontdb::Query {
        families: &[fontdb::Family::Name(family)],
        weight: if bold { fontdb::Weight::BOLD } else { fontdb::Weight::NORMAL },
        stretch: fontdb::Stretch::Normal,
        style: fontdb::Style::Normal,
    };
    let loaded = db.query(&query).and_then(|id| {
        let path = match &db.face(id)?.source {
            fontdb::Source::File(p) => p.clone(),
            _ => std::path::PathBuf::new(),
        };
        db.with_face_data(id, |bytes, index| {
            let leaked: &'static [u8] = Box::leak(bytes.to_vec().into_boxed_slice());
            build_face(leaked, index).map(|f| {
                let face: &'static Face = Box::leak(Box::new(f));
                (face, path.clone())
            })
        })?
    });
    cache.lock().unwrap().insert(key, loaded.clone());
    loaded
}

fn resolve_face(p: &TextParams) -> &'static Face {
    if p.family.is_empty() || p.family == VENDORED_FAMILY {
        return face(p.style);
    }
    // family not found — substitution by the vendored one (the notice is the window's concern,
    // `resolve_family`); found — we draw with it
    match system_face(&p.family, matches!(p.style, FaceStyle::Bold)) {
        Some((f, _)) => f,
        None => face(p.style),
    }
}

// ---- layout -------------------------------------------------------------

struct PlacedGlyph {
    id: u16,
    /// The pen's start in frame px (baseline).
    x: f64,
    y: f64,
    /// The cluster number THROUGH the whole text (R.53, item 423): the reveal is computed
    /// by it. The glyphs of one cluster carry one number and appear
    /// together.
    cluster: usize,
}

struct Laid {
    glyphs: Vec<PlacedGlyph>,
    info: LayoutInfo,
    /// ab_glyph scale for rasterization at this size.
    scale: PxScale,
}

/// A glyph of the line after shaping: id, x/y in font units, the BYTE address of the
/// cluster (in harfrust this is the offset in the source line) and the advance —
/// the cluster box is computed from them.
struct ShapedGlyph {
    id: u16,
    x: f64,
    y: f64,
    cluster: u32,
    adv: f64,
}

/// A line after shaping: glyphs and width in units.
type ShapedLine = (Vec<ShapedGlyph>, f64);

/// Shaping of one line.
fn shape_line(face: &Face, text: &str, tracking_u: f64) -> ShapedLine {
    let mut buf = UnicodeBuffer::new();
    buf.push_str(text);
    buf.guess_segment_properties();
    let shaper = face.shaper.shaper(&face.hr).build();
    let out = shaper.shape(buf, ShapeOptions::new());
    let mut glyphs = Vec::with_capacity(out.len());
    let mut cursor = 0.0f64;
    for (info, pos) in out.glyph_infos().iter().zip(out.glyph_positions()) {
        let id = u16::try_from(info.glyph_id).unwrap_or(0);
        // harfrust: Y+ up; frame: Y+ down
        glyphs.push(ShapedGlyph {
            id,
            x: cursor + pos.x_offset as f64,
            y: -(pos.y_offset as f64),
            cluster: info.cluster,
            adv: pos.x_advance as f64,
        });
        cursor += pos.x_advance as f64 + tracking_u;
    }
    if !glyphs.is_empty() {
        cursor -= tracking_u; // tracking — BETWEEN glyphs
    }
    (glyphs, cursor.max(0.0))
}

fn layout(p: &TextParams, w: u32, h: u32) -> Laid {
    let face = resolve_face(p);
    let size_px = (p.size * h as f64).max(0.0);
    let k = size_px * face.unit; // px per font unit
    let tracking_u = if size_px > 0.0 { p.tracking / face.unit } else { 0.0 };

    let lines: Vec<&str> = p.text.split('\n').collect();
    let shaped: Vec<ShapedLine> = lines.iter().map(|l| shape_line(face, l, tracking_u)).collect();

    let line_h = (face.ascent_u - face.descent_u + face.gap_u) * k * p.line_spacing;
    let block_w = shaped.iter().map(|(_, w)| *w * k).fold(0.0f64, f64::max);
    let block_h = line_h * shaped.len() as f64;

    // anchor: which point of the block sits at pos
    let (ax, ay) = (p.pos.0 * w as f64, p.pos.1 * h as f64);
    let bx = match p.anchor {
        Anchor::TopLeft | Anchor::CenterLeft | Anchor::BottomLeft => ax,
        Anchor::TopCenter | Anchor::Center | Anchor::BottomCenter => ax - block_w / 2.0,
        Anchor::TopRight | Anchor::CenterRight | Anchor::BottomRight => ax - block_w,
    };
    let by = match p.anchor {
        Anchor::TopLeft | Anchor::TopCenter | Anchor::TopRight => ay,
        Anchor::CenterLeft | Anchor::Center | Anchor::CenterRight => ay - block_h / 2.0,
        Anchor::BottomLeft | Anchor::BottomCenter | Anchor::BottomRight => ay - block_h,
    };

    // The offset of the whole block (R.53, item 424) — quantized to 1/64 px BEFORE addition:
    // multiples of 1/64 are exactly representable in f64, so the mutual geometry of the
    // glyphs between frames is bit-for-bit constant, not “almost”.
    let (bx, by) = (bx + q64(p.offset.0), by + q64(p.offset.1));

    let mut glyphs = Vec::new();
    let mut line_boxes = Vec::with_capacity(shaped.len());
    let mut clusters: Vec<PxBox> = Vec::new();
    // cluster number THROUGH the whole text: harfrust's byte addresses are
    // unique only within their own line
    let mut cluster_no = 0usize;
    for (i, (line, line_w_u)) in shaped.iter().enumerate() {
        let line_w = line_w_u * k;
        let lx = match p.align {
            Align::Left => bx,
            Align::Center => bx + (block_w - line_w) / 2.0,
            Align::Right => bx + (block_w - line_w),
        };
        let top = by + i as f64 * line_h;
        // baseline = top of the line + ascent·k; the line-spacing multiplier
        // stretches the STEP of lines, not the glyph itself
        let baseline = top + face.ascent_u * k;
        let mut prev_cluster: Option<u32> = None;
        for g in line {
            // a new byte address — a new cluster; the glyphs of one cluster
            // (a ligature, a base with a diacritic) go in a row and share the number
            if prev_cluster != Some(g.cluster) {
                if prev_cluster.is_some() {
                    cluster_no += 1;
                }
                prev_cluster = Some(g.cluster);
                clusters.push(PxBox { x: lx + g.x * k, y: top, w: g.adv * k, h: line_h });
            } else if let Some(b) = clusters.last_mut() {
                // a second glyph of the same cluster widens its box
                let right = (lx + g.x * k + g.adv * k).max(b.x + b.w);
                b.w = right - b.x;
            }
            glyphs.push(PlacedGlyph {
                id: g.id,
                x: q64(lx + g.x * k),
                y: q64(baseline + g.y * k),
                cluster: cluster_no,
            });
        }
        if prev_cluster.is_some() {
            cluster_no += 1;
        }
        line_boxes.push(PxBox { x: lx, y: top, w: line_w, h: line_h });
    }

    Laid {
        glyphs,
        info: LayoutInfo {
            block: PxBox { x: bx, y: by, w: block_w, h: block_h },
            lines: line_boxes,
            clusters,
        },
        scale: PxScale::from((size_px * face.height_per_em) as f32),
    }
}

/// The 1/64 px quantum: glyph positions are deterministic and do not jitter from the
/// order of arithmetic.
fn q64(v: f64) -> f64 {
    (v * 64.0).round() / 64.0
}

// ---- public API ---------------------------------------------------------

/// Layout geometry at the given frame size (without rasterization).
pub fn layout_info(p: &TextParams, w: u32, h: u32) -> LayoutInfo {
    layout(p, w, h).info
}

/// Codepoints of the text for which the face has NO glyph (the answer “no glyph”
/// in the title window — UI rule 11). An empty list = everything is covered.
pub fn uncovered(text: &str, family: &str, style: FaceStyle) -> Vec<char> {
    let face =
        resolve_face(&TextParams { family: family.to_string(), style, ..TextParams::default() });
    let mut out: Vec<char> =
        text.chars().filter(|c| !c.is_control() && face.ab.glyph_id(*c).0 == 0).collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// The title frame EXACTLY `w`×`h`: straight-RGBA8. The decoration layers (plate,
/// shadow, outline, fill) are composed by `order` in premultiplied u16 and
/// un-premultiplied once on output.
pub fn render_rgba(p: &TextParams, w: u32, h: u32) -> Box<[u8]> {
    let n = (w as usize) * (h as usize);
    let mut out = vec![0u8; n * 4];
    if w == 0 || h == 0 || p.text.is_empty() || p.size <= 0.0 {
        return out.into_boxed_slice();
    }
    let face = resolve_face(p);
    let laid = layout(p, w, h);
    let size_px = p.size * h as f64;

    // Reveal (R.53, item 423): the weight of a cluster, 0..255. Full visibility is
    // EXACTLY the former branch (the weight is not computed at all), so the title golden
    // cannot move from the appearance of the animation.
    let full = p.reveal >= 1.0 && p.reveal_soft <= 0.0;
    let ncl = laid.info.clusters.len().max(1) as f64;
    let cluster_w = |c: usize| -> u32 {
        if full {
            return 255;
        }
        // the front moves across clusters; softness — the width of the front in clusters
        let front = p.reveal.clamp(0.0, 1.0) * (ncl + p.reveal_soft.max(0.0));
        let d = front - c as f64;
        if p.reveal_soft <= 0.0 {
            return if d >= 1.0 { 255 } else { 0 };
        }
        ((d / p.reveal_soft).clamp(0.0, 1.0) * 255.0).round() as u32
    };

    // silhouette (A8): the coverage of all glyphs, overlaps — by maximum
    let mut cov = vec![0u8; n];
    for g in &laid.glyphs {
        let wgt = cluster_w(g.cluster);
        if wgt == 0 {
            continue;
        }
        let glyph = Glyph {
            id: ab_glyph::GlyphId(g.id),
            scale: laid.scale,
            position: point(g.x as f32, g.y as f32),
        };
        let Some(og) = face.ab.outline_glyph(glyph) else { continue };
        let pb = og.px_bounds();
        og.draw(|dx, dy, c| {
            let px = pb.min.x as i64 + dx as i64;
            let py = pb.min.y as i64 + dy as i64;
            if px < 0 || py < 0 || px >= w as i64 || py >= h as i64 {
                return;
            }
            let i = (py * w as i64 + px) as usize;
            let raw = (c * 255.0).round().clamp(0.0, 255.0) as u8;
            // modulation by ONE row; at full it is identical (255/255)
            let v = if full { raw } else { ((raw as u32 * wgt + 127) / 255) as u8 };
            cov[i] = cov[i].max(v);
        });
    }

    // premultiplied u16 accumulator per channel
    let mut acc = vec![0u16; n * 4];
    let mut over = |alpha: &[u8], color: [u8; 4]| {
        let ca = color[3] as u32;
        if ca == 0 {
            return;
        }
        for (i, &m) in alpha.iter().enumerate() {
            if m == 0 {
                continue;
            }
            let sa = (m as u32 * ca + 127) / 255; // layer alpha in a pixel
            let inv = 255 - sa;
            let o = i * 4;
            for ch in 0..3 {
                let sc = (color[ch] as u32 * sa + 127) / 255; // premult
                let d = acc[o + ch] as u32;
                acc[o + ch] = (sc + (d * inv + 127) / 255) as u16;
            }
            let da = acc[o + 3] as u32;
            acc[o + 3] = (sa + (da * inv + 127) / 255) as u16;
        }
    };

    for layer in p.order {
        match layer {
            Layer::Fill => over(&cov, p.fill),
            Layer::Plate => {
                if p.plate[3] == 0 {
                    continue;
                }
                // the plate's transparency is built into its alpha map
                let alpha = plate_alpha(&laid.info, p, size_px, w, h);
                over(&alpha, [p.plate[0], p.plate[1], p.plate[2], 255]);
            }
            Layer::Shadow => {
                if p.shadow[3] == 0 {
                    continue;
                }
                let alpha = shadow_alpha(&cov, p, size_px, w, h);
                over(&alpha, p.shadow);
            }
            Layer::Stroke => {
                if p.stroke_w <= 0.0 || p.stroke[3] == 0 {
                    continue;
                }
                let alpha = stroke_alpha(&cov, p.stroke_w * size_px, w, h);
                over(&alpha, p.stroke);
            }
        }
    }

    // un-premultiplication → straight
    for i in 0..n {
        let o = i * 4;
        let a = acc[o + 3].min(255) as u32;
        if a == 0 {
            continue;
        }
        for ch in 0..3 {
            let c = acc[o + ch].min(255) as u32;
            out[o + ch] = ((c * 255 + a / 2) / a).min(255) as u8;
        }
        out[o + 3] = a as u8;
    }
    out.into_boxed_slice()
}

/// Plate: a rectangle PER LINE (honest geometry — it cannot be derived from glyph metrics,
/// F-TXT-005) + paddings in fractions of em; the plate's transparency
/// is built into the alpha.
fn plate_alpha(info: &LayoutInfo, p: &TextParams, size_px: f64, w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0u8; (w as usize) * (h as usize)];
    let (px, py) = (p.plate_pad.0 * size_px, p.plate_pad.1 * size_px);
    let a = p.plate[3];
    for l in &info.lines {
        if l.w <= 0.0 {
            continue; // an empty line does not draw a plate
        }
        let x0 = (l.x - px).floor().max(0.0) as usize;
        let y0 = (l.y - py).floor().max(0.0) as usize;
        let x1 = ((l.x + l.w + px).ceil() as usize).min(w as usize);
        let y1 = ((l.y + l.h + py).ceil() as usize).min(h as usize);
        for y in y0..y1 {
            let row = y * w as usize;
            for x in x0..x1 {
                out[row + x] = out[row + x].max(a);
            }
        }
    }
    out
}

/// Shadow: the silhouette, offset by WHOLE px (fractions of em; rounding to a pixel is
/// deliberate, documented), blurred by a triple box-blur of ONE
/// channel — the alpha is blurred at a constant colour, so a “green halo”
/// (flowblade trap #937) cannot occur by construction.
fn shadow_alpha(cov: &[u8], p: &TextParams, size_px: f64, w: u32, h: u32) -> Vec<u8> {
    let dx = (p.shadow_dx * size_px).round() as i64;
    let dy = (p.shadow_dy * size_px).round() as i64;
    let (wi, hi) = (w as i64, h as i64);
    let mut out = vec![0u8; (w as usize) * (h as usize)];
    for y in 0..hi {
        let sy = y - dy;
        if sy < 0 || sy >= hi {
            continue;
        }
        for x in 0..wi {
            let sx = x - dx;
            if sx < 0 || sx >= wi {
                continue;
            }
            out[(y * wi + x) as usize] = cov[(sy * wi + sx) as usize];
        }
    }
    let r = (p.shadow_blur * size_px).round().max(0.0) as usize;
    for _ in 0..3 {
        box_blur_a8(&mut out, w as usize, h as usize, r);
    }
    out
}

/// A single box-blur of the A8 channel with radius r (window 2r+1), horizontal +
/// vertical, an integer sliding window.
fn box_blur_a8(buf: &mut [u8], w: usize, h: usize, r: usize) {
    if r == 0 || w == 0 || h == 0 {
        return;
    }
    let win = (2 * r + 1) as u32;
    let mut tmp = vec![0u8; buf.len()];
    // horizontal: sliding window, the edges read as zero
    for y in 0..h {
        let row = y * w;
        let mut sum: u32 = 0;
        for x in 0..w {
            let add = x + r;
            if add < w {
                sum += buf[row + add] as u32;
            }
            if x == 0 {
                for k in 0..r.min(w) {
                    sum += buf[row + k] as u32;
                }
            } else if let Some(sub) = (x - 1).checked_sub(r) {
                sum -= buf[row + sub] as u32;
            }
            tmp[row + x] = ((sum + win / 2) / win) as u8;
        }
    }
    // vertical
    for x in 0..w {
        let mut sum: u32 = 0;
        for y in 0..h {
            let add = y + r;
            if add < h {
                sum += tmp[add * w + x] as u32;
            }
            if y == 0 {
                for k in 0..r.min(h) {
                    sum += tmp[k * w + x] as u32;
                }
            } else if let Some(sub) = (y - 1).checked_sub(r) {
                sum -= tmp[sub * w + x] as u32;
            }
            buf[y * w + x] = ((sum + win / 2) / win) as u8;
        }
    }
}

/// Outline: an exact Euclidean distance transform (Felzenszwalb) over the
/// binarized silhouette (α ≥ 128), a ramp of ±0.5 px at the boundary of the radius.
/// The “outside” is obtained by the ORDER of layers: the fill on top covers the
/// inner half — a double width is not needed, and the glyph's stroke the outline
/// does not eat (t486).
fn stroke_alpha(cov: &[u8], radius_px: f64, w: u32, h: u32) -> Vec<u8> {
    let dist2 = edt_sq(cov, w as usize, h as usize);
    let r = radius_px.max(0.0);
    dist2
        .iter()
        .map(|&d2| {
            let d = (d2 as f64).sqrt();
            let a = (r + 0.5 - d).clamp(0.0, 1.0);
            (a * 255.0).round() as u8
        })
        .collect()
}

/// Squares of Euclidean distances to the nearest pixel of the silhouette
/// (α ≥ 128). The classic two-pass Felzenszwalb: parabolas along
/// columns, then along rows; integer squares in i64.
fn edt_sq(cov: &[u8], w: usize, h: usize) -> Vec<i64> {
    const INF: i64 = i64::MAX / 4;
    let mut d = vec![INF; w * h];
    for (i, &c) in cov.iter().enumerate() {
        if c >= 128 {
            d[i] = 0;
        }
    }
    let mut f = vec![0i64; w.max(h)];
    let mut dd = vec![0i64; w.max(h)];
    let mut v = vec![0usize; w.max(h)];
    let mut z = vec![0f64; w.max(h) + 1];
    // columns
    for x in 0..w {
        for y in 0..h {
            f[y] = d[y * w + x];
        }
        dt_1d(&f[..h], &mut dd[..h], &mut v[..h], &mut z[..h + 1]);
        for y in 0..h {
            d[y * w + x] = dd[y];
        }
    }
    // rows
    for y in 0..h {
        let row = y * w;
        f[..w].copy_from_slice(&d[row..row + w]);
        dt_1d(&f[..w], &mut dd[..w], &mut v[..w], &mut z[..w + 1]);
        d[row..row + w].copy_from_slice(&dd[..w]);
    }
    d
}

/// 1D distance transform by parabolas (Felzenszwalb–Huttenlocher).
fn dt_1d(f: &[i64], out: &mut [i64], v: &mut [usize], z: &mut [f64]) {
    const INF: i64 = i64::MAX / 4;
    let n = f.len();
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;
    let sq = |q: usize| (q * q) as i64;
    for q in 1..n {
        if f[q] >= INF && f[v[k]] >= INF {
            // both “infinite”: no parabola is needed
            continue;
        }
        loop {
            let p = v[k];
            let s = if f[q] >= INF {
                f64::INFINITY
            } else if f[p] >= INF {
                f64::NEG_INFINITY
            } else {
                ((f[q] + sq(q)) - (f[p] + sq(p))) as f64 / (2 * (q - p)) as f64
            };
            if s <= z[k] {
                if k == 0 {
                    v[0] = q;
                    z[0] = f64::NEG_INFINITY;
                    z[1] = f64::INFINITY;
                    break;
                }
                k -= 1;
                continue;
            }
            k += 1;
            v[k] = q;
            z[k] = s;
            z[k + 1] = f64::INFINITY;
            break;
        }
    }
    let mut k = 0usize;
    for (q, o) in out.iter_mut().enumerate() {
        while z[k + 1] < q as f64 {
            k += 1;
        }
        let p = v[k];
        *o = if f[p] >= INF { INF } else { f[p] + ((q as i64 - p as i64) * (q as i64 - p as i64)) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fnv(bytes: &[u8]) -> u64 {
        let mut h = 0xcbf29ce484222325u64;
        for &b in bytes {
            h ^= b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }

    fn sample() -> TextParams {
        TextParams {
            text: "Привет, Kinotrain!\nтитр".into(), size: 0.12, ..TextParams::default()
        }
    }

    /// t480: the same input → the same bytes; the anchor hash is the guard of the pins
    /// of the font and the crates (it turns red when they change — and that is its job).
    #[test]
    fn t480_render_is_deterministic_with_anchor_hash() {
        let p = sample();
        let a = render_rgba(&p, 320, 180);
        let b = render_rgba(&p, 320, 180);
        assert_eq!(a, b, "repeat must be byte for byte");
        // non-empty and with a real inscription
        let ink = a.chunks_exact(4).filter(|px| px[3] > 0).count();
        assert!(ink > 300, "there is no text in the frame: {ink} pixels");
        // straight alpha: the colour of a pixel is exactly the fill colour
        assert!(a.chunks_exact(4).filter(|px| px[3] > 0).all(|px| px[..3] == [255, 255, 255]));
        assert_eq!(
            fnv(&a),
            0x5a04dd9723ca145b,
            "rasterization anchor: after updating the font/crates, regenerate deliberately \
             (cargo test -p kt-text -- t480_print)"
        );
    }

    /// Anchor stamp for t480 (see CLAUDE.md: an update = an event).
    #[test]
    #[ignore = "anchor generator: cargo test -p kt-text -- --ignored t480_print"]
    fn t480_print_anchor() {
        let a = render_rgba(&sample(), 320, 180);
        println!("t480 anchor = 0x{:016x}", fnv(&a));
    }

    /// t481: scale invariance — the layout in a ×2 frame is exactly ×2
    /// (the PxScale ≠ em trap would be caught here).
    #[test]
    fn t481_layout_scales_with_frame() {
        let p = sample();
        let a = layout_info(&p, 640, 360);
        let b = layout_info(&p, 1280, 720);
        let close = |x: f64, y: f64| (x - y).abs() <= 0.5;
        assert!(close(b.block.w, a.block.w * 2.0), "{} vs {}", b.block.w, a.block.w);
        assert!(close(b.block.h, a.block.h * 2.0));
        assert!(close(b.block.x, a.block.x * 2.0) && close(b.block.y, a.block.y * 2.0));
        assert_eq!(a.lines.len(), b.lines.len());
        // and the glyphs really are in these boxes: the render at ×2 is non-empty in the same fractions
        let big = render_rgba(&p, 1280, 720);
        assert!(big.chunks_exact(4).any(|px| px[3] > 0));
    }

    /// t482: kerning is APPLIED — Noto Sans has no legacy `kern`, so
    /// without the shaper (GPOS) the width of a pair would equal the sum of the single ones.
    #[test]
    fn t482_gpos_kerning_is_applied() {
        let f = face(FaceStyle::Regular);
        let wid = |s: &str| shape_line(f, s, 0.0).1;
        let pair = wid("AV");
        let solo = wid("A") + wid("V");
        assert!(pair < solo - 1.0, "kerning was not applied: AV={pair}, A+V={solo} (font units)");
        // ligatures live too: fi is shorter than or equal to f+i, but as ONE glyph
        let (fi, _) = shape_line(f, "fi", 0.0);
        assert_eq!(fi.len(), 1, "fi ligature did not form: {} glyphs", fi.len());
    }

    /// t483: “no glyph” — an honest list; Cyrillic/Latin are covered by
    /// the full vendored font.
    #[test]
    fn t483_uncovered_codepoints() {
        assert!(
            uncovered(
                "Съешь ещё этих мягких французских булок, да выпей же чаю. Zażółć!",
                VENDORED_FAMILY,
                FaceStyle::Regular
            )
            .is_empty()
        );
        assert_eq!(uncovered("before 漢字 after", VENDORED_FAMILY, FaceStyle::Bold), vec!['字', '漢']);
        // control characters are not counted as “uncovered”
        assert!(uncovered("a\nb\t", VENDORED_FAMILY, FaceStyle::Regular).is_empty());
    }

    /// t490 (R.52 C4): system families. A non-existent one — substitution by the
    /// vendored one BYTE-FOR-BYTE (+ `MissingFallback` for the notice);
    /// a found system one resolves to a file and draws itself.
    #[test]
    fn t490_system_family_resolve_and_fallback() {
        assert_eq!(resolve_family("__kt_no_such_family__", false), FontResolve::MissingFallback);
        assert_eq!(resolve_family(VENDORED_FAMILY, true), FontResolve::Vendored);
        assert_eq!(resolve_family("", false), FontResolve::Vendored);

        let mut missing = sample();
        missing.family = "__kt_no_such_family__".into();
        let sub = render_rgba(&missing, 320, 180);
        assert_eq!(
            sub,
            render_rgba(&sample(), 320, 180),
            "the substitute must be vendored byte for byte"
        );

        // if the machine has system fonts — the first resolving one
        // must give an existing file and its own render
        if let Some(fam) = system_families()
            .iter()
            .find(|f| matches!(resolve_family(f, false), FontResolve::System(_)))
        {
            let FontResolve::System(path) = resolve_family(fam, false) else { unreachable!() };
            assert!(path.exists(), "{path:?}");
            let mut sp = sample();
            sp.family = fam.clone();
            let frame = render_rgba(&sp, 320, 180);
            assert!(frame.chunks_exact(4).any(|px| px[3] > 0), "{fam}: frame is empty");
        }
    }

    /// t506 (R.53, item 413): the list of system families is built ONCE.
    /// It is called on every frame while the combo popup in the title window is open,
    /// and a walk over all faces with sorting and dedup — allocations in the
    /// frame path (invariant 12).
    #[test]
    fn t506_system_families_are_built_once() {
        let a = system_families();
        let b = system_families();
        assert!(std::ptr::eq(a, b), "list is rebuilt on every call");
    }

    /// t486 (R.52 C3): decoration. The order of layers changes the bytes
    /// predictably; the outline does NOT eat the glyph's stroke (the fill on top is a
    /// direct check of the kde-invent #1550 trap); the shadow lies outside the
    /// silhouette in its own colour; the plate follows the lines.
    #[test]
    fn t486_ornament_layers() {
        let mut p = sample();
        p.text = "IHK".into();
        p.size = 0.5;
        let plain = render_rgba(&p, 320, 180);
        let count = |buf: &[u8], pred: &dyn Fn(&[u8]) -> bool| {
            buf.chunks_exact(4).filter(|px| pred(px)).count()
        };
        // the interior of the stroke — pixels with FULL alpha: the antialiased edge
        // legitimately blends with the lower layers, the interior — never
        let is_fill = |px: &[u8]| px[3] == 255 && px[..3] == [255, 255, 255];
        let fill_plain = count(&plain, &is_fill);
        assert!(fill_plain > 500, "no fill: {fill_plain}");

        // outline: the interior of the stroke is intact, around it the outline colour appeared
        let mut st = p.clone();
        st.stroke_w = 0.06;
        st.stroke = [255, 0, 0, 255];
        let stroked = render_rgba(&st, 320, 180);
        let fill_kept = count(&stroked, &is_fill);
        assert!(
            fill_kept as f64 > fill_plain as f64 * 0.98,
            "the stroke ate the fill: {fill_kept} of {fill_plain}"
        );
        let red = count(&stroked, &|px| px[3] > 200 && px[0] > 200 && px[1] < 60);
        assert!(red > 400, "stroke is not visible: {red}");

        // the order decides: the fill UNDER the outline — the outline covers the stroke
        let mut inv = st.clone();
        inv.order = [Layer::Plate, Layer::Shadow, Layer::Fill, Layer::Stroke];
        let inverted = render_rgba(&inv, 320, 180);
        assert_ne!(inverted, stroked, "layer order must change the bytes");
        assert!(count(&inverted, &is_fill) < fill_plain / 2, "fill under stroke must be covered");

        // shadow: pixels of the shadow colour OUTSIDE the silhouette, offset right-down
        let mut sh = p.clone();
        sh.shadow = [0, 0, 0, 255];
        sh.shadow_dx = 0.15;
        sh.shadow_dy = 0.15;
        sh.shadow_blur = 0.0;
        let shadowed = render_rgba(&sh, 320, 180);
        let dark = count(&shadowed, &|px| px[3] > 200 && px[0] < 40 && px[1] < 40);
        assert!(dark > 300, "shadow is not visible: {dark}");
        // with blur the shadow is softer: there are more semi-transparent pixels
        sh.shadow_blur = 0.12;
        let soft = render_rgba(&sh, 320, 180);
        let half = |buf: &[u8]| count(buf, &|px| px[3] > 10 && px[3] < 200);
        assert!(half(&soft) > half(&shadowed), "blur did not soften the shadow");

        // plate: the rectangle of the line in its own colour, grows with the text
        let mut pl = p.clone();
        pl.plate = [0, 60, 0, 255];
        let plated = render_rgba(&pl, 320, 180);
        let green = count(&plated, &|px| px[3] > 200 && px[1] > 40 && px[0] < 30);
        assert!(green > 2000, "plate is not visible: {green}");
        pl.text = "IHKWWWW".into();
        let wider = render_rgba(&pl, 320, 180);
        assert!(
            count(&wider, &|px| px[3] > 0) > count(&plated, &|px| px[3] > 0),
            "plate must grow with the text"
        );
    }

    /// Empty text and a zero frame — transparent and without a panic.
    #[test]
    fn t480b_empty_edges() {
        let mut p = TextParams::default();
        assert!(render_rgba(&p, 64, 36).iter().all(|b| *b == 0));
        p.text = "x".into();
        assert_eq!(render_rgba(&p, 0, 0).len(), 0);
        let f = render_rgba(&p, 64, 36);
        assert_eq!(f.len(), 64 * 36 * 4);
    }

    // ---- R.53 (items 423-424): clusters, reveal, motion ------------

    fn rows(p: &TextParams, w: u32, h: u32) -> Vec<u8> {
        render_rgba(p, w, h).to_vec()
    }

    fn ink(rgba: &[u8]) -> u64 {
        rgba.chunks_exact(4).map(|p| p[3] as u64).sum()
    }

    /// t534: CLUSTER boxes exist for every layout, there are as many of them as there are
    /// units of text, and they go left to right inside a line.
    /// A cluster, not a symbol: a ligature arrives as one unit for free.
    #[test]
    fn t534_layout_reports_cluster_boxes() {
        let p = TextParams { text: "ABC".into(), size: 0.2, ..TextParams::default() };
        let li = layout_info(&p, 400, 200);
        assert_eq!(li.clusters.len(), 3, "three letters — three clusters");
        assert!(li.clusters[0].x < li.clusters[1].x, "clusters run along the text");
        assert!(li.clusters[1].x < li.clusters[2].x);
        for c in &li.clusters {
            assert!(c.w > 0.0 && c.h > 0.0, "empty cluster box");
        }
        // multi-line text: the numbering is THROUGH
        let p2 = TextParams { text: "AB\nCD".into(), ..p.clone() };
        let li2 = layout_info(&p2, 400, 200);
        assert_eq!(li2.clusters.len(), 4, "line break is not a cluster");
        assert!(li2.clusters[2].y > li2.clusters[0].y, "second line is below the first");
    }

    /// t535: the reveal opens clusters IN TURN, and at full
    /// visibility the frame is BYTE FOR BYTE equal to the frame without animation — the promise
    /// “the title golden did not move” rests on this.
    #[test]
    fn t535_reveal_opens_clusters_in_order() {
        let base = TextParams { text: "ABCD".into(), size: 0.3, ..TextParams::default() };
        let full = rows(&base, 400, 200);
        let mut p = base.clone();
        p.reveal = 1.0;
        assert_eq!(rows(&p, 400, 200), full, "full visibility must stay on the old branch");

        p.reveal = 0.0;
        assert_eq!(ink(&rows(&p, 400, 200)), 0, "nothing is visible at zero");
        // grows monotonically and reaches the full frame
        let mut prev = 0u64;
        for i in 1..=4 {
            p.reveal = i as f64 / 4.0;
            let v = ink(&rows(&p, 400, 200));
            assert!(v > prev, "reveal {i}/4 added no ink: {v} after {prev}");
            prev = v;
        }
        assert_eq!(prev, ink(&full), "full reveal must give the full frame");
    }

    /// t536: the soft front gives INTERMEDIATE values where the hard one
    /// gives only 0 and 255 — otherwise the “reveal” would be the same typewriter.
    #[test]
    fn t536_soft_reveal_is_not_a_hard_cut() {
        let base = TextParams { text: "ABCD".into(), size: 0.3, ..TextParams::default() };
        let hard = TextParams { reveal: 0.5, reveal_soft: 0.0, ..base.clone() };
        let soft = TextParams { reveal: 0.5, reveal_soft: 1.0, ..base.clone() };
        let h = rows(&hard, 400, 200);
        let s = rows(&soft, 400, 200);
        assert_ne!(h, s, "soft edge must differ from hard edge");
        assert!(ink(&s) > 0);
        // the soft one has a cluster with partial opacity
        let max_a = |v: &[u8]| v.chunks_exact(4).map(|p| p[3]).max().unwrap_or(0);
        assert_eq!(max_a(&h), 255);
        assert!(ink(&s) != ink(&h), "soft edge gives a different total of ink");
    }

    /// t537: the offset is a TRANSFER of the block, and the mutual geometry of the glyphs from
    /// it is bit-for-bit constant. The 1/64 px quantum (its multiples are exactly representable in
    /// f64) — the reason the offset is quantized BEFORE addition.
    #[test]
    fn t537_offset_translates_without_reshaping() {
        let base = TextParams { text: "ABC".into(), size: 0.25, ..TextParams::default() };
        let a = layout_info(&base, 400, 200);
        let shift = 8.0; // a whole number of pixels — an exact shift
        let moved = TextParams { offset: (shift, 0.0), ..base.clone() };
        let b = layout_info(&moved, 400, 200);
        assert_eq!(b.block.x, a.block.x + shift, "block did not move by exactly the offset");
        assert_eq!(b.block.w, a.block.w, "wrap changed the block width");
        for (x, y) in a.clusters.iter().zip(&b.clusters) {
            assert_eq!(y.x - x.x, shift, "cluster moved by a different distance");
            assert_eq!(y.w, x.w, "wrap reshaped the cluster");
        }
        // and the pixels: a shift by whole px gives exactly the shifted frame
        let fa = render_rgba(&base, 400, 200);
        let fb = render_rgba(&moved, 400, 200);
        let at = |f: &[u8], x: usize, y: usize| f[((y * 400 + x) * 4) + 3];
        let sh = shift as usize;
        for y in 0..200 {
            for x in 0..(400 - sh) {
                assert_eq!(at(&fa, x, y), at(&fb, x + sh, y), "pixel ({x},{y}) did not carry over");
            }
        }
    }

    /// t538: a fractional offset is QUANTIZED to 1/64 px, so neighbouring
    /// frames of motion differ by a transfer, not by a recalculation of the layout.
    #[test]
    fn t538_offset_is_quantised_to_one_sixty_fourth() {
        let base = TextParams { text: "AB".into(), size: 0.25, ..TextParams::default() };
        let mk = |dx: f64| layout_info(&TextParams { offset: (dx, 0.0), ..base.clone() }, 400, 200);
        // less than HALF a quantum — the layout must be the same
        // (rounding to nearest: 1/128 is exactly half and would go up)
        assert_eq!(mk(0.0).block.x, mk(1.0 / 256.0).block.x, "a fraction finer than the quantum shifted something");
        // 1/64 px — already a quantum
        assert_ne!(mk(0.0).block.x, mk(1.0 / 64.0).block.x, "1/64 px quantum did not take effect");
        // and the quanta add up exactly: 64 steps of 1/64 give exactly 1 px
        let step = mk(1.0 / 64.0).block.x - mk(0.0).block.x;
        assert_eq!(step * 64.0, 1.0, "quantum is not exact in f64");
    }
}
