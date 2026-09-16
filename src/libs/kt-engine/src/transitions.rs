//! Ядра переходов (Р.33 С4): CPU-эталон и упаковка параметров для GPU.
//!
//! Инвариант 7 требует ОДНОГО кода композиции на превью и экспорт, а
//! бэкенд считает исполнением. Поэтому вид перехода превращается в
//! `TransMix` — плоскую пару (режим, 8 чисел) — РОВНО один раз, здесь; и
//! CPU-ядро ниже, и WGSL-зеркало (`kt-gpu/src/shaders/composite.wgsl`)
//! читают одни и те же числа и повторяют одни и те же формулы. Любая
//! правка формулы обязана идти в оба файла одним коммитом (t181/t182).
//!
//! Кроссфейд (`mode == 0`) сюда НЕ заходит: он остаётся целочисленным
//! путём `dissolve_into`, байт-в-байт прежним (GOLDEN_VIDEO не двигается).

use kt_media::VideoFrame;
use kt_model::{Dir, IrisShape, TransitionKind};

/// Режимы, общие для CPU и WGSL. Значение уезжает в униформу, поэтому
/// числа заморожены: вставлять новый режим только В КОНЕЦ.
pub const MODE_CROSSFADE: u32 = 0;
pub const MODE_WIPE: u32 = 1;
pub const MODE_IRIS: u32 = 2;
pub const MODE_CLOCK: u32 = 3;
pub const MODE_BARN: u32 = 4;
pub const MODE_LUMA: u32 = 5;
pub const MODE_PUSH: u32 = 6;
pub const MODE_SLIDE: u32 = 7;
pub const MODE_ZOOM: u32 = 8;
pub const MODE_SPIN: u32 = 9;
pub const MODE_WHIP: u32 = 10;
pub const MODE_BLUR: u32 = 11;
pub const MODE_PIXELATE: u32 = 12;
pub const MODE_GLITCH: u32 = 13;
pub const MODE_FLASH: u32 = 14;
pub const MODE_DIP: u32 = 15;
// Р.51 (п.400): узоры, сдвиги-симбиозы, стилизованные. Числа — контракт
// ТОЛЬКО между transitions.rs и composite.wgsl (правятся одним коммитом):
// в файл проекта едет tag(), в кэш-ключи — уже готовый TransMix.
pub const MODE_BLINDS: u32 = 16;
pub const MODE_CHECKER: u32 = 17;
pub const MODE_BLOCKS: u32 = 18;
pub const MODE_SPIRAL: u32 = 19;
pub const MODE_SPLIT: u32 = 20;
pub const MODE_SQUEEZE: u32 = 21;
pub const MODE_STRETCH: u32 = 22;
pub const MODE_SWAP: u32 = 23;
pub const MODE_SHAKE: u32 = 24;
pub const MODE_BURN: u32 = 25;
pub const MODE_LEAKS: u32 = 26;
pub const MODE_ZOOMBLUR: u32 = 27;

/// Разрешённый переход для исполнителей: режим + 8 чисел (ровно f0/f1
/// униформы GPU). Своей семантики у полей нет — её задают ядра ниже.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TransMix {
    pub mode: u32,
    pub p: [f32; 8],
}

impl Default for TransMix {
    fn default() -> TransMix {
        TransMix::CROSSFADE
    }
}

impl TransMix {
    pub const CROSSFADE: TransMix = TransMix { mode: MODE_CROSSFADE, p: [0.0; 8] };

    pub fn is_crossfade(&self) -> bool {
        self.mode == MODE_CROSSFADE
    }

