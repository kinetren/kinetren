//! Видео-FX (порт bc_fx.cpp, п.25): 10 эффектов v1, RGBA8 in/out, CPU.
//! Порядок применения = порядок в массиве fx (первый — ближайший к
//! декодеру). Один код для превью и экспорта (инвариант №7).
//! Детерминизм: noise/vhs зависят только от timeline_burst_index.

use kt_model::{CamKeyframe, FxInstance, PanMode};

/// Контекст кадра для цепочки.
#[derive(Debug, Clone, Copy)]
pub struct VideoFxCtx {
    /// Целевой размер (разрешение проекта/превью).
    pub out_w: u32,
    pub out_h: u32,
    /// Прогресс клипа 0..1 (для CamMotion).
    pub u: f64,
    /// Абсолютная позиция таймлайна в тиках (для noise/vhs).
    pub burst_index: i64,
}

/// Кадр в работе.
pub struct Frame {
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
    pub has_alpha: bool,
}

/// Пул RGBA-буферов цепочки (п.2 р.7): раньше каждый эффект аллоцировал
/// полноразмерный буфер на КАЖДЫЙ кадр (Mirror+Grid+Glow+VHS ≈ 6 аллокаций
/// и 5 полных проходов на compose). take() отдаёт занулённый буфер, потому
/// что edges/event_grid/pan_zoom полагаются на нулевую инициализацию.
/// Арифметика эффектов не изменена: golden-тест 26 байт-в-байт.
#[derive(Default)]
pub struct FxScratch {
    bufs: Vec<Vec<u8>>,
}

impl FxScratch {
    pub fn take(&mut self, len: usize) -> Vec<u8> {
        let mut b = self.bufs.pop().unwrap_or_default();
        b.clear();
        b.resize(len, 0);
        b
    }
    pub fn put(&mut self, b: Vec<u8>) {
        if self.bufs.len() < 8 {
            self.bufs.push(b);
        }
    }
}

/// Применить цепочку видео-FX клипа. Аудио-варианты игнорируются.
/// Обёртка с локальным пулом — результат идентичен pooled-варианту.
pub fn apply_video_chain(fx: &[FxInstance], frame: Frame, ctx: &VideoFxCtx) -> Frame {
    let mut scratch = FxScratch::default();
    apply_video_chain_pooled(fx, frame, ctx, &mut scratch)
}

/// Вариант с переиспользуемым пулом буферов (превью зовёт цепочку на каждый
/// новый кадр — держатель пула живёт в VideoRenderer).
pub fn apply_video_chain_pooled(
    fx: &[FxInstance],
    mut frame: Frame,
    ctx: &VideoFxCtx,
    scratch: &mut FxScratch,
) -> Frame {
    for f in fx {
        frame = match f {
            FxInstance::Pan { mode, pan_x, pan_y, zoom } => {
                let nf = pan_zoom(&frame, ctx, *mode, *pan_x, *pan_y, *zoom, scratch);
                scratch.put(frame.rgba);
                nf
            }
            FxInstance::CamMotion { mode, ease, time_scale, keyframes } => {
                let (px, py, z) = cam_at(keyframes, ctx.u, *ease, *time_scale);
                let nf = pan_zoom(&frame, ctx, *mode, px, py, z, scratch);
                scratch.put(frame.rgba);
                nf
            }
            FxInstance::Mirror { horizontal, vertical } => mirror(frame, *horizontal, *vertical),
            FxInstance::Blur { radius, radius_y } => blur(frame, *radius, *radius_y, scratch),
            FxInstance::Greenscreen { key_r, key_g, key_b, tolerance, softness, spill } => {
                greenscreen(frame, [*key_r, *key_g, *key_b], *tolerance, *softness, *spill)
            }
            FxInstance::Edges { threshold, thickness, color_r, color_g, color_b } => edges(
                frame,
                *threshold,
                *thickness,
                [*color_r, *color_g, *color_b],
                scratch,
            ),
            FxInstance::Noise { amount, size, color } => {
                noise(frame, *amount, *size, *color, ctx.burst_index)
            }
            FxInstance::Vhs { rgb_shift, bleed, scan_step, jitter } => {
                vhs(frame, *rgb_shift, *bleed, *scan_step, *jitter, ctx.burst_index, scratch)
            }
            FxInstance::Glow { strength, radius, threshold, tint_r, tint_g, tint_b } => glow(
                frame,
                *strength,
                *radius,
                *threshold,
                [*tint_r, *tint_g, *tint_b],
                scratch,
            ),
            FxInstance::EventGrid { cols, rows, center } => {
                let nf = event_grid(&frame, ctx, *cols, *rows, *center, scratch);
                scratch.put(frame.rgba);
                nf
            }
            _ => frame, // аудио-FX
        };
    }
    frame
}

