//! Растеризация титров (Р.52, F-TXT-001): чистая функция
//! (параметры, W, H) → straight-RGBA байты. Правила — CLAUDE.md крейта:
//! вендоренный шрифт, запиненные harfrust/ab_glyph, никакого GUI.
//!
//! harfrust даёт ПОЗИЦИИ (кернинг GPOS, лигатуры, вязь), ab_glyph —
//! ПИКСЕЛИ по GlyphId; метрики — от тех же байт шрифта. В С2 рисуется
//! только заливка; обводка/тень/плашка (F-TXT-005) — С3, поля под них
//! уже в структуре: формат не догоняет код.

use ab_glyph::{point, Font as _, FontRef as AbFont, Glyph, PxScale};
use harfrust::{FontRef as HrFont, ShapeOptions, ShaperData, UnicodeBuffer};
use std::sync::OnceLock;

/// Единственная вендоренная семья v1 (С4 добавит системные).
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

/// Якорь 3×3: какой точкой блок текста сидит в `pos`.
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

/// Слой оформления (порядок отрисовки СНИЗУ ВВЕРХ — сохраняемое поле с
/// первого дня: урок Kdenlive из F-TXT-005). Рендер слоёв, кроме
/// заливки, — С3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Plate,
    Shadow,
    Stroke,
    Fill,
}

/// Параметры титра — вход растеризатора. Всё ОТНОСИТЕЛЬНОЕ: кегль — доля
/// высоты кадра, трекинг/обводка/тень/паддинги — доли em; кадр рождается
/// в любом Quality без пересчёта параметров.
#[derive(Debug, Clone, PartialEq)]
pub struct TextParams {
    /// Многострочный текст, явные '\n'. Пустая строка = прозрачный кадр.
    pub text: String,
    /// Семья шрифта. v1: не-вендоренная семья тихо падает в вендоренную
    /// (С4 добавит системные и видимое уведомление о подмене).
    pub family: String,
    pub style: FaceStyle,
    /// Кегль: доля ВЫСОТЫ кадра (0.07 ≈ нижняя треть).
    pub size: f64,
    /// Множитель межстрочного интервала.
    pub line_spacing: f64,
    /// Трекинг, доля em.
    pub tracking: f64,
    pub align: Align,
    pub anchor: Anchor,
    /// Нормированные координаты якоря (0..1 от ширины/высоты кадра).
    pub pos: (f64, f64),
    /// Заливка RGBA (straight).
    pub fill: [u8; 4],
    /// Обводка: ширина в долях em (0 = выкл) + цвет. Рендер — С3.
    pub stroke_w: f64,
    pub stroke: [u8; 4],
    /// Тень: сдвиг и размытие в долях em, цвет (альфа 0 = выкл). С3.
    pub shadow_dx: f64,
    pub shadow_dy: f64,
    pub shadow_blur: f64,
    pub shadow: [u8; 4],
    /// Плашка: паддинги в долях em, цвет (альфа 0 = выкл). С3.
    pub plate_pad: (f64, f64),
    pub plate: [u8; 4],
    /// Порядок слоёв снизу вверх.
    pub order: [Layer; 4],
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
        }
    }
}

/// Бокс в пикселях кадра.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PxBox {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

/// Геометрия раскладки: бокс блока и боксы строк — для драга в превью,
/// плашки и посимвольной анимации будущего (F-TXT-006).
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutInfo {
    pub block: PxBox,
    pub lines: Vec<PxBox>,
}

pub struct FaceInfo {
    pub family: &'static str,
    pub style: FaceStyle,
}

/// Вендоренные начертания — для комбо в окне титра.
pub fn faces() -> &'static [FaceInfo] {
    &[
        FaceInfo { family: VENDORED_FAMILY, style: FaceStyle::Regular },
        FaceInfo { family: VENDORED_FAMILY, style: FaceStyle::Bold },
    ]
}

// ---- шрифт -----------------------------------------------------------------

static REGULAR_BYTES: &[u8] = include_bytes!("../assets/NotoSans-Regular.ttf");
static BOLD_BYTES: &[u8] = include_bytes!("../assets/NotoSans-Bold.ttf");