    /// Единственный перевод «вид из модели → числа для ядер».
    pub fn of(kind: &TransitionKind) -> TransMix {
        let d = |dir: Dir| {
            let (x, y) = dir.vec();
            (x as f32, y as f32)
        };
        let mut p = [0.0f32; 8];
        let mode = match kind {
            TransitionKind::Crossfade => MODE_CROSSFADE,
            TransitionKind::Wipe { dir, angle, softness } => {
                let (dx, dy) = d(*dir);
                let a = (*angle as f32).to_radians();
                let (s, c) = (a.sin(), a.cos());
                // угол доворачивает направление — диагональ получается им
                p[0] = dx * c - dy * s;
                p[1] = dx * s + dy * c;
                p[2] = *softness as f32;
                MODE_WIPE
            }
            TransitionKind::Iris { shape, cx, cy, softness } => {
                p[0] = match shape {
                    IrisShape::Circle => 0.0,
                    IrisShape::Diamond => 1.0,
                    IrisShape::Box => 2.0,
                };
                p[1] = *softness as f32;
                p[2] = *cx as f32;
                p[3] = *cy as f32;
                MODE_IRIS
            }
            TransitionKind::Clock { reverse, cx, cy, softness } => {
                p[0] = if *reverse { 1.0 } else { 0.0 };
                p[1] = *softness as f32;
                p[2] = *cx as f32;
                p[3] = *cy as f32;
                MODE_CLOCK
            }
            TransitionKind::Barn { vertical, softness } => {
                p[0] = if *vertical { 1.0 } else { 0.0 };
                p[1] = *softness as f32;
                MODE_BARN
            }
            TransitionKind::Luma { invert, softness } => {
                p[0] = if *invert { 1.0 } else { 0.0 };
                p[1] = *softness as f32;
                MODE_LUMA
            }
            TransitionKind::Push { dir } => {
                (p[0], p[1]) = d(*dir);
                MODE_PUSH
            }
            TransitionKind::Slide { dir } => {
                (p[0], p[1]) = d(*dir);
                MODE_SLIDE
            }
            TransitionKind::Zoom { amount, cx, cy } => {
                p[0] = *amount as f32;
                p[1] = *cx as f32;
                p[2] = *cy as f32;
                MODE_ZOOM
            }
            TransitionKind::Spin { turns, amount, cx, cy } => {
                p[0] = *turns as f32;
                p[1] = *amount as f32;
                p[2] = *cx as f32;
                p[3] = *cy as f32;
                MODE_SPIN
            }
            TransitionKind::Whip { dir, blur } => {
                (p[0], p[1]) = d(*dir);
                p[2] = *blur as f32;
                MODE_WHIP
            }
            TransitionKind::BlurDissolve { amount } => {
                p[0] = *amount as f32;
                MODE_BLUR
            }
            TransitionKind::Pixelate { amount } => {
                p[0] = *amount as f32;
                MODE_PIXELATE
            }
            TransitionKind::Glitch { amount } => {
                p[0] = *amount as f32;
                MODE_GLITCH
            }
            TransitionKind::Flash { color_r, color_g, color_b, strength } => {
                p[0] = *color_r as f32;
                p[1] = *color_g as f32;
                p[2] = *color_b as f32;
                p[3] = *strength as f32;
                MODE_FLASH
            }
            TransitionKind::Dip { color_r, color_g, color_b } => {
                p[0] = *color_r as f32;
                p[1] = *color_g as f32;
                p[2] = *color_b as f32;
                MODE_DIP
            }
            TransitionKind::Blinds { count, angle, softness } => {
                // угол поворачивает ось полос — та же схема, что у wipe
                let a = (*angle as f32).to_radians();
                let (sn, cs) = (a.sin(), a.cos());
                p[0] = cs;
                p[1] = sn;
                p[2] = *count as f32;
                p[3] = *softness as f32;
                MODE_BLINDS
            }
            TransitionKind::Checker { cols, rows, softness } => {
                p[0] = *cols as f32;
                p[1] = *rows as f32;
                p[2] = *softness as f32;
                MODE_CHECKER
            }
            TransitionKind::Blocks { cols, rows, softness } => {
                p[0] = *cols as f32;
                p[1] = *rows as f32;
                p[2] = *softness as f32;
                MODE_BLOCKS
            }
            TransitionKind::Spiral { turns, cx, cy, softness } => {
                p[0] = *turns as f32;
                p[1] = *softness as f32;
                p[2] = *cx as f32;
                p[3] = *cy as f32;
                MODE_SPIRAL
            }
            TransitionKind::Split { vertical } => {
                p[0] = if *vertical { 1.0 } else { 0.0 };
                MODE_SPLIT
            }
            TransitionKind::Squeeze { dir } => {
                (p[0], p[1]) = d(*dir);
                MODE_SQUEEZE
            }
            TransitionKind::Stretch { vertical } => {
                p[0] = if *vertical { 1.0 } else { 0.0 };
                MODE_STRETCH
            }
            TransitionKind::Swap { vertical, depth } => {
                p[0] = if *vertical { 1.0 } else { 0.0 };
                p[1] = *depth as f32;
                MODE_SWAP
            }
            TransitionKind::Shake { amount, steps } => {
                p[0] = *amount as f32;
                p[1] = *steps as f32;
                MODE_SHAKE
            }
            TransitionKind::FilmBurn { scale, softness, tint_r, tint_g, tint_b } => {
                p[0] = *scale as f32;
                p[1] = *softness as f32;
                p[2] = *tint_r as f32;
                p[3] = *tint_g as f32;
                p[4] = *tint_b as f32;
                MODE_BURN
            }
            TransitionKind::LightLeaks { strength, tint_r, tint_g, tint_b } => {
                p[0] = *strength as f32;
                p[1] = *tint_r as f32;
                p[2] = *tint_g as f32;
                p[3] = *tint_b as f32;
                MODE_LEAKS
            }
            TransitionKind::ZoomBlur { amount, cx, cy } => {
                p[0] = *amount as f32;
                p[1] = *cx as f32;
                p[2] = *cy as f32;
                MODE_ZOOMBLUR
            }
        };
        TransMix { mode, p }
    }
}