/// Есть ли в цепочке видео-эффекты (иначе композит идёт без копий).
pub fn has_video_fx(fx: &[FxInstance]) -> bool {
    // сила слоя (Р.38 п.12) — не пиксельный фильтр, а правило смешения
    // входа цепочки с выходом; её применяет kt-engine ПОСЛЕ цепочки.
    // Цепочка из одной только силы пикселей не трогает вовсе.
    fx.iter().any(|f| !f.is_audio() && !f.is_layer_mix())
}

/// Вписать кадр в `ow`×`oh` ТОЙ ЖЕ билинейкой, что и все остальные
/// ресемплы (её зеркало в kt-gpu — `Resample` с рамкой во весь кадр).
/// Нужна силе слоя: смешивать вход цепочки с её выходом можно только на
/// одном размере, а Pan/CamMotion размер меняют.
pub fn fit_into(src: &Frame, ow: u32, oh: u32, out: &mut [u8]) {
    bilinear_into(src, 0.0, 0.0, src.w as f64, src.h as f64, ow, oh, out);
}

// ---- пан/зум ---------------------------------------------------------------

/// Интерполяция кейфреймов CamMotion (v1: smoothstep внутри сегмента).
/// Pub (Р.29): GPU-путь считает рамку тем же кодом, в шейдер — юниформы.
pub fn cam_at(keys: &[CamKeyframe], u: f64, ease: bool, time_scale: f64) -> (f64, f64, f64) {
    // дефолт v1: наезд 1.0 → 1.35
    let default = [
        CamKeyframe { t: 0.0, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 },
        CamKeyframe { t: 1.0, pan_x: 0.0, pan_y: 0.0, zoom: 1.35 },
    ];
    let keys: &[CamKeyframe] = if keys.is_empty() { &default } else { keys };
    let u = (u * time_scale.clamp(0.05, 8.0)).clamp(0.0, 1.0);
    if u <= keys[0].t {
        let k = &keys[0];
        return (k.pan_x, k.pan_y, k.zoom);
    }
    for w in keys.windows(2) {
        let (a, b) = (&w[0], &w[1]);
        if u <= b.t {
            let span = (b.t - a.t).max(1e-9);
            let mut s = ((u - a.t) / span).clamp(0.0, 1.0);
            if ease {
                s = s * s * (3.0 - 2.0 * s); // smoothstep01
            }
            return (
                a.pan_x + (b.pan_x - a.pan_x) * s,
                a.pan_y + (b.pan_y - a.pan_y) * s,
                a.zoom + (b.zoom - a.zoom) * s,
            );
        }
    }
    let k = keys.last().unwrap();
    (k.pan_x, k.pan_y, k.zoom)
}

