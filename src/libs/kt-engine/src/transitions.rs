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
            TransitionKind::Iris { shape, softness } => {
                p[0] = match shape {
                    IrisShape::Circle => 0.0,
                    IrisShape::Diamond => 1.0,
                    IrisShape::Box => 2.0,
                };
                p[1] = *softness as f32;
                MODE_IRIS
            }
            TransitionKind::Clock { reverse, softness } => {
                p[0] = if *reverse { 1.0 } else { 0.0 };
                p[1] = *softness as f32;
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
            TransitionKind::Zoom { amount } => {
                p[0] = *amount as f32;
                MODE_ZOOM
            }
            TransitionKind::Spin { turns, amount } => {
                p[0] = *turns as f32;
                p[1] = *amount as f32;
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
            let (cx, cy) = ((u - 0.5) * aspect, v - 0.5);
            let r = match p[0] as i32 {
                1 => cx.abs() + cy.abs(),
                2 => cx.abs().max(cy.abs()),
                _ => (cx * cx + cy * cy).sqrt(),
            };
            // радиус, накрывающий кадр целиком
            let rmax = match p[0] as i32 {
                1 => (aspect + 1.0) * 0.5,
                2 => (aspect * 0.5).max(0.5),
                _ => ((aspect * aspect + 1.0) * 0.25).sqrt(),
            };
            mix4(a.sample(u, v), b.sample(u, v), front_mask(r / rmax, k, p[1]))
        }
        MODE_CLOCK => {
            let (cx, cy) = (u - 0.5, v - 0.5);
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
            // показывает источник ПОЗАДИ себя на dir·k
            let (dx, dy) = (p[0], p[1]);
            let (au, av) = (u - dx * k, v - dy * k);
            if (0.0..1.0).contains(&au) && (0.0..1.0).contains(&av) {
                a.sample(au, av)
            } else {
                b.sample(au - dx, av - dy)
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
            let za = 1.0 + amt * k; // уходящий разъезжается
            let zb = 1.0 + amt * (1.0 - k); // входящий налетает
            let sa = a.sample((u - 0.5) / za + 0.5, (v - 0.5) / za + 0.5);
            let sb = b.sample((u - 0.5) / zb + 0.5, (v - 0.5) / zb + 0.5);
            mix4(sa, sb, k)
        }
        MODE_SPIN => {
            let (turns, amt) = (p[0], p[1]);
            let sa = rot_sample(a, u, v, turns * k * std::f32::consts::TAU, 1.0 + amt * k, aspect);
            let sb = rot_sample(
                b,
                u,
                v,
                -turns * (1.0 - k) * std::f32::consts::TAU,
                1.0 + amt * (1.0 - k),
                aspect,
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
                    b.sample(au - dx, av - dy)
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
        _ => mix4(a.sample(u, v), b.sample(u, v), k),
    }
}

const WHIP_TAPS: usize = 7;

fn quant(x: f32, step: f32) -> f32 {
    if step <= 0.0 {
        return x;
    }
    (x / step).floor() * step + step * 0.5
}

fn rot_sample(img: &Img<'_>, u: f32, v: f32, ang: f32, zoom: f32, aspect: f32) -> [f32; 4] {
    let (s, c) = (ang.sin(), ang.cos());
    let (cx, cy) = ((u - 0.5) * aspect / zoom, (v - 0.5) / zoom);
    let rx = cx * c - cy * s;
    let ry = cx * s + cy * c;
    img.sample(rx / aspect + 0.5, ry + 0.5)
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