/// Кадр как источник выборки. Кадр без альфы читается как непрозрачный —
/// та же политика, что у `dissolve_into`. `Clear` — ПУСТАЯ сторона
/// сольного фейда (прозрачность): благодаря ей фейд у края дорожки
/// считается тем же ядром, что и пара, и вайп «из ничего» работает.
enum Img<'a> {
    Frame { px: &'a [u8], w: i32, h: i32, has_alpha: bool },
    Clear,
}

impl Img<'_> {
    fn of(f: Option<&VideoFrame>) -> Img<'_> {
        match f {
            Some(f) => Img::Frame {
                px: &f.rgba,
                w: f.width as i32,
                h: f.height as i32,
                has_alpha: f.has_alpha,
            },
            None => Img::Clear,
        }
    }

    fn texel(&self, x: i32, y: i32) -> [f32; 4] {
        let Img::Frame { px, w, h, has_alpha } = self else { return [0.0; 4] };
        let x = x.clamp(0, w - 1);
        let y = y.clamp(0, h - 1);
        let i = ((y * w + x) * 4) as usize;
        [
            px[i] as f32,
            px[i + 1] as f32,
            px[i + 2] as f32,
            if *has_alpha { px[i + 3] as f32 } else { 255.0 },
        ]
    }

    /// Билинейная выборка по нормированным координатам (кламп по краям) —
    /// та же схема координат, что у `bilinear_into_scalar`.
    fn sample(&self, u: f32, v: f32) -> [f32; 4] {
        let Img::Frame { w, h, .. } = self else { return [0.0; 4] };
        let sx = u * *w as f32 - 0.5;
        let sy = v * *h as f32 - 0.5;
        let x = sx.floor();
        let y = sy.floor();
        let fx = sx - x;
        let fy = sy - y;
        let (x, y) = (x as i32, y as i32);
        let p00 = self.texel(x, y);
        let p10 = self.texel(x + 1, y);
        let p01 = self.texel(x, y + 1);
        let p11 = self.texel(x + 1, y + 1);
        let mut out = [0.0f32; 4];
        for c in 0..4 {
            let top = p00[c] * (1.0 - fx) + p10[c] * fx;
            let bot = p01[c] * (1.0 - fx) + p11[c] * fx;
            out[c] = top * (1.0 - fy) + bot * fy;
        }
        out
    }
}