/// Кроп-рамка Pan/CamMotion в координатах источника: (x0, y0, cw, ch).
/// Pub (Р.29): ОБЩАЯ математика CPU- и GPU-путей — GPU считает рамку этой
/// же функцией и передаёт её в шейдер юниформами.
#[allow(clippy::too_many_arguments)]
pub fn pan_frame(
    src_w: u32,
    src_h: u32,
    out_w: u32,
    out_h: u32,
    mode: PanMode,
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
) -> (f64, f64, f64, f64) {
    let (ow, oh) = (out_w.max(2), out_h.max(2));
    let zoom = zoom.clamp(0.05, 32.0);
    let (sw, sh) = (src_w as f64, src_h as f64);
    let ar_out = ow as f64 / oh as f64;

    // базовая рамка в координатах источника
    let (mut cw, mut ch) = match mode {
        PanMode::Stretch => (sw, sh),
        PanMode::Fill => {
            // кроп по короткой стороне до ar_out
            if sw / sh > ar_out { (sh * ar_out, sh) } else { (sw, sw / ar_out) }
        }
        PanMode::Fit => {
            // наибольший прямоугольник с ar_out ВНУТРИ источника
            if sw / sh > ar_out { (sh * ar_out, sh) } else { (sw, sw / ar_out) }
        }
    };
    cw /= zoom;
    ch /= zoom;
    // пан: −1..1 двигает рамку в пределах свободного места
    let max_dx = (sw - cw) / 2.0;
    let max_dy = (sh - ch) / 2.0;
    let cx = sw / 2.0 + pan_x.clamp(-4.0, 4.0) * max_dx;
    let cy = sh / 2.0 + pan_y.clamp(-4.0, 4.0) * max_dy;
    let x0 = (cx - cw / 2.0).clamp(0.0, sw - cw.min(sw));
    let y0 = (cy - ch / 2.0).clamp(0.0, sh - ch.min(sh));
    (x0, y0, cw, ch)
}

/// Кроп-рамка + билинейный ресемпл в out (v1 composePanZoomOutput).
fn pan_zoom(
    src: &Frame,
    ctx: &VideoFxCtx,
    mode: PanMode,
    pan_x: f64,
    pan_y: f64,
    zoom: f64,
    scratch: &mut FxScratch,
) -> Frame {
    let (ow, oh) = (ctx.out_w.max(2), ctx.out_h.max(2));
    let (x0, y0, cw, ch) = pan_frame(src.w, src.h, ctx.out_w, ctx.out_h, mode, pan_x, pan_y, zoom);
    let mut out = scratch.take((ow * oh * 4) as usize);
    bilinear_into(src, x0, y0, cw, ch, ow, oh, &mut out);
    Frame { w: ow, h: oh, rgba: out, has_alpha: src.has_alpha }
}

/// Билинейная выборка рамки [x0,y0,cw,ch] источника в буфер ow×oh.
///
/// Диспетчер SIMD-уровня (Р.32): AVX2-ядро обязано быть байт-в-байт
/// равно скаляру, поэтому выбор пути не влияет ни на один golden.
/// Ветка стоит на ВЫЗОВ (кадр/слой), не на пиксель.
#[allow(clippy::too_many_arguments)]
fn bilinear_into(
    src: &Frame,
    x0: f64,
    y0: f64,
    cw: f64,
    ch: f64,
    ow: u32,
    oh: u32,
    out: &mut [u8],
) {
    #[cfg(target_arch = "x86_64")]
    if crate::simd::simd_level() == crate::simd::SimdLevel::Avx2
        // страховка контракта unsafe-ядра: буферы обязаны быть полными
        && src.rgba.len() >= (src.w as usize) * (src.h as usize) * 4
        && out.len() >= (ow as usize) * (oh as usize) * 4
    {
        // SAFETY: уровень проверен рантайм-детектом, длины буферов — выше
        unsafe {
            crate::video_simd::bilinear_into_avx2(src, x0, y0, cw, ch, ow, oh, out);
        }
        return;
    }
    bilinear_into_scalar(src, x0, y0, cw, ch, ow, oh, out);
}

/// Скалярный эталон билинейки. НЕ рефакторится вместе с SIMD-ядром:
/// он и есть определение байтов CPU-пути (инвариант №11).
#[allow(clippy::too_many_arguments)]
pub(crate) fn bilinear_into_scalar(
    src: &Frame,
    x0: f64,
    y0: f64,
    cw: f64,
    ch: f64,
    ow: u32,
    oh: u32,
    out: &mut [u8],
) {
    let (sw, sh) = (src.w as i64, src.h as i64);
    for oy in 0..oh as i64 {
        let sy = y0 + (oy as f64 + 0.5) / oh as f64 * ch - 0.5;
        let y = sy.floor() as i64;
        let fy = sy - y as f64;
        for ox in 0..ow as i64 {
            let sx = x0 + (ox as f64 + 0.5) / ow as f64 * cw - 0.5;
            let x = sx.floor() as i64;
            let fx = sx - x as f64;
            let mut px = [0f64; 4];
            for (dy, wy) in [(0i64, 1.0 - fy), (1, fy)] {
                for (dx, wx) in [(0i64, 1.0 - fx), (1, fx)] {
                    let xi = (x + dx).clamp(0, sw - 1);
                    let yi = (y + dy).clamp(0, sh - 1);
                    let si = ((yi * sw + xi) * 4) as usize;
                    let w = wy * wx;
                    for (c, acc) in px.iter_mut().enumerate() {
                        *acc += src.rgba[si + c] as f64 * w;
                    }
                }
            }
            let oi = ((oy * ow as i64 + ox) * 4) as usize;
            for c in 0..4 {
                out[oi + c] = px[c].round().clamp(0.0, 255.0) as u8;
            }
        }
    }
}