struct Face {
    ab: AbFont<'static>,
    hr: HrFont<'static>,
    shaper: ShaperData,
    /// px на font unit при кегле в 1 px: 1/units_per_em.
    unit: f64,
    ascent_u: f64,
    descent_u: f64,
    gap_u: f64,
    /// Ловушка ab_glyph: `PxScale` — это ВЫСОТА (ascent−descent), не em.
    /// Масштаб для «кегль = size_px» выводится только отсюда (t481).
    height_per_em: f64,
}

fn face(style: FaceStyle) -> &'static Face {
    static REGULAR: OnceLock<Face> = OnceLock::new();
    static BOLD: OnceLock<Face> = OnceLock::new();
    let (cell, bytes) = match style {
        FaceStyle::Regular => (&REGULAR, REGULAR_BYTES),
        FaceStyle::Bold => (&BOLD, BOLD_BYTES),
    };
    cell.get_or_init(|| {
        let ab = AbFont::try_from_slice(bytes).expect("vendored font parses");
        let hr = HrFont::new(bytes).expect("vendored font parses (read-fonts)");
        let shaper = ShaperData::new(&hr);
        let upem = ab.units_per_em().expect("vendored font has upem") as f64;
        let ascent_u = ab.ascent_unscaled() as f64;
        let descent_u = ab.descent_unscaled() as f64; // отрицательный
        let gap_u = ab.line_gap_unscaled() as f64;
        Face {
            ab,
            hr,
            shaper,
            unit: 1.0 / upem,
            ascent_u,
            descent_u,
            gap_u,
            height_per_em: (ascent_u - descent_u) / upem,
        }
    })
}

fn resolve_face(p: &TextParams) -> &'static Face {
    // v1: любая семья, кроме вендоренной, падает в вендоренную; выбор
    // системных семей и уведомление о подмене — С4
    face(p.style)
}

// ---- раскладка -------------------------------------------------------------

struct PlacedGlyph {
    id: u16,
    /// Начало пера в px кадра (baseline).
    x: f64,
    y: f64,
}

struct Laid {
    glyphs: Vec<PlacedGlyph>,
    info: LayoutInfo,
    /// ab_glyph-масштаб для растеризации при этом кегле.
    scale: PxScale,
}

/// Строка после шейпинга: глифы (id, x, y в font units) и ширина в units.
type ShapedLine = (Vec<(u16, f64, f64)>, f64);

/// Шейпинг одной строки.
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
        // harfrust: Y+ вверх; кадр: Y+ вниз
        glyphs.push((id, cursor + pos.x_offset as f64, -(pos.y_offset as f64)));
        cursor += pos.x_advance as f64 + tracking_u;
    }
    if !glyphs.is_empty() {
        cursor -= tracking_u; // трекинг — МЕЖДУ глифами
    }
    (glyphs, cursor.max(0.0))
}

fn layout(p: &TextParams, w: u32, h: u32) -> Laid {
    let face = resolve_face(p);
    let size_px = (p.size * h as f64).max(0.0);
    let k = size_px * face.unit; // px на font unit
    let tracking_u = if size_px > 0.0 { p.tracking / face.unit } else { 0.0 };

    let lines: Vec<&str> = p.text.split('\n').collect();
    let shaped: Vec<ShapedLine> =
        lines.iter().map(|l| shape_line(face, l, tracking_u)).collect();

    let line_h = (face.ascent_u - face.descent_u + face.gap_u) * k * p.line_spacing;
    let block_w = shaped.iter().map(|(_, w)| *w * k).fold(0.0f64, f64::max);
    let block_h = line_h * shaped.len() as f64;

    // якорь: какой точкой блок сидит в pos
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

    let mut glyphs = Vec::new();
    let mut line_boxes = Vec::with_capacity(shaped.len());
    for (i, (line, line_w_u)) in shaped.iter().enumerate() {
        let line_w = line_w_u * k;
        let lx = match p.align {
            Align::Left => bx,
            Align::Center => bx + (block_w - line_w) / 2.0,
            Align::Right => bx + (block_w - line_w),
        };
        let top = by + i as f64 * line_h;
        // базовая линия = верх строки + ascent·k; межстрочный множитель
        // растягивает ШАГ строк, а не сам глиф
        let baseline = top + face.ascent_u * k;
        for (id, gx_u, gy_u) in line {
            glyphs.push(PlacedGlyph {
                id: *id,
                x: q64(lx + gx_u * k),
                y: q64(baseline + gy_u * k),
            });
        }
        line_boxes.push(PxBox { x: lx, y: top, w: line_w, h: line_h });
    }

    Laid {
        glyphs,
        info: LayoutInfo {
            block: PxBox { x: bx, y: by, w: block_w, h: block_h },
            lines: line_boxes,
        },
        scale: PxScale::from((size_px * face.height_per_em) as f32),
    }
}