fn mix4(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let mut o = [0.0f32; 4];
    for c in 0..4 {
        o[c] = a[c] + (b[c] - a[c]) * t;
    }
    o
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    if e1 <= e0 {
        return if x < e0 { 0.0 } else { 1.0 };
    }
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Фронт «маскирующих» переходов: 1 — уже показан B, 0 — ещё A.
/// `s` — координата фронта 0..1, `k` — прогресс 0..1.
fn front_mask(s: f32, k: f32, soft: f32) -> f32 {
    let soft = soft.clamp(0.0, 1.0);
    if soft < 1e-4 {
        return if s < k { 1.0 } else { 0.0 };
    }
    // фронт выезжает за оба края на ширину размытия — иначе на k=0 и k=1
    // оставалась бы видимая кромка
    let front = k * (1.0 + soft) - soft;
    1.0 - smoothstep(front, front + soft, s)
}

/// Целочисленный вес k (0..4096) в доли.
fn kf(k: u32) -> f32 {
    k as f32 / 4096.0
}

/// Хэш для «цифрового сбоя»: тот же целочисленный хэш, что у шума
/// kt-fx (и он же повторён в WGSL) — иначе сбой на CPU и GPU лёг бы
/// в разных местах и parity-тест ловил бы шум вместо ошибки.
fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

fn hash01(x: u32) -> f32 {
    (hash_u32(x) >> 8) as f32 / 16_777_216.0
}

/// Ядро перехода: `dst = mix(a, b)` по виду `m` с прогрессом `k`
/// (0..4096). Возвращает «в результате есть альфа». Кроссфейд сюда не
/// приходит — его считает целочисленный `dissolve_into`.
pub fn mix_into(
    dst: &mut Vec<u8>,
    a: Option<&VideoFrame>,
    b: Option<&VideoFrame>,
    k: u32,
    m: TransMix,
    w: usize,
    h: usize,
) -> bool {
    let n = w * h * 4;
    dst.clear();
    dst.resize(n, 0);
    let ia = Img::of(a);
    let ib = Img::of(b);
    let k = kf(k);
    let (fw, fh) = (w as f32, h as f32);
    let aspect = fw / fh;
    for y in 0..h {
        let v = (y as f32 + 0.5) / fh;
        for x in 0..w {
            let u = (x as f32 + 0.5) / fw;
            let px = kernel(&ia, &ib, u, v, k, m, aspect, fw, fh);
            let i = (y * w + x) * 4;
            for c in 0..4 {
                dst[i + c] = (px[c] + 0.5).clamp(0.0, 255.0) as u8;
            }
        }
    }
    true
}

/// Один пиксель перехода. Точное зеркало `trans()` из composite.wgsl.
#[allow(clippy::too_many_arguments)]
fn kernel(
    a: &Img<'_>,
    b: &Img<'_>,
    u: f32,
    v: f32,
    k: f32,
    m: TransMix,
    aspect: f32,
    fw: f32,
    fh: f32,
) -> [f32; 4] {
    let p = m.p;
    match m.mode {
        MODE_WIPE => {
            // проекция на направление, нормированная охватом квадрата
            let (dx, dy) = (p[0], p[1]);
            let ext = (dx.abs() + dy.abs()).max(1e-6);
            let s = ((u - 0.5) * dx + (v - 0.5) * dy + ext * 0.5) / ext;
            mix4(a.sample(u, v), b.sample(u, v), front_mask(s, k, p[2]))
        }
        MODE_IRIS => {
            // центр (p2, p3) произвольный (Р.51); rmax — расстояние до
            // САМОГО ДАЛЬНЕГО угла кадра в той же метрике, чтобы фронт
            // всегда накрывал кадр целиком
            let (cx, cy) = ((u - p[2]) * aspect, v - p[3]);
            let metric = |dx: f32, dy: f32| match p[0] as i32 {
                1 => dx.abs() + dy.abs(),
                2 => dx.abs().max(dy.abs()),
                _ => (dx * dx + dy * dy).sqrt(),
            };
            let r = metric(cx, cy);
            let ex = (p[2].max(1.0 - p[2])) * aspect;
            let ey = p[3].max(1.0 - p[3]);
            let rmax = metric(ex, ey).max(1e-6);
            mix4(a.sample(u, v), b.sample(u, v), front_mask(r / rmax, k, p[1]))
        }
        MODE_CLOCK => {
            let (cx, cy) = (u - p[2], v - p[3]);
            // 0 наверху, по часовой; reverse — против
            let mut ang = cx.atan2(-cy);
            if ang < 0.0 {
                ang += std::f32::consts::TAU;
            }
            if p[0] > 0.5 {
                ang = std::f32::consts::TAU - ang;
            }
            let s = ang / std::f32::consts::TAU;
            mix4(a.sample(u, v), b.sample(u, v), front_mask(s, k, p[1]))
        }
        MODE_BARN => {
            let s = if p[0] > 0.5 { (u - 0.5).abs() * 2.0 } else { (v - 0.5).abs() * 2.0 };
            // створки идут ОТ центра: фронт считается от него наружу
            mix4(a.sample(u, v), b.sample(u, v), front_mask(s, k, p[1]))
        }
        MODE_LUMA => {
            let c = a.sample(u, v);
            let mut l = (0.299 * c[0] + 0.587 * c[1] + 0.114 * c[2]) / 255.0;
            if p[0] > 0.5 {
                l = 1.0 - l;
            }
            mix4(c, b.sample(u, v), front_mask(l, k, p[1]))
        }
        MODE_PUSH => {
            // содержимое ЕДЕТ в сторону dir, поэтому пиксель экрана
            // показывает источник ПОЗАДИ себя на dir·k; B стоит вплотную
            // ЗА уходящим A — на целый экран дальше ПО направлению
            // движения (`+dir`, п.396: с `−dir` координата уходила за
            // [0,1) и кламп рисовал размазанную кромку вместо B)
            let (dx, dy) = (p[0], p[1]);
            let (au, av) = (u - dx * k, v - dy * k);
            if (0.0..1.0).contains(&au) && (0.0..1.0).contains(&av) {
                a.sample(au, av)
            } else {
                b.sample(au + dx, av + dy)
            }
        }
        MODE_SLIDE => {
            let (bu, bv) = (u + p[0] * (1.0 - k), v + p[1] * (1.0 - k));
            if (0.0..1.0).contains(&bu) && (0.0..1.0).contains(&bv) {
                b.sample(bu, bv)
            } else {
                a.sample(u, v)
            }
        }
        MODE_ZOOM => {
            let amt = p[0];
            let (ox, oy) = (p[1], p[2]);
            let za = 1.0 + amt * k; // уходящий разъезжается
            let zb = 1.0 + amt * (1.0 - k); // входящий налетает
            let sa = a.sample((u - ox) / za + ox, (v - oy) / za + oy);
            let sb = b.sample((u - ox) / zb + ox, (v - oy) / zb + oy);
            mix4(sa, sb, k)
        }
        MODE_SPIN => {
            let (turns, amt) = (p[0], p[1]);
            let (ox, oy) = (p[2], p[3]);
            let sa = rot_sample(
                a,
                u,
                v,
                turns * k * std::f32::consts::TAU,
                1.0 + amt * k,
                aspect,
                ox,
                oy,
            );
            let sb = rot_sample(
                b,
                u,
                v,
                -turns * (1.0 - k) * std::f32::consts::TAU,
                1.0 + amt * (1.0 - k),
                aspect,
                ox,
                oy,
            );
            mix4(sa, sb, k)
        }
        MODE_WHIP => {
            // пролёт как push, но со смазом вдоль движения; сила смаза
            // максимальна в середине жеста
            let (dx, dy) = (p[0], p[1]);
            let blur = p[2] * (std::f32::consts::PI * k).sin();
            let mut acc = [0.0f32; 4];
            for i in 0..WHIP_TAPS {
                let o = (i as f32 / (WHIP_TAPS - 1) as f32 - 0.5) * blur * 0.25;
                let (su, sv) = (u + dx * o, v + dy * o);
                let (au, av) = (su - dx * k, sv - dy * k);
                let c = if (0.0..1.0).contains(&au) && (0.0..1.0).contains(&av) {
                    a.sample(au, av)
                } else {
                    // B за уходящим A — `+dir`, как у push (п.396)
                    b.sample(au + dx, av + dy)
                };
                for ch in 0..4 {
                    acc[ch] += c[ch];
                }
            }
            for ch in acc.iter_mut() {
                *ch /= WHIP_TAPS as f32;
            }
            acc
        }
        MODE_BLUR => {
            // радиус растёт к середине перехода и там же гаснет
            let r = p[0] * (std::f32::consts::PI * k).sin();
            let sa = box9(a, u, v, r / fw, r / fh);
            let sb = box9(b, u, v, r / fw, r / fh);
            mix4(sa, sb, k)
        }
        MODE_PIXELATE => {
            let s = (p[0] * (std::f32::consts::PI * k).sin()).max(1.0);
            let (bu, bv) = (quant(u, s / fw), quant(v, s / fh));
            mix4(a.sample(bu, bv), b.sample(bu, bv), k)
        }
        MODE_GLITCH => {
            let amt = p[0] * (std::f32::consts::PI * k).sin();
            // полосы строк едут по горизонтали, каналы расходятся
            let band = (v * 24.0).floor() as u32;
            let shift = (hash01(band.wrapping_mul(1973)) - 0.5) * amt * 0.25;
            let ch = amt * 8.0 / fw;
            let (su, sv) = (u + shift, v);
            let pick = |img: &Img<'_>| {
                let r = img.sample(su + ch, sv);
                let g = img.sample(su, sv);
                let bl = img.sample(su - ch, sv);
                [r[0], g[1], bl[2], g[3]]
            };
            mix4(pick(a), pick(b), k)
        }
        MODE_FLASH => {
            let base = mix4(a.sample(u, v), b.sample(u, v), k);
            let w = (p[3] * (std::f32::consts::PI * k).sin()).clamp(0.0, 1.0);
            [
                base[0] + (p[0] - base[0]) * w,
                base[1] + (p[1] - base[1]) * w,
                base[2] + (p[2] - base[2]) * w,
                base[3],
            ]
        }
        MODE_DIP => {
            // две фазы: A → цвет, цвет → B (классический dip-to-color)
            let col = [p[0], p[1], p[2]];
            let (src, t) =
                if k < 0.5 { (a.sample(u, v), k * 2.0) } else { (b.sample(u, v), 2.0 - k * 2.0) };
            [
                src[0] + (col[0] - src[0]) * t,
                src[1] + (col[1] - src[1]) * t,
                src[2] + (col[2] - src[2]) * t,
                src[3],
            ]
        }
        MODE_BLINDS => {
            // проекция на повёрнутую ось (как wipe), фракция ВНУТРИ полосы:
            // все полосы открываются одновременно
            let (dx, dy) = (p[0], p[1]);
            let ext = (dx.abs() + dy.abs()).max(1e-6);
            let c = ((u - 0.5) * dx + (v - 0.5) * dy + ext * 0.5) / ext;
            let f = fracf(c * p[2].max(1.0));
            mix4(a.sample(u, v), b.sample(u, v), front_mask(f, k, p[3]))
        }
        MODE_CHECKER => {
            // локальный вайп в клетке, чётные и нечётные — навстречу
            let (cols, rows) = (p[0].max(1.0), p[1].max(1.0));
            let (i, j) = ((u * cols).floor(), (v * rows).floor());
            let fx = fracf(u * cols);
            let s = if ((i + j) as i32) % 2 == 0 { fx } else { 1.0 - fx };
            mix4(a.sample(u, v), b.sample(u, v), front_mask(s, k, p[2]))
        }
        MODE_BLOCKS => {
            // порядок клеток — хэш клетки: детерминизм без seed, тот же
            // приём, что у glitch. softness — пер-клеточный кроссфейд
            let (cols, rows) = (p[0].max(1.0), p[1].max(1.0));
            let (i, j) = ((u * cols).floor() as u32, (v * rows).floor() as u32);
            let s = hash01(hash_u32(i.wrapping_mul(1973)) ^ hash_u32(j.wrapping_mul(9277)));
            mix4(a.sample(u, v), b.sample(u, v), front_mask(s, k, p[2]))
        }
        MODE_SPIRAL => {
            // архимедов фронт: угол (как clock) + обороты по радиусу
            let (cx, cy) = (u - p[2], v - p[3]);
            let mut ang = cx.atan2(-cy);
            if ang < 0.0 {
                ang += std::f32::consts::TAU;
            }
            let theta = ang / std::f32::consts::TAU;
            let ex = (p[2].max(1.0 - p[2])) * aspect;
            let ey = p[3].max(1.0 - p[3]);
            let rmax = (ex * ex + ey * ey).sqrt().max(1e-6);
            let r = ((cx * aspect) * (cx * aspect) + cy * cy).sqrt() / rmax;
            let turns = p[0].max(0.25);
            let sfrac = (theta + turns * r) / (1.0 + turns);
            mix4(a.sample(u, v), b.sample(u, v), front_mask(sfrac, k, p[1]))
        }
        MODE_SPLIT => {
            // симбиоз: половинки A разъезжаются, B открывается в щели
            let c = if p[0] > 0.5 { v - 0.5 } else { u - 0.5 };
            let h = k * 0.5;
            if c.abs() >= h {
                let off = h * c.signum();
                if p[0] > 0.5 { a.sample(u, v - off) } else { a.sample(u - off, v) }
            } else {
                b.sample(u, v)
            }
        }
        MODE_SQUEEZE => {
            // симбиоз: A сжимается к дальнему краю, B расправляется от
            // ближнего; движение вдоль dir
            let (dx, dy) = (p[0], p[1]);
            let c = if dx > 0.5 {
                u
            } else if dx < -0.5 {
                1.0 - u
            } else if dy > 0.5 {
                v
            } else {
                1.0 - v
            };
            let back = |c2: f32| -> (f32, f32) {
                if dx > 0.5 {
                    (c2, v)
                } else if dx < -0.5 {
                    (1.0 - c2, v)
                } else if dy > 0.5 {
                    (u, c2)
                } else {
                    (u, 1.0 - c2)
                }
            };
            if c < k {
                let (su, sv) = back(c / k.max(1e-4));
                b.sample(su, sv)
            } else {
                let (su, sv) = back((c - k) / (1.0 - k).max(1e-4));
                a.sample(su, sv)
            }
        }
        MODE_STRETCH => {
            // симбиоз: B растягивается из центральной линии поверх A
            let c = if p[0] > 0.5 { v - 0.5 } else { u - 0.5 };
            let h = k * 0.5;
            if c.abs() < h {
                let c2 = c / k.max(1e-4) + 0.5;
                if p[0] > 0.5 { b.sample(u, c2) } else { b.sample(c2, v) }
            } else {
                a.sample(u, v)
            }
        }
        MODE_SWAP => {
            // 2.5D-обмен: стороны едут по дуге, сжимаясь вглубь; порядок
            // переключается на середине — окклюзия аналитическая, без
            // глубины (depth-буфера в пайплайне нет и не надо)
            let vert = p[0] > 0.5;
            let wave = (std::f32::consts::PI * k).sin();
            let off = 0.3 * wave;
            let scale = (1.0 - p[1] * wave).max(0.05);
            let quad = |img: &Img<'_>, side: f32| -> [f32; 4] {
                let (qx, qy) = if vert { (0.5, 0.5 + side * off) } else { (0.5 + side * off, 0.5) };
                let su = (u - qx) / scale + 0.5;
                let sv = (v - qy) / scale + 0.5;
                if (0.0..1.0).contains(&su) && (0.0..1.0).contains(&sv) {
                    let mut c = img.sample(su, sv);
                    // дальняя сторона чуть темнее — глубина читается
                    let shade = 1.0 - 0.25 * wave;
                    c[0] *= shade;
                    c[1] *= shade;
                    c[2] *= shade;
                    c
                } else {
                    [0.0; 4]
                }
            };
            let sa = quad(a, 1.0);
            let sb = quad(b, -1.0);
            // k < 0.5 — уходящий ещё СВЕРХУ, дальше — входящий
            let (top, bot) = if k < 0.5 { (sa, sb) } else { (sb, sa) };
            over_straight(top, bot)
        }
        MODE_SHAKE => {
            // рывки кадра: индекс шага по прогрессу, кадр — катом на 0.5
            let steps = p[1].max(1.0);
            let i = ((k * steps) as u32).min(steps as u32 - 1);
            let amp = p[0] * 0.2 * (std::f32::consts::PI * k).sin();
            let ox = (hash01(i.wrapping_mul(2).wrapping_add(1)) - 0.5) * amp;
            let oy = (hash01(i.wrapping_mul(2).wrapping_add(2)) - 0.5) * amp;
            let img = if k < 0.5 { a } else { b };
            img.sample(u + ox, v + oy)
        }
        MODE_BURN => {
            // фронт по крупному шуму, на кромке — горячая примесь цвета;
            // вклад гейтится альфой (иначе соло-фейд подсветил бы пустоту)
            let n = value_noise(u * p[0] * aspect, v * p[0]);
            let mask = front_mask(n, k, p[1].max(0.02));
            let base = mix4(a.sample(u, v), b.sample(u, v), mask);
            let rim = (mask * (1.0 - mask) * 4.0).clamp(0.0, 1.0);
            let gate = rim * base[3] / 255.0;
            [
                base[0] + (p[2] - base[0]) * gate,
                base[1] + (p[3] - base[1]) * gate,
                base[2] + (p[4] - base[2]) * gate,
                base[3],
            ]
        }
        MODE_LEAKS => {
            // два цветных пятна пролетают по диагоналям поверх растворения
            let base = mix4(a.sample(u, v), b.sample(u, v), k);
            let wave = (std::f32::consts::PI * k).sin();
            let blob = |bx: f32, by: f32| -> f32 {
                let dx = (u - bx) * aspect;
                let dy = v - by;
                let d = (dx * dx + dy * dy).sqrt() / 0.45;
                let w = (1.0 - d).max(0.0);
                w * w
            };
            let w = (blob(k * 1.2 - 0.1, 0.3 + 0.4 * k) + blob(1.1 - 1.2 * k, 0.7 - 0.4 * k))
                * p[0]
                * wave;
            let gate = w.clamp(0.0, 1.0) * base[3] / 255.0;
            [
                base[0] + (p[1] - base[0]) * gate,
                base[1] + (p[2] - base[1]) * gate,
                base[2] + (p[3] - base[2]) * gate,
                base[3],
            ]
        }
        MODE_ZOOMBLUR => {
            // лучи от центра: 7 тапов вдоль луча, сторона — катом на 0.5
            let (ox, oy) = (p[1], p[2]);
            let amt = p[0] * (std::f32::consts::PI * k).sin();
            let img = if k < 0.5 { a } else { b };
            let mut acc = [0.0f32; 4];
            for i in 0..WHIP_TAPS {
                let z = 1.0 + amt * 0.6 * (i as f32 / (WHIP_TAPS - 1) as f32);
                let c = img.sample((u - ox) / z + ox, (v - oy) / z + oy);
                for ch in 0..4 {
                    acc[ch] += c[ch];
                }
            }
            for ch in acc.iter_mut() {
                *ch /= WHIP_TAPS as f32;
            }
            acc
        }
        _ => mix4(a.sample(u, v), b.sample(u, v), k),
    }
}