// ---- простые ---------------------------------------------------------------

fn mirror(mut f: Frame, h: bool, v: bool) -> Frame {
    if !h && !v {
        return f;
    }
    // in-place свопы вместо второго буфера (п.2 р.7); результат байт-в-байт
    // равен прежнему out[y][x] = src[sy][sx]
    let (w, hh) = (f.w as usize, f.h as usize);
    let px = |x: usize, y: usize| (y * w + x) * 4;
    if h && v {
        // поворот на 180°: пиксель p ↔ total-1-p, середина неподвижна
        let total = w * hh;
        for p in 0..total / 2 {
            let (i, j) = (p * 4, (total - 1 - p) * 4);
            for c in 0..4 {
                f.rgba.swap(i + c, j + c);
            }
        }
    } else if h {
        for y in 0..hh {
            for x in 0..w / 2 {
                let (i, j) = (px(x, y), px(w - 1 - x, y));
                for c in 0..4 {
                    f.rgba.swap(i + c, j + c);
                }
            }
        }
    } else {
        for y in 0..hh / 2 {
            let (i, j) = (px(0, y), px(0, hh - 1 - y));
            for c in 0..w * 4 {
                f.rgba.swap(i + c, j + c);
            }
        }
    }
    f
}

/// «Дешёвое» размытие v1: даунскейл в step раз и обратно (билинейно).
/// Размытие даунскейлом. `radius_y == 0` — «как по горизонтали»: ровно
/// прежний путь (один шаг на обе оси), поэтому golden'ы не двигаются;
/// разные радиусы дают смаз по одной оси (Р.33 С5).
fn blur(f: Frame, radius: u32, radius_y: u32, scratch: &mut FxScratch) -> Frame {
    let radius = radius.clamp(1, 48);
    let step = (radius / 3).max(1) + 1;
    let step_y = if radius_y == 0 { step } else { (radius_y.clamp(1, 48) / 3).max(1) + 1 };
    let (dw, dh) = ((f.w / step).max(1), (f.h / step_y).max(1));
    let mut small = scratch.take((dw * dh * 4) as usize);
    bilinear_into(&f, 0.0, 0.0, f.w as f64, f.h as f64, dw, dh, &mut small);
    let small_f = Frame { w: dw, h: dh, rgba: small, has_alpha: f.has_alpha };
    let mut out = scratch.take(f.rgba.len());
    bilinear_into(&small_f, 0.0, 0.0, dw as f64, dh as f64, f.w, f.h, &mut out);
    scratch.put(small_f.rgba);
    scratch.put(f.rgba);
    Frame { w: f.w, h: f.h, rgba: out, has_alpha: f.has_alpha }
}

fn greenscreen(
    mut f: Frame,
    key: [u8; 3],
    tolerance: u32,
    softness: u32,
    spill: f64,
) -> Frame {
    let tol = tolerance.clamp(1, 200) as f64;
    let soft = softness.clamp(0, 100) as f64;
    let hard = (tol - soft).max(0.0);
    // подавление засветки (Р.33 С5): доминирующий канал КЛЮЧА тянется к
    // большему из соседних. spill = 0 — прежний путь байт-в-байт
    let spill = spill.clamp(0.0, 1.0);
    let kc = (0..3).max_by_key(|&i| key[i]).unwrap_or(1);
    // greenscreen оставлен СКАЛЯРОМ осознанно (Р.32): замер показал
    // всего ×1.29 при пороге 1.3 — LLVM уже неплохо векторизует этот
    // цикл сам, а 60 строк интринзиков в месте, где важна побайтовая
    // точность, того не стоят. Цифры — в porting-notes п.198.
    for px in f.rgba.chunks_exact_mut(4) {
        let d = ((px[0] as f64 - key[0] as f64).powi(2)
            + (px[1] as f64 - key[1] as f64).powi(2)
            + (px[2] as f64 - key[2] as f64).powi(2))
        .sqrt();
        if d <= hard {
            px[3] = 0;
        } else if d < tol {
            let a = (d - hard) / (tol - hard).max(1e-9);
            px[3] = (px[3] as f64 * a).round() as u8;
        }
        if spill > 0.0 && px[3] > 0 {
            let other = px[(kc + 1) % 3].max(px[(kc + 2) % 3]) as f64;
            let v = px[kc] as f64;
            if v > other {
                px[kc] = (v - (v - other) * spill).round() as u8;
            }
        }
    }
    f.has_alpha = true;
    f
}