/// Квант 1/64 px: позиции глифов детерминированы и не дрожат от порядка
/// арифметики.
fn q64(v: f64) -> f64 {
    (v * 64.0).round() / 64.0
}

// ---- публичное API ---------------------------------------------------------

/// Геометрия раскладки при данном размере кадра (без растеризации).
pub fn layout_info(p: &TextParams, w: u32, h: u32) -> LayoutInfo {
    layout(p, w, h).info
}

/// Кодпоинты текста, для которых в начертании НЕТ глифа (ответ «глифа
/// нет» в окне титра — правило UI 11). Пустой список = всё покрыто.
pub fn uncovered(text: &str, _family: &str, style: FaceStyle) -> Vec<char> {
    let face = face(style);
    let mut out: Vec<char> = text
        .chars()
        .filter(|c| !c.is_control() && face.ab.glyph_id(*c).0 == 0)
        .collect();
    out.sort_unstable();
    out.dedup();
    out
}

/// Кадр титра РОВНО `w`×`h`: straight-RGBA8. Слои оформления (плашка,
/// тень, обводка, заливка) компонуются по `order` в premultiplied u16 и
/// разпремультиплицируются один раз на выходе.
pub fn render_rgba(p: &TextParams, w: u32, h: u32) -> Box<[u8]> {
    let n = (w as usize) * (h as usize);
    let mut out = vec![0u8; n * 4];
    if w == 0 || h == 0 || p.text.is_empty() || p.size <= 0.0 {
        return out.into_boxed_slice();
    }
    let face = resolve_face(p);
    let laid = layout(p, w, h);
    let size_px = p.size * h as f64;

    // силуэт (A8): покрытие всех глифов, перекрытия — максимумом
    let mut cov = vec![0u8; n];
    for g in &laid.glyphs {
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
            let v = (c * 255.0).round().clamp(0.0, 255.0) as u8;
            cov[i] = cov[i].max(v);
        });
    }

    // premultiplied-аккумулятор u16 на канал
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
            let sa = (m as u32 * ca + 127) / 255; // альфа слоя в пикселе
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
                // прозрачность плашки вшита в её альфа-карту
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

    // разпремультипликация → straight
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