const WHIP_TAPS: usize = 7;

/// Дробная часть «как в WGSL»: x − floor(x) (у f32::fract другой знак
/// на отрицательных — зеркала разъехались бы).
fn fracf(x: f32) -> f32 {
    x - x.floor()
}

/// Straight-alpha «top поверх bottom» в f32 — для 2.5D-обмена, где
/// стороны движутся раздельно и появляется настоящая прозрачность.
fn over_straight(top: [f32; 4], bot: [f32; 4]) -> [f32; 4] {
    let ta = (top[3] / 255.0).clamp(0.0, 1.0);
    let ba = (bot[3] / 255.0).clamp(0.0, 1.0);
    let oa = ta + ba * (1.0 - ta);
    if oa <= 0.0 {
        return [0.0; 4];
    }
    let mut o = [0.0f32; 4];
    for c in 0..3 {
        o[c] = (top[c] * ta + bot[c] * ba * (1.0 - ta)) / oa;
    }
    o[3] = oa * 255.0;
    o
}

/// Клеточный value-noise на общем целочисленном хэше (тот же, что у
/// glitch/blocks) — WGSL повторяет дословно.
fn value_noise(x: f32, y: f32) -> f32 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let h = |i: f32, j: f32| -> f32 {
        let (i, j) = (i as i32 as u32, j as i32 as u32);
        hash01(hash_u32(i.wrapping_mul(1973)) ^ hash_u32(j.wrapping_mul(9277)))
    };
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let top = h(xi, yi) + (h(xi + 1.0, yi) - h(xi, yi)) * sx;
    let bot = h(xi, yi + 1.0) + (h(xi + 1.0, yi + 1.0) - h(xi, yi + 1.0)) * sx;
    top + (bot - top) * sy
}