/// Собель по яркости (v1: (77R+150G+29B)>>8, порог, серый выход).
fn edges(
    f: Frame,
    threshold: u32,
    thickness: u32,
    color: [u8; 3],
    scratch: &mut FxScratch,
) -> Frame {
    let th = threshold.clamp(1, 255) as i32;
    // толщина = шаг выборки Собеля; 1 и белый цвет — прежнее ядро 3×3
    // байт-в-байт (в том числе AVX2-путь)
    let d = thickness.clamp(1, 8) as i64;
    let plain = d == 1 && color == [255, 255, 255];
    let (w, h) = (f.w as usize, f.h as usize);
    #[cfg(target_arch = "x86_64")]
    let simd = crate::simd::simd_level() == crate::simd::SimdLevel::Avx2
        && f.rgba.len() >= w * h * 4;
    #[cfg(not(target_arch = "x86_64"))]
    let simd = false;
    let luma: Vec<i32> = if simd {
        let mut v = vec![0i32; w * h];
        #[cfg(target_arch = "x86_64")]
        // SAFETY: уровень проверен детектом, длины буферов — выше
        unsafe {
            crate::video_simd::luma_avx2(&mut v, &f.rgba, w * h)
        };
        v
    } else {
        f.rgba
            .chunks_exact(4)
            .map(|p| (77 * p[0] as i32 + 150 * p[1] as i32 + 29 * p[2] as i32) >> 8)
            .collect()
    };
    let mut out = scratch.take(f.rgba.len());
    for p in out.chunks_exact_mut(4) {
        p[3] = 255;
    }
    let m = d as usize; // кайма шире при толстой линии
    for y in m..h.saturating_sub(m) {
        // векторная часть строки; скалярный цикл ниже дорисовывает хвост
        let mut x_from = m;
        #[cfg(target_arch = "x86_64")]
        if plain && simd && w > 2 {
            // SAFETY: уровень проверен; строки y-1..y+1 существуют, out полон
            x_from = unsafe {
                crate::video_simd::sobel_row_avx2(&mut out, &luma, w, y, th, w - 1)
            };
        }
        for x in x_from..w.saturating_sub(m) {
            let l = |dx: i64, dy: i64| {
                luma[(y as i64 + dy * d) as usize * w + (x as i64 + dx * d) as usize]
            };
            let gx = -l(-1, -1) - 2 * l(-1, 0) - l(-1, 1) + l(1, -1) + 2 * l(1, 0) + l(1, 1);
            let gy = -l(-1, -1) - 2 * l(0, -1) - l(1, -1) + l(-1, 1) + 2 * l(0, 1) + l(1, 1);
            let mag = (((gx * gx + gy * gy) as f64).sqrt() as i32).min(255);
            let v = if mag < th { 0 } else { mag as u8 };
            let i = (y * w + x) * 4;
            // цвет линий: серая величина домножается на него (255 — прежний)
            for c in 0..3 {
                out[i + c] = ((v as u32 * color[c] as u32) / 255) as u8;
            }
        }
    }
    scratch.put(f.rgba);
    Frame { w: f.w, h: f.h, rgba: out, has_alpha: false }
}

/// Финализатор lowbias32 (Chris Wellons): целочисленный avalanche-хэш,
/// побитно воспроизводимый в WGSL (u32-арифметика без float).
#[inline]
pub fn noise_hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846ca68b);
    x ^= x >> 16;
    x
}