/// Плашка: прямоугольник ПО СТРОКЕ (честная геометрия — из метрик глифов
/// её не вывести, F-TXT-005) + паддинги в долях em; прозрачность плашки
/// вшита в альфу.
fn plate_alpha(info: &LayoutInfo, p: &TextParams, size_px: f64, w: u32, h: u32) -> Vec<u8> {
    let mut out = vec![0u8; (w as usize) * (h as usize)];
    let (px, py) = (p.plate_pad.0 * size_px, p.plate_pad.1 * size_px);
    let a = p.plate[3];
    for l in &info.lines {
        if l.w <= 0.0 {
            continue; // пустая строка плашку не рисует
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

/// Тень: силуэт, сдвинутый на ЦЕЛЫЕ px (доли em; округление до пикселя —
/// осознанное, документировано), размытый трёхкратным box-blur ОДНОГО
/// канала — размывается альфа при постоянном цвете, «зелёного ореола»
/// (ловушка flowblade #937) не бывает по построению.
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

/// Однократный box-blur канала A8 радиусом r (окно 2r+1), горизонталь +
/// вертикаль, целочисленное скользящее окно.
fn box_blur_a8(buf: &mut [u8], w: usize, h: usize, r: usize) {
    if r == 0 || w == 0 || h == 0 {
        return;
    }
    let win = (2 * r + 1) as u32;
    let mut tmp = vec![0u8; buf.len()];
    // горизонталь: скользящее окно, края читаются как ноль
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
    // вертикаль
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

/// Обводка: точная евклидова дистанс-трансформа (Felzenszwalb) по
/// бинаризованному силуэту (α ≥ 128), рамп ±0.5 px на границе радиуса.
/// «Наружность» получается ПОРЯДКОМ слоёв: заливка сверху перекрывает
/// внутреннюю половину — двойная ширина не нужна, а штрих глифа обводка
/// не съедает (t486).
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

/// Квадраты евклидовых расстояний до ближайшего пикселя силуэта
/// (α ≥ 128). Классический двухпроходный Felzenszwalb: параболы по
/// столбцам, затем по строкам; целые квадраты в i64.
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
    // столбцы
    for x in 0..w {
        for y in 0..h {
            f[y] = d[y * w + x];
        }
        dt_1d(&f[..h], &mut dd[..h], &mut v[..h], &mut z[..h + 1]);
        for y in 0..h {
            d[y * w + x] = dd[y];
        }
    }
    // строки
    for y in 0..h {
        let row = y * w;
        f[..w].copy_from_slice(&d[row..row + w]);
        dt_1d(&f[..w], &mut dd[..w], &mut v[..w], &mut z[..w + 1]);
        d[row..row + w].copy_from_slice(&dd[..w]);
    }
    d
}

/// 1D-дистанс-трансформа параболами (Felzenszwalb–Huttenlocher).
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
            // оба «бесконечны»: парабола не нужна
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
            text: "Привет, Kinotrain!\nтитр".into(),
            size: 0.12,
            ..TextParams::default()
        }
    }

    /// t480: тот же вход → те же байты; якорный хэш — сторож пинов
    /// шрифта и крейтов (краснеет при их смене — и это его работа).
    #[test]
    fn t480_render_is_deterministic_with_anchor_hash() {
        let p = sample();
        let a = render_rgba(&p, 320, 180);
        let b = render_rgba(&p, 320, 180);
        assert_eq!(a, b, "повтор обязан быть байт-в-байт");
        // непустой и с настоящей надписью
        let ink = a.chunks_exact(4).filter(|px| px[3] > 0).count();
        assert!(ink > 300, "надписи на кадре нет: {ink} пикселей");
        // straight-альфа: цвет пикселя — ровно цвет заливки
        assert!(a.chunks_exact(4).filter(|px| px[3] > 0).all(|px| px[..3] == [255, 255, 255]));
        assert_eq!(
            fnv(&a),
            0x5a04dd9723ca145b,
            "якорь растеризации: обновил шрифт/крейты — перегенерируй осознанно \
             (cargo test -p kt-text -- t480_print)"
        );
    }

    /// Печать якоря для t480 (см. CLAUDE.md: обновление = событие).
    #[test]
    #[ignore = "генератор якоря: cargo test -p kt-text -- --ignored t480_print"]
    fn t480_print_anchor() {
        let a = render_rgba(&sample(), 320, 180);
        println!("t480 anchor = 0x{:016x}", fnv(&a));
    }

    /// t481: масштабная инвариантность — раскладка в ×2 кадре ровно ×2
    /// (ловушка PxScale ≠ em поймалась бы здесь).
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
        // и глифы реально в этих боксах: рендер в ×2 непуст в тех же долях
        let big = render_rgba(&p, 1280, 720);
        assert!(big.chunks_exact(4).any(|px| px[3] > 0));
    }

    /// t482: кернинг ПРИМЕНЯЕТСЯ — у Noto Sans нет legacy `kern`, так что
    /// без шейпера (GPOS) ширина пары равнялась бы сумме одиночных.
    #[test]
    fn t482_gpos_kerning_is_applied() {
        let f = face(FaceStyle::Regular);
        let wid = |s: &str| shape_line(f, s, 0.0).1;
        let pair = wid("AV");
        let solo = wid("A") + wid("V");
        assert!(
            pair < solo - 1.0,
            "кернинг не применился: AV={pair}, A+V={solo} (font units)"
        );
        // лигатуры тоже живут: fi короче или равна f+i, но ОДНИМ глифом
        let (fi, _) = shape_line(f, "fi", 0.0);
        assert_eq!(fi.len(), 1, "лигатура fi не собралась: {} глифов", fi.len());
    }

    /// t483: «глифа нет» — честный список; кириллица/латиница покрыты
    /// полным вендоренным шрифтом.
    #[test]
    fn t483_uncovered_codepoints() {
        assert!(uncovered("Съешь ещё этих мягких французских булок, да выпей же чаю. Zażółć!",
            VENDORED_FAMILY, FaceStyle::Regular)
            .is_empty());
        assert_eq!(
            uncovered("до 漢字 после", VENDORED_FAMILY, FaceStyle::Bold),
            vec!['字', '漢']
        );
        // управляющие не считаются «непокрытыми»
        assert!(uncovered("a\nb\t", VENDORED_FAMILY, FaceStyle::Regular).is_empty());
    }

    /// t486 (Р.52 С3): оформление. Порядок слоёв меняет байты
    /// предсказуемо; обводка НЕ съедает штрих глифа (заливка сверху —
    /// прямая проверка ловушки kde-invent #1550); тень лежит вне
    /// силуэта своим цветом; плашка следует за строками.
    #[test]
    fn t486_ornament_layers() {
        let mut p = sample();
        p.text = "IHK".into();
        p.size = 0.5;
        let plain = render_rgba(&p, 320, 180);
        let count = |buf: &[u8], pred: &dyn Fn(&[u8]) -> bool| {
            buf.chunks_exact(4).filter(|px| pred(px)).count()
        };
        // интерьер штриха — пиксели с ПОЛНОЙ альфой: антиалиасный край
        // законно смешивается с нижними слоями, интерьер — никогда
        let is_fill = |px: &[u8]| px[3] == 255 && px[..3] == [255, 255, 255];
        let fill_plain = count(&plain, &is_fill);
        assert!(fill_plain > 500, "заливки нет: {fill_plain}");

        // обводка: интерьер штриха цел, вокруг появился цвет обводки
        let mut st = p.clone();
        st.stroke_w = 0.06;
        st.stroke = [255, 0, 0, 255];
        let stroked = render_rgba(&st, 320, 180);
        let fill_kept = count(&stroked, &is_fill);
        assert!(
            fill_kept as f64 > fill_plain as f64 * 0.98,
            "обводка съела штрих: {fill_kept} из {fill_plain}"
        );
        let red = count(&stroked, &|px| px[3] > 200 && px[0] > 200 && px[1] < 60);
        assert!(red > 400, "обводки не видно: {red}");

        // порядок решает: заливка ПОД обводкой — обводка накрывает штрих
        let mut inv = st.clone();
        inv.order = [Layer::Plate, Layer::Shadow, Layer::Fill, Layer::Stroke];
        let inverted = render_rgba(&inv, 320, 180);
        assert_ne!(inverted, stroked, "порядок слоёв обязан менять байты");
        assert!(
            count(&inverted, &is_fill) < fill_plain / 2,
            "fill под stroke обязан быть накрыт"
        );

        // тень: пиксели цвета тени ВНЕ силуэта, сдвинутые вправо-вниз
        let mut sh = p.clone();
        sh.shadow = [0, 0, 0, 255];
        sh.shadow_dx = 0.15;
        sh.shadow_dy = 0.15;
        sh.shadow_blur = 0.0;
        let shadowed = render_rgba(&sh, 320, 180);
        let dark = count(&shadowed, &|px| px[3] > 200 && px[0] < 40 && px[1] < 40);
        assert!(dark > 300, "тени не видно: {dark}");
        // с размытием тень мягче: полупрозрачных пикселей больше
        sh.shadow_blur = 0.12;
        let soft = render_rgba(&sh, 320, 180);
        let half = |buf: &[u8]| count(buf, &|px| px[3] > 10 && px[3] < 200);
        assert!(half(&soft) > half(&shadowed), "размытие не смягчило тень");

        // плашка: прямоугольник строки своим цветом, растёт с текстом
        let mut pl = p.clone();
        pl.plate = [0, 60, 0, 255];
        let plated = render_rgba(&pl, 320, 180);
        let green = count(&plated, &|px| px[3] > 200 && px[1] > 40 && px[0] < 30);
        assert!(green > 2000, "плашки не видно: {green}");
        pl.text = "IHKWWWW".into();
        let wider = render_rgba(&pl, 320, 180);
        assert!(
            count(&wider, &|px| px[3] > 0) > count(&plated, &|px| px[3] > 0),
            "плашка обязана расти за текстом"
        );
    }

    /// Пустой текст и нулевой кадр — прозрачно и без паники.
    #[test]
    fn t480b_empty_edges() {
        let mut p = TextParams::default();
        assert!(render_rgba(&p, 64, 36).iter().all(|b| *b == 0));
        p.text = "x".into();
        assert_eq!(render_rgba(&p, 0, 0).len(), 0);
        let f = render_rgba(&p, 64, 36);
        assert_eq!(f.len(), 64 * 36 * 4);
    }
}