fn quant(x: f32, step: f32) -> f32 {
    if step <= 0.0 {
        return x;
    }
    (x / step).floor() * step + step * 0.5
}

#[allow(clippy::too_many_arguments)]
fn rot_sample(
    img: &Img<'_>,
    u: f32,
    v: f32,
    ang: f32,
    zoom: f32,
    aspect: f32,
    ox: f32,
    oy: f32,
) -> [f32; 4] {
    let (s, c) = (ang.sin(), ang.cos());
    let (cx, cy) = ((u - ox) * aspect / zoom, (v - oy) / zoom);
    let rx = cx * c - cy * s;
    let ry = cx * s + cy * c;
    img.sample(rx / aspect + ox, ry + oy)
}

/// Дешёвое размытие 9 отсчётами (3×3 с шагом r) — форма выбрана так,
/// чтобы точно повторяться в WGSL.
fn box9(img: &Img<'_>, u: f32, v: f32, rx: f32, ry: f32) -> [f32; 4] {
    if rx <= 0.0 && ry <= 0.0 {
        return img.sample(u, v);
    }
    let mut acc = [0.0f32; 4];
    for dy in -1..=1 {
        for dx in -1..=1 {
            let c = img.sample(u + dx as f32 * rx, v + dy as f32 * ry);
            for ch in 0..4 {
                acc[ch] += c[ch];
            }
        }
    }
    for ch in acc.iter_mut() {
        *ch /= 9.0;
    }
    acc
}