/// Зерно кадра для noise: одна величина на кадр, в GPU-путь уходит юниформой.
#[inline]
pub fn noise_seed(burst_index: i64) -> u32 {
    noise_hash(burst_index as u32)
}

/// Детерминированное зерно. Р.29: counter-based hash вместо
/// последовательного LCG v1 — зерно каждого пикселя считается из (x, y,
/// seed) независимо, порядок обхода не важен → та же формула в WGSL даёт
/// байт-в-байт тот же шум на GPU. Санкционированная смена картинки шума
/// (перегенерирован только noise-эталон t26).
/// Зерно. `size == 1` и `color == false` — прежняя формула байт-в-байт
/// (в том числе AVX2-путь); крупное зерно и цветной шум идут скаляром.
fn noise(mut f: Frame, amount: f64, size: u32, color: bool, burst_index: i64) -> Frame {
    let k = (amount.clamp(0.0, 1.0) * 32.0) as i64;
    if k == 0 {
        return f;
    }
    let seed = noise_seed(burst_index);
    let w = f.w;
    let size = size.clamp(1, 64);
    if size > 1 || color {
        let off_at = |x: u32, y: u32, ch: u32| -> i32 {
            let h = noise_hash(x ^ noise_hash(y ^ noise_hash(ch ^ seed)));
            let n = ((h >> 16) & 0x7fff) as i64 - 16384;
            (n * k / 32767) as i32
        };
        for (p, px) in f.rgba.chunks_exact_mut(4).enumerate() {
            let (x, y) = (p as u32 % w / size, p as u32 / w / size);
            if color {
                for (c, v) in px.iter_mut().take(3).enumerate() {
                    *v = (*v as i32 + off_at(x, y, c as u32 + 1)).clamp(0, 255) as u8;
                }
            } else {
                let off = off_at(x, y, 0);
                for v in px.iter_mut().take(3) {
                    *v = (*v as i32 + off).clamp(0, 255) as u8;
                }
            }
        }
        return f;
    }
    #[cfg(target_arch = "x86_64")]
    if crate::simd::simd_level() == crate::simd::SimdLevel::Avx2
        && w >= 8
        && f.rgba.len() >= (w as usize) * (f.h as usize) * 4
    {
        // SAFETY: уровень проверен детектом, длина буфера — выше
        unsafe { crate::video_simd::noise_avx2(&mut f.rgba, w, f.h, seed, k) };
        // хвост столбцов (w % 8) — той же формулой, что скаляр ниже
        let vec_w = w & !7;
        if vec_w < w {
            for y in 0..f.h {
                for x in vec_w..w {
                    let i = ((y * w + x) * 4) as usize;
                    let h = noise_hash(x ^ noise_hash(y ^ seed));
                    let n = ((h >> 16) & 0x7fff) as i64 - 16384;
                    let off = (n * k / 32767) as i32;
                    for c in 0..3 {
                        f.rgba[i + c] = (f.rgba[i + c] as i32 + off).clamp(0, 255) as u8;
                    }
                }
            }
        }
        return f;
    }
    for (p, px) in f.rgba.chunks_exact_mut(4).enumerate() {
        let (x, y) = (p as u32 % w, p as u32 / w);
        let h = noise_hash(x ^ noise_hash(y ^ seed));
        let n = ((h >> 16) & 0x7fff) as i64 - 16384;
        let off = (n * k / 32767) as i32;
        for c in px.iter_mut().take(3) {
            *c = (*c as i32 + off).clamp(0, 255) as u8;
        }
    }
    f
}

/// VHS: RGB-сдвиг + скан-линии (v1: строки каждые 4, дрожание от времени).
#[allow(clippy::too_many_arguments)]
fn vhs(
    mut f: Frame,
    rgb_shift: u32,
    bleed: f64,
    scan_step: u32,
    jitter: f64,
    burst_index: i64,
    scratch: &mut FxScratch,
) -> Frame {
    let shift = rgb_shift.clamp(0, 12) as i64;
    let bleed = bleed.clamp(0.0, 1.0);
    let line_off = ((burst_index / 3000) % 7) as usize;
    let (w, h) = (f.w as usize, f.h as usize);
    // scan_step = 4 и jitter = 0 — прежнее поведение байт-в-байт (в том
    // числе AVX2-путь); всё остальное считается скаляром
    let step = scan_step.clamp(0, 32) as usize;
    let jitter = jitter.clamp(0.0, 1.0);
    let plain = step == 4 && jitter == 0.0;
    let jq = (jitter * 1024.0).round() as i64;
    let seed = noise_seed(burst_index);
    // читаются только пиксели ТЕКУЩЕЙ строки → строковый scratch вместо
    // полного clone (п.2 р.7); арифметика не изменена
    let mut row = scratch.take(w * 4);
    for y in 0..h {
        let scan =
            if step > 0 && (y + line_off).is_multiple_of(step) { 1i64 } else { 0 };
        // дрожание строки: сдвиг всей строки на долю ширины
        // ЦЕЛОЧИСЛЕННО: у GPU round() округляет половины к чётному, и на
        // дробном пути строка иногда уезжала на пиксель мимо CPU (t181).
        // Квантование параметра одно на оба пути.
        let jx = if jq == 0 {
            0i64
        } else {
            let hsh = noise_hash(y as u32 ^ noise_hash(seed));
            let n = ((hsh >> 16) & 0x7fff) as i64 - 16384;
            (n * jq / 32767) * w as i64 / 4096
        };
        let r0 = y * w * 4;
        row.copy_from_slice(&f.rgba[r0..r0 + w * 4]);
        // векторный интерьер строки; кромки и хвост — скалярным циклом
        let mut x_from = 0usize;
        #[cfg(target_arch = "x86_64")]
        if plain && crate::simd::simd_level() == crate::simd::SimdLevel::Avx2 {
            let d = (shift + scan) as usize;
            // SAFETY: уровень проверен; row длиной w*4, срез строки — тоже
            x_from = unsafe {
                crate::video_simd::vhs_row_avx2(
                    &mut f.rgba[r0..r0 + w * 4],
                    &row,
                    w,
                    d,
                    bleed,
                    scan == 1,
                    0,
                )
            };
        }
        // левая кромка (её пропустил вектор, начав с x = d)
        #[cfg(target_arch = "x86_64")]
        if x_from > 0 {
            for x in 0..(shift + scan) as usize {
                let i = r0 + x * 4;
                let xr = (x as i64 + shift + scan).clamp(0, w as i64 - 1) as usize;
                let xb = (x as i64 - shift - scan).clamp(0, w as i64 - 1) as usize;
                let r = row[xr * 4] as f64;
                let b = row[xb * 4 + 2] as f64;
                f.rgba[i] = (row[x * 4] as f64 * (1.0 - bleed) + r * bleed).round() as u8;
                f.rgba[i + 2] =
                    (row[x * 4 + 2] as f64 * (1.0 - bleed) + b * bleed).round() as u8;
                if scan == 1 {
                    f.rgba[i + 1] = row[x * 4 + 1].saturating_sub(18);
                }
            }
        }
        for x in x_from..w {
            let i = r0 + x * 4;
            // база строки уезжает на jx (0 — прежняя формула дословно)
            let xj = (x as i64 + jx).clamp(0, w as i64 - 1) as usize;
            let xr = (xj as i64 + shift + scan).clamp(0, w as i64 - 1) as usize;
            let xb = (xj as i64 - shift - scan).clamp(0, w as i64 - 1) as usize;
            let r = row[xr * 4] as f64;
            let b = row[xb * 4 + 2] as f64;
            f.rgba[i] = (row[xj * 4] as f64 * (1.0 - bleed) + r * bleed).round() as u8;
            f.rgba[i + 2] = (row[xj * 4 + 2] as f64 * (1.0 - bleed) + b * bleed).round() as u8;
            if scan == 1 {
                f.rgba[i + 1] = row[xj * 4 + 1].saturating_sub(18);
            } else if jx != 0 {
                // при дрожании едет ВСЯ строка, включая зелёный: без
                // этого он оставался бы от несдвинутого пикселя (артефакт
                // in-place, из-за него расходились CPU и GPU)
                f.rgba[i + 1] = row[xj * 4 + 1];
            }
        }
    }
    scratch.put(row);
    f
}

/// Bloom: даунскейл ×1/4 → апскейл, аддитивно ×strength×0.35 (v1).
fn glow(
    mut f: Frame,
    strength: f64,
    radius: u32,
    threshold: u32,
    tint: [u8; 3],
    scratch: &mut FxScratch,
) -> Frame {
    let s = strength.clamp(0.0, 2.0) * 0.35;
    if s <= 0.0 {
        return f;
    }
    // радиус 4, порог 0 и белый оттенок — прежний путь байт-в-байт
    let r = radius.clamp(1, 32);
    let th = threshold.min(255) as i32;
    let plain = th == 0 && tint == [255, 255, 255];
    let (dw, dh) = ((f.w / r).max(1), (f.h / r).max(1));
    let mut small = scratch.take((dw * dh * 4) as usize);
    bilinear_into(&f, 0.0, 0.0, f.w as f64, f.h as f64, dw, dh, &mut small);
    let small_f = Frame { w: dw, h: dh, rgba: small, has_alpha: false };
    let mut up = scratch.take(f.rgba.len());
    bilinear_into(&small_f, 0.0, 0.0, dw as f64, dh as f64, f.w, f.h, &mut up);
    // аддитив in-place: читается только сам пиксель + up (п.2 р.7)
    let pixels = f.rgba.len().min(up.len()) / 4;
    let mut done = 0usize;
    #[cfg(target_arch = "x86_64")]
    if plain && crate::simd::simd_level() == crate::simd::SimdLevel::Avx2 {
        // SAFETY: уровень проверен детектом; pixels не выходит за оба буфера
        unsafe { crate::video_simd::glow_add_avx2(&mut f.rgba, &up, s, pixels) };
        done = pixels;
    }
    for (o, u) in
        f.rgba.chunks_exact_mut(4).zip(up.chunks_exact(4)).skip(done)
    {
        // порог по яркости РАЗМЫТОГО кадра: мягкое колено шириной th
        let w = if th == 0 {
            1.0
        } else {
            let luma = (77 * u[0] as i32 + 150 * u[1] as i32 + 29 * u[2] as i32) >> 8;
            (((luma - th) as f64) / th as f64).clamp(0.0, 1.0)
        };
        for c in 0..3 {
            let k = s * w * tint[c] as f64 / 255.0;
            o[c] = (o[c] as f64 + u[c] as f64 * k).min(255.0) as u8;
        }
    }
    scratch.put(small_f.rgba);
    scratch.put(up);
    f
}

/// Мозаика cols×rows (v1 eventgrid): кадр тиражируется по ячейкам.
fn event_grid(
    src: &Frame,
    ctx: &VideoFxCtx,
    cols: u32,
    rows: u32,
    center: bool,
    scratch: &mut FxScratch,
) -> Frame {
    let cols = cols.clamp(1, 512);
    let rows = rows.clamp(1, 512);
    let (ow, oh) = (ctx.out_w.max(2), ctx.out_h.max(2));
    let (cw, chh) = ((ow / cols).max(1), (oh / rows).max(1));
    // одна ячейка
    let mut cell = scratch.take((cw * chh * 4) as usize);
    bilinear_into(src, 0.0, 0.0, src.w as f64, src.h as f64, cw, chh, &mut cell);
    let mut out = scratch.take((ow * oh * 4) as usize);
    for p in out.chunks_exact_mut(4) {
        p[3] = 255; // чёрный холст
    }
    let (grid_w, grid_h) = (cw * cols, chh * rows);
    let (offx, offy) = if center { ((ow - grid_w) / 2, (oh - grid_h) / 2) } else { (0, 0) };
    for ry in 0..rows {
        for rx in 0..cols {
            for y in 0..chh {
                let dy = offy + ry * chh + y;
                if dy >= oh {
                    continue;
                }
                let d0 = ((dy * ow + offx + rx * cw) * 4) as usize;
                let s0 = ((y * cw) * 4) as usize;
                let n = (cw * 4) as usize;
                if d0 + n <= out.len() {
                    out[d0..d0 + n].copy_from_slice(&cell[s0..s0 + n]);
                }
            }
        }
    }
    scratch.put(cell);
    Frame { w: ow, h: oh, rgba: out, has_alpha: false }
}
