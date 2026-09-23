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
    /// Разрешённая 3D-камера слоя (Р.44: Pan3d считается и на CPU).
    /// None — Pan3d в цепочке пропускается, как раньше без камеры.
    pub cam3d: Option<crate::geom::Cam3d>,
    /// Ступень декода кадра (Р.46, п.318): входной кадр в in_scale раз
    /// крупнее логического размера цепочки. 1 — как было всегда.
    pub in_scale: u32,
    /// Контекст ведущего узла «Геометрия» (Р.62): вид источника и правило
    /// приведения. `None` — узел считается «растянуть» (рамка = холст).
    pub lead: Option<crate::conform::LeadCtx>,
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

/// Встроенный образец для МИНИАТЮР эффектов (Р.54): что показать в
/// иконке, когда своего кадра ещё нет — пикер открыт на клипе-вложении,
/// тумба источника не приехала, или речь про иконку самого вида.
///
/// Свойства выбраны под то, что миниатюра обязана показать:
/// * АСИММЕТРИЯ по обеим осям (диагональ + круг в одном углу) — иначе
///   `Mirror` неотличим от входа, а его как раз и надо показать;
/// * зелёная плашка — `Greenscreen` должен чему-то кеиться;
/// * цветные полосы и яркостная рампа — `ColorGrade`/`Duotone`/
///   `Posterize`/`VHS` меняют именно их;
/// * мелкая сетка и жёсткие кромки — `Blur`/`Sharpen`/`Edges`/`Glow`/
///   `Pixelate`/`Halftone` без высоких частот показали бы пустое место.
///
/// `alpha_margin` — контент в рамке с ПРОЗРАЧНЫМИ полями: `DropShadow` на
/// непрозрачном кадре тождествен по построению (тень падает из-под
/// альфы), а `Pan3d`/`CornerPin` без полей упирают квад в кромку.
///
/// Детерминирован (ни времени, ни RNG) и заморожен голденом t544: это
/// ЭТАЛОН, а не украшение — по нему сверяют иконки.
pub fn test_pattern(w: u32, h: u32, alpha_margin: bool) -> Frame {
    let (w, h) = (w.max(4), h.max(4));
    let mut rgba = vec![0u8; (w as usize) * (h as usize) * 4];
    // 8 цветных полос сверху (жёсткие вертикальные кромки)
    const BARS: [[u8; 3]; 8] = [
        [235, 235, 235],
        [235, 235, 40],
        [40, 235, 235],
        [40, 200, 60],
        [235, 60, 200],
        [220, 50, 50],
        [50, 70, 220],
        [20, 20, 20],
    ];
    let grid = (w / 40).max(2);
    let (fw, fh) = (w as f64, h as f64);
    for y in 0..h {
        let v = (y as f64 + 0.5) / fh;
        for x in 0..w {
            let u = (x as f64 + 0.5) / fw;
            let i = ((y as usize) * (w as usize) + x as usize) * 4;
            let (mut r, mut g, mut b) = if v < 0.34 {
                let bar = ((u * BARS.len() as f64) as usize).min(BARS.len() - 1);
                (BARS[bar][0], BARS[bar][1], BARS[bar][2])
            } else if v < 0.67 {
                // яркостная рампа; в левой половине — мелкая шахматка
                let l = (u * 255.0) as u8;
                if u < 0.5 && ((x / grid) + (y / grid)) % 2 == 0 {
                    (l.saturating_add(60), l.saturating_add(60), l.saturating_add(60))
                } else {
                    (l, l, l)
                }
            } else if u > 0.62 {
                (0, 255, 0) // плашка под кеинг
            } else {
                (96, 96, 104)
            };
            // диагональ (асимметрия по обеим осям)
            if ((u - v).abs() * fw) < grid as f64 {
                (r, g, b) = (255, 255, 255);
            }
            // круг в ЛЕВОМ нижнем углу: зеркало по любой оси его сдвинет
            let (dx, dy) = (u - 0.22, v - 0.80);
            if dx * dx + dy * dy < 0.012 {
                (r, g, b) = (250, 170, 30);
            }
            let inside = !alpha_margin || (0.11..0.89).contains(&u) && (0.11..0.89).contains(&v);
            rgba[i] = if inside { r } else { 0 };
            rgba[i + 1] = if inside { g } else { 0 };
            rgba[i + 2] = if inside { b } else { 0 };
            rgba[i + 3] = if inside { 255 } else { 0 };
        }
    }
    Frame { w, h, rgba, has_alpha: alpha_margin }
}

/// Миниатюра цепочки на готовом кадре (Р.54): вписать `base` в w×h и
/// прогнать цепочку. ЕДИНСТВЕННЫЙ вход миниатюр — и у иконок вида, и у
/// полоски вариантов, и у пресетов библиотеки.
///
/// Семантика контекста — семантика движка, чтобы картинка не расходилась
/// с тем, что получится после клика: `u` — прогресс клипа в точке
/// курсора, `burst_index` — абсолютный тик таймлайна (зерно noise/vhs),
/// `cam3d` — разрешённая камера слоя (без неё Pan3d — passthrough).
/// `in_scale` здесь всегда 1: ступень декода (Р.46) говорит «кадр пришёл
/// КРУПНЕЕ логического размера», а у миниатюры наоборот.
///
/// О РАЗМЕРЕ. Часть параметров задана в ПИКСЕЛЯХ (`Pixelate.size` до 128,
/// `Halftone.size`, радиусы размытий, `Vhs.rgb_shift`), поэтому одна и та
/// же цепочка на 96×54 и на 1080p показывает разную СИЛУ. Миниатюра — про
/// вид, не про силу; звать её осмысленно на опорном размере (у нас это
/// размер тумбы, 320×180) и ужимать результат `fit_into`, а не считать
/// сразу в размере ячейки.
#[allow(clippy::too_many_arguments)] // это параметры VideoFxCtx, а не «много всего»
pub fn render_fx_thumb(
    base: &Frame,
    fx: &[FxInstance],
    w: u32,
    h: u32,
    u: f64,
    burst_index: i64,
    cam3d: Option<crate::geom::Cam3d>,
    scratch: &mut FxScratch,
) -> Frame {
    let (w, h) = (w.max(1), h.max(1));
    let src = if base.w == w && base.h == h {
        Frame { w, h, rgba: base.rgba.clone(), has_alpha: base.has_alpha }
    } else {
        let mut buf = scratch.take((w as usize) * (h as usize) * 4);
        fit_into(base, w, h, &mut buf);
        Frame { w, h, rgba: buf, has_alpha: base.has_alpha }
    };
    let ctx = VideoFxCtx { out_w: w, out_h: h, u, burst_index, cam3d, in_scale: 1, lead: None };
    apply_video_chain_pooled(fx, src, &ctx, scratch)
}

/// Применить цепочку видео-FX клипа. Аудио-варианты игнорируются.
/// Обёртка с локальным пулом — результат идентичен pooled-варианту.
pub fn apply_video_chain(fx: &[FxInstance], frame: Frame, ctx: &VideoFxCtx) -> Frame {
    let mut scratch = FxScratch::default();
    apply_video_chain_pooled(fx, frame, ctx, &mut scratch)
}

/// Вариант с переиспользуемым пулом буферов (превью зовёт цепочку на каждый
/// новый кадр — держатель пула живёт в VideoRenderer).
///
/// С Р.44 подряд идущие ГЕОМЕТРИЧЕСКИЕ эффекты (Pan/CamMotion/Mirror/
/// EventGrid) исполняются одним слитым проходом обратных отображений
/// (`crate::geom`, конкатенация трансформов) — без промежуточных растров.
/// Серия длины 1 идёт старыми ядрами байт-в-байт (t297); барьеры
/// (Blur/Glow/Edges/VHS/Noise/Greenscreen) — как раньше.
pub fn apply_video_chain_pooled(
    fx: &[FxInstance],
    mut frame: Frame,
    ctx: &VideoFxCtx,
    scratch: &mut FxScratch,
) -> Frame {
    // План строится ОБЩИМ кодом (crate::plan, п.311) — тем же, что зовёт
    // GPU-путь; здесь только исполнение опов.
    let plan = crate::plan::plan_chain_in(
        fx,
        frame.w,
        frame.h,
        ctx.in_scale,
        ctx.out_w,
        ctx.out_h,
        ctx.u,
        ctx.cam3d,
        ctx.burst_index,
        &[],
        ctx.lead,
    );
    for op in plan.ops {
        frame = match op {
            crate::plan::PlanOp::Run { i, j, exec } => match exec {
                crate::geom::GeomExec::Legacy => {
                    let step = fx[i..j]
                        .iter()
                        .find(|f| crate::geom::is_geo_active(f, ctx.lead, ctx.out_w, ctx.out_h))
                        .expect("legacy run has a geo step");
                    apply_legacy_geo(step, frame, ctx, scratch)
                }
                crate::geom::GeomExec::Fused(prog) => {
                    let nf = crate::geom::exec_geom_cpu(&prog, &frame, scratch);
                    scratch.put(frame.rgba);
                    nf
                }
            },
            crate::plan::PlanOp::Barrier { idx, res_scale } => {
                apply_barrier(&fx[idx], frame, ctx, scratch, res_scale)
            }
            // сила слоя применяется уровнем выше (apply_chain_with_mixes в
            // kt-engine); в плоской цепочке слот — passthrough, как раньше
            crate::plan::PlanOp::Snapshot | crate::plan::PlanOp::Mix { .. } => frame,
        };
    }
    frame
}

/// Один барьерный эффект (нелокальный/поточечный вне серий) — прежние
/// ядра; `res_scale` — K×-исполнение (Р.45 п.313: пиксельная геометрия
/// ядра ×K, вид инвариантен; K=1 — дословно прежний код). Pub — для
/// t303 (ядёрная инвариантность вида без цепочек); продуктовый вызов —
/// только исполнитель плана.
pub fn apply_barrier(
    f: &FxInstance,
    frame: Frame,
    ctx: &VideoFxCtx,
    scratch: &mut FxScratch,
    res_scale: u32,
) -> Frame {
    let rs = res_scale.max(1);
    match f {
        FxInstance::Blur { radius, radius_y } => blur(frame, *radius, *radius_y, scratch, rs),
        // greenscreen поточечный — разрешение его не касается
        FxInstance::Greenscreen { key_r, key_g, key_b, tolerance, softness, spill } => {
            greenscreen(frame, [*key_r, *key_g, *key_b], *tolerance, *softness, *spill)
        }
        FxInstance::Edges { threshold, thickness, color_r, color_g, color_b } => {
            edges(frame, *threshold, *thickness, [*color_r, *color_g, *color_b], scratch, rs)
        }
        FxInstance::Noise { amount, size, color } => {
            noise(frame, *amount, *size, *color, ctx.burst_index, rs)
        }
        FxInstance::Vhs { rgb_shift, bleed, scan_step, jitter } => {
            vhs(frame, *rgb_shift, *bleed, *scan_step, *jitter, ctx.burst_index, scratch, rs)
        }
        FxInstance::Glow { strength, radius, threshold, tint_r, tint_g, tint_b } => {
            glow(frame, *strength, *radius, *threshold, [*tint_r, *tint_g, *tint_b], scratch, rs)
        }
        // поточечные Р.47 в БАРЬЕРНОЙ позиции (ведущий/замыкающий слот):
        // тот же f64-текст формулы, что у тапа слитого прохода, — один дом
        FxInstance::ColorGrade { brightness, contrast, saturation, hue } => {
            let h = (*hue as f64).to_radians();
            let (hc, hs) = (h.cos(), h.sin());
            pointwise_barrier(frame, |v, _| {
                crate::geom::tap_grade(
                    v,
                    *brightness as f64,
                    *contrast as f64,
                    *saturation as f64,
                    hc,
                    hs,
                )
            })
        }
        FxInstance::Duotone { shadow_r, shadow_g, shadow_b, high_r, high_g, high_b, amount } => {
            let sh = [*shadow_r as f64, *shadow_g as f64, *shadow_b as f64];
            let hi = [*high_r as f64, *high_g as f64, *high_b as f64];
            let a = (*amount as f64).clamp(0.0, 1.0);
            pointwise_barrier(frame, |v, _| crate::geom::tap_duo(v, &sh, &hi, a))
        }
        FxInstance::Posterize { levels } => {
            let n = (*levels).clamp(2, 32);
            pointwise_barrier(frame, |v, _| crate::geom::tap_poster(v, n))
        }
        FxInstance::Vignette { strength, radius, softness } => {
            let (st, ra, so) = (
                (*strength as f64).clamp(0.0, 1.0),
                (*radius as f64).clamp(0.0, 1.5),
                (*softness as f64).clamp(0.05, 1.0),
            );
            let (w, h) = (frame.w, frame.h);
            pointwise_barrier(frame, move |v, xy| crate::geom::tap_vig(v, xy, st, ra, so, w, h))
        }
        // барьеры Р.47 Кв3
        FxInstance::Sharpen { amount, radius } => {
            sharpen(frame, (*amount as f64).clamp(0.0, 4.0), *radius, scratch, rs)
        }
        FxInstance::Pixelate { size } => pixelate(frame, *size, scratch, rs),
        FxInstance::ChromaticAberration { amount } => {
            chroma_ab(frame, (*amount as f64).clamp(0.0, 0.05), scratch)
        }
        FxInstance::RadialBlur { amount, center_x, center_y, spin } => radial_blur(
            frame,
            (*amount as f64).clamp(0.0, 1.0),
            *center_x as f64,
            *center_y as f64,
            *spin,
            scratch,
        ),
        FxInstance::DropShadow { offset_x, offset_y, radius, opacity, tint_r, tint_g, tint_b } => {
            drop_shadow(
                frame,
                (*offset_x, *offset_y),
                *radius,
                (*opacity as f64).clamp(0.0, 1.0),
                [*tint_r, *tint_g, *tint_b],
                scratch,
                rs,
            )
        }
        FxInstance::Halftone { size, angle } => halftone(frame, *size, *angle as f64, scratch, rs),
        _ => {
            debug_assert!(false, "barrier op on non-barrier fx");
            frame
        }
    }
}

/// Нерезкое маскирование (Р.47): src + (src − мип-база)·amount. Шаг мипа
/// = radius·K (radius ≥ 2: шаг 1 дал бы базу == источнику и нулевой
/// эффект — билинейка 1:1 точна). Альфа не трогается.
fn sharpen(f: Frame, amount: f64, radius: u32, scratch: &mut FxScratch, res_scale: u32) -> Frame {
    let step = radius.clamp(2, 16) * res_scale;
    let (dw, dh) = ((f.w / step).max(1), (f.h / step).max(1));
    let mut small = scratch.take((dw * dh * 4) as usize);
    bilinear_into(&f, 0.0, 0.0, f.w as f64, f.h as f64, dw, dh, &mut small);
    let small_f = Frame { w: dw, h: dh, rgba: small, has_alpha: f.has_alpha };
    let mut base = scratch.take(f.rgba.len());
    bilinear_into(&small_f, 0.0, 0.0, dw as f64, dh as f64, f.w, f.h, &mut base);
    scratch.put(small_f.rgba);
    let mut out = f;
    for (px, bp) in out.rgba.chunks_exact_mut(4).zip(base.chunks_exact(4)) {
        for c in 0..3 {
            let s = px[c] as f64;
            px[c] = (s + (s - bp[c] as f64) * amount).round().clamp(0.0, 255.0) as u8;
        }
    }
    scratch.put(base);
    out
}

/// Мозаика (Р.47): целочисленная копия центра ячейки C = size·K —
/// EXACT-паритет с GPU по построению.
fn pixelate(f: Frame, size: u32, scratch: &mut FxScratch, res_scale: u32) -> Frame {
    let c = size.clamp(2, 128) * res_scale;
    let mut out = scratch.take(f.rgba.len());
    for y in 0..f.h {
        let sy = (y / c * c + c / 2).min(f.h - 1);
        for x in 0..f.w {
            let sx = (x / c * c + c / 2).min(f.w - 1);
            let d = ((y * f.w + x) * 4) as usize;
            let s = ((sy * f.w + sx) * 4) as usize;
            out[d..d + 4].copy_from_slice(&f.rgba[s..s + 4]);
        }
    }
    let res = Frame { w: f.w, h: f.h, rgba: out, has_alpha: f.has_alpha };
    scratch.put(f.rgba);
    res
}

/// Хроматическая аберрация (Р.47): R сэмплится ближе к центру (×(1−a)),
/// B — дальше (×(1+a)), G и альфа на месте. Позиции — доли кадра,
/// K×-исполнение масштабируется само.
fn chroma_ab(f: Frame, amount: f64, scratch: &mut FxScratch) -> Frame {
    let (cw, ch) = (f.w as f64 / 2.0, f.h as f64 / 2.0);
    let mut out = scratch.take(f.rgba.len());
    for y in 0..f.h {
        for x in 0..f.w {
            let d = ((y * f.w + x) * 4) as usize;
            let (px, py) = (x as f64 + 0.5 - cw, y as f64 + 0.5 - ch);
            let r = crate::geom::sample_bilinear_f64(
                &f,
                cw + px * (1.0 - amount) - 0.5,
                ch + py * (1.0 - amount) - 0.5,
            )[0];
            let b = crate::geom::sample_bilinear_f64(
                &f,
                cw + px * (1.0 + amount) - 0.5,
                ch + py * (1.0 + amount) - 0.5,
            )[2];
            out[d] = r.round().clamp(0.0, 255.0) as u8;
            out[d + 1] = f.rgba[d + 1];
            out[d + 2] = b.round().clamp(0.0, 255.0) as u8;
            out[d + 3] = f.rgba[d + 3];
        }
    }
    let res = Frame { w: f.w, h: f.h, rgba: out, has_alpha: f.has_alpha };
    scratch.put(f.rgba);
    res
}

/// Радиальное размытие (Р.47): среднее РОВНО 16 тапов вдоль зум-луча или
/// дуги вращения — число тапов константа обоих бэкендов, доли кадра
/// масштабируют K× сами.
fn radial_blur(
    f: Frame,
    amount: f64,
    center_x: f64,
    center_y: f64,
    spin: bool,
    scratch: &mut FxScratch,
) -> Frame {
    const N: usize = 16;
    let (cx, cy) = (
        (0.5 + center_x.clamp(-1.0, 1.0) * 0.5) * f.w as f64,
        (0.5 + center_y.clamp(-1.0, 1.0) * 0.5) * f.h as f64,
    );
    let mut out = scratch.take(f.rgba.len());
    for y in 0..f.h {
        for x in 0..f.w {
            let d = ((y * f.w + x) * 4) as usize;
            let (px, py) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
            let mut acc = [0f64; 4];
            for i in 0..N {
                let t = i as f64 / (N - 1) as f64;
                let (sx, sy) = if spin {
                    let a = 0.6 * amount * (t - 0.5);
                    let (sa, ca) = a.sin_cos();
                    (px * ca - py * sa, px * sa + py * ca)
                } else {
                    let k = 1.0 - 0.5 * amount * t;
                    (px * k, py * k)
                };
                let s = crate::geom::sample_bilinear_f64(&f, cx + sx - 0.5, cy + sy - 0.5);
                for (a, v) in acc.iter_mut().zip(s) {
                    *a += v;
                }
            }
            for (c, a) in acc.iter().enumerate() {
                out[d + c] = (a / N as f64).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    let res = Frame { w: f.w, h: f.h, rgba: out, has_alpha: f.has_alpha };
    scratch.put(f.rgba);
    res
}

/// Тень под слоем (Р.47): размытая (мип radius·K) альфа источника со
/// сдвигом offset·K, источник ПОВЕРХ тени (straight alpha, f64,
/// округление один раз). Вход без альфы — passthrough: формула на
/// непрозрачном входе вырождается в тождество, ранний выход лишь
/// экономит работу (GPU-сторона согласована — см. chain_ops).
fn drop_shadow(
    f: Frame,
    offset: (i32, i32),
    radius: u32,
    opacity: f64,
    tint: [u8; 3],
    scratch: &mut FxScratch,
    res_scale: u32,
) -> Frame {
    if !f.has_alpha {
        return f;
    }
    let step = radius.clamp(1, 32) * res_scale;
    let (dw, dh) = ((f.w / step).max(1), (f.h / step).max(1));
    let mut small = scratch.take((dw * dh * 4) as usize);
    bilinear_into(&f, 0.0, 0.0, f.w as f64, f.h as f64, dw, dh, &mut small);
    let small_f = Frame { w: dw, h: dh, rgba: small, has_alpha: true };
    let mut blurred = scratch.take(f.rgba.len());
    bilinear_into(&small_f, 0.0, 0.0, dw as f64, dh as f64, f.w, f.h, &mut blurred);
    scratch.put(small_f.rgba);
    let (ox, oy) =
        (offset.0.clamp(-64, 64) * res_scale as i32, offset.1.clamp(-64, 64) * res_scale as i32);
    let mut out = scratch.take(f.rgba.len());
    for y in 0..f.h as i64 {
        for x in 0..f.w as i64 {
            let d = ((y * f.w as i64 + x) * 4) as usize;
            let (bx, by) = (x - ox as i64, y - oy as i64);
            let ba = if bx >= 0 && by >= 0 && bx < f.w as i64 && by < f.h as i64 {
                blurred[((by * f.w as i64 + bx) * 4 + 3) as usize] as f64 / 255.0
            } else {
                0.0
            };
            let ash = ba * opacity;
            let asrc = f.rgba[d + 3] as f64 / 255.0;
            let oa = asrc + ash * (1.0 - asrc);
            if oa <= 0.0 {
                out[d..d + 4].copy_from_slice(&[0, 0, 0, 0]);
            } else {
                for c in 0..3 {
                    let v =
                        (f.rgba[d + c] as f64 * asrc + tint[c] as f64 * ash * (1.0 - asrc)) / oa;
                    out[d + c] = v.round().clamp(0.0, 255.0) as u8;
                }
                out[d + 3] = (oa * 255.0).round().clamp(0.0, 255.0) as u8;
            }
        }
    }
    scratch.put(blurred);
    let res = Frame { w: f.w, h: f.h, rgba: out, has_alpha: true };
    scratch.put(f.rgba);
    res
}

/// Полутоновый растр (Р.47): чёрные точки на белом, радиус от люмы
/// центра ячейки. Узор целиком в ЛОГИЧЕСКИХ координатах l = p/K —
/// K×-исполнение видит ту же сетку.
fn halftone(f: Frame, size: u32, angle: f64, scratch: &mut FxScratch, res_scale: u32) -> Frame {
    let s = size.clamp(2, 64) as f64;
    let k = res_scale as f64;
    let (sa, ca) = angle.clamp(-90.0, 90.0).to_radians().sin_cos();
    let mut out = scratch.take(f.rgba.len());
    for y in 0..f.h {
        for x in 0..f.w {
            let d = ((y * f.w + x) * 4) as usize;
            let (lx, ly) = ((x as f64 + 0.5) / k, (y as f64 + 0.5) / k);
            // поворот на −angle: сетка стоит в повёрнутом пространстве
            let (ux, uy) = (lx * ca + ly * sa, -lx * sa + ly * ca);
            let (ccx, ccy) = ((ux / s).floor() * s + 0.5 * s, (uy / s).floor() * s + 0.5 * s);
            // центр ячейки назад в логику и в физику кадра
            let (gx, gy) = (ccx * ca - ccy * sa, ccx * sa + ccy * ca);
            let smp = crate::geom::sample_bilinear_f64(&f, gx * k - 0.5, gy * k - 0.5);
            let l = (77.0 * smp[0] + 150.0 * smp[1] + 29.0 * smp[2]) / 256.0;
            let rd = 0.5 * s * 1.25 * (1.0 - (l / 255.0).clamp(0.0, 1.0)).sqrt();
            let dist = ((ux - ccx) * (ux - ccx) + (uy - ccy) * (uy - ccy)).sqrt();
            let v = if dist <= rd { 0u8 } else { 255u8 };
            out[d] = v;
            out[d + 1] = v;
            out[d + 2] = v;
            out[d + 3] = f.rgba[d + 3];
        }
    }
    let res = Frame { w: f.w, h: f.h, rgba: out, has_alpha: f.has_alpha };
    scratch.put(f.rgba);
    res
}

/// Поточечный эффект как барьер: f64-континуум формулы тапа на каждом
/// пикселе, clamp+round ОДИН раз на выходе (та же дисциплина, что у
/// exec_geom_cpu).
fn pointwise_barrier(mut f: Frame, op: impl Fn(&mut [f64; 4], (f64, f64))) -> Frame {
    let w = f.w as usize;
    for (i, px) in f.rgba.chunks_exact_mut(4).enumerate() {
        let (x, y) = ((i % w) as f64, (i / w) as f64);
        let mut v = [px[0] as f64, px[1] as f64, px[2] as f64, px[3] as f64];
        op(&mut v, (x, y));
        for (c, out) in v.iter().zip(px.iter_mut()) {
            *out = c.round().clamp(0.0, 255.0) as u8;
        }
    }
    f
}

/// Одиночный гео-эффект СТАРЫМИ ядрами (`GeomExec::Legacy`): байты как до
/// Р.44 — на этом стоят голдены pan/cammotion/mirror t26 и правило «серия
/// длины 1 — прежний проход». Зовётся только из `geom::apply_geo_run`.
pub(crate) fn apply_legacy_geo(
    f: &FxInstance,
    frame: Frame,
    ctx: &VideoFxCtx,
    scratch: &mut FxScratch,
) -> Frame {
    match f {
        FxInstance::CamMotion { mode, ease, time_scale, keyframes } => {
            let (px, py, z) = cam_at(keyframes, ctx.u, *ease, *time_scale);
            let nf = pan_zoom(&frame, ctx, *mode, px, py, z, scratch);
            scratch.put(frame.rgba);
            nf
        }
        FxInstance::Mirror { horizontal, vertical } => mirror(frame, *horizontal, *vertical),
        _ => {
            debug_assert!(false, "apply_legacy_geo: not a legacy geo fx");
            frame
        }
    }
}

/// Есть ли в цепочке видео-эффекты (иначе композит идёт без копий).
pub fn has_video_fx(fx: &[FxInstance]) -> bool {
    has_video_fx_in(fx, None, 0, 0)
}

/// То же с контекстом ведущего узла «Геометрия» (Р.62): ведущий узел с
/// тождественными параметрами — «не эффект» ровно тогда, когда рамка
/// приведения равна холсту; иначе он обязан положить кадр в рамку.
pub fn has_video_fx_in(
    fx: &[FxInstance],
    lead: Option<crate::conform::LeadCtx>,
    out_w: u32,
    out_h: u32,
) -> bool {
    // сила слоя (Р.38 п.12) — не пиксельный фильтр, а правило смешения
    // входа цепочки с выходом; её применяет kt-engine ПОСЛЕ цепочки.
    // Цепочка из одной только силы пикселей не трогает вовсе.
    fx.iter().any(|f| {
        !f.is_audio()
            && !f.is_layer_mix()
            && !crate::geom::geometry_is_identity_in(f, lead, out_w, out_h)
    })
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
/// Премультиплицировать НА МЕСТЕ: `rgb' = rgb·a/255` с округлением к
/// ближайшему (Р.53, п.418).
///
/// Зачем: мип и любая свёртка смешивают СОСЕДНИЕ пиксели, а у прозрачного
/// пикселя цвет — мусор (чаще всего чёрный). На прямой альфе этот мусор
/// затекает в видимые пиксели, и у титра появляется тёмный ореол по
/// краю. Премультипликация делает вклад прозрачного пикселя нулевым по
/// построению.
///
/// Тождество при `a == 255`: на непрозрачном кадре премультипликация НЕ
/// МЕНЯЕТ НИ БАЙТА — на этом стоят все голдены, и поэтому шаг гейтится
/// `has_alpha`, а не включается всегда.
pub fn premultiply(rgba: &mut [u8]) {
    for px in rgba.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a == 255 {
            continue;
        }
        for c in px[..3].iter_mut() {
            *c = ((*c as u32 * a + 127) / 255) as u8;
        }
    }
}

/// Прямая альфа из премультиплицированной: `rgb' = rgb·255/a`.
///
/// Тождество при `a == 255` (непрозрачный пиксель не трогается вовсе) и
/// ноль при `a == 0` (цвета там нет). Целочисленно и точно зеркалится в
/// WGSL — все текстуры kt-gpu `Rgba8Uint`. Дом функции — ЗДЕСЬ: её зовут
/// и граница вложения (kt-engine), и premul-гейт ядер (п.418), а два
/// экземпляра одного правила разошлись бы на округлении.
pub fn unpremultiply(rgba: &mut [u8]) {
    for px in rgba.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a == 255 {
            continue;
        }
        if a == 0 {
            px[0] = 0;
            px[1] = 0;
            px[2] = 0;
            continue;
        }
        for c in px[..3].iter_mut() {
            *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
        }
    }
}

fn blur(
    mut f: Frame,
    radius: u32,
    radius_y: u32,
    scratch: &mut FxScratch,
    res_scale: u32,
) -> Frame {
    let radius = radius.clamp(1, 48);
    // Р.45 (п.313): K×-исполнение масштабирует ШАГ — floor(K·w/(K·step))
    // == floor(w/step), мип тот же, вид инвариантен точно; K=1 — ×1
    let step = ((radius / 3).max(1) + 1) * res_scale;
    let step_y =
        if radius_y == 0 { step } else { ((radius_y.clamp(1, 48) / 3).max(1) + 1) * res_scale };
    let (dw, dh) = ((f.w / step).max(1), (f.h / step_y).max(1));
    // ПРАВИЛО АЛЬФЫ (п.418): размывается премультиплицированное, иначе в
    // видимые пиксели затекает цвет прозрачных — тёмный ореол по краю
    // титра. На непрозрачном кадре обе конверсии тождественны, поэтому
    // гейт по `has_alpha` и оставляет все голдены нетронутыми.
    let pm = f.has_alpha;
    if pm {
        premultiply(&mut f.rgba);
    }
    let mut small = scratch.take((dw * dh * 4) as usize);
    bilinear_into(&f, 0.0, 0.0, f.w as f64, f.h as f64, dw, dh, &mut small);
    let small_f = Frame { w: dw, h: dh, rgba: small, has_alpha: f.has_alpha };
    let mut out = scratch.take(f.rgba.len());
    bilinear_into(&small_f, 0.0, 0.0, dw as f64, dh as f64, f.w, f.h, &mut out);
    if pm {
        unpremultiply(&mut out);
    }
    scratch.put(small_f.rgba);
    scratch.put(f.rgba);
    Frame { w: f.w, h: f.h, rgba: out, has_alpha: f.has_alpha }
}

fn greenscreen(mut f: Frame, key: [u8; 3], tolerance: u32, softness: u32, spill: f64) -> Frame {
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
    res_scale: u32,
) -> Frame {
    let th = threshold.clamp(1, 255) as i32;
    // толщина = шаг выборки Собеля; 1 и белый цвет — прежнее ядро 3×3
    // байт-в-байт (в том числе AVX2-путь)
    // Р.45 (п.313): страйд Собеля и кайма ×K; plain-гейт (d == 1)
    // при K > 1 гаснет сам — эффективный страйд ≥ 2
    let d = thickness.clamp(1, 8) as i64 * res_scale as i64;
    let plain = d == 1 && color == [255, 255, 255];
    let (w, h) = (f.w as usize, f.h as usize);
    #[cfg(target_arch = "x86_64")]
    let simd =
        crate::simd::simd_level() == crate::simd::SimdLevel::Avx2 && f.rgba.len() >= w * h * 4;
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
    // ПРАВИЛО АЛЬФЫ (Р.53, п.418): edges больше не заливает альфу 255, а
    // ПОДНИМАЕТ её до силы линии — `a_out = max(a_in, v)`. На непрозрачном
    // входе (a_in = 255) это тождество, и байты прежние; на входе с
    // альфой обводка перестала быть непрозрачной плитой во весь кадр и
    // стала тем, чем её и просят быть — накладкой контуров.
    let keep_alpha = f.has_alpha;
    let mut out = scratch.take(f.rgba.len());
    for (o, i) in out.chunks_exact_mut(4).zip(f.rgba.chunks_exact(4)) {
        o[3] = if keep_alpha { i[3] } else { 255 };
    }
    let m = d as usize; // кайма шире при толстой линии
    for y in m..h.saturating_sub(m) {
        // векторная часть строки; скалярный цикл ниже дорисовывает хвост
        let mut x_from = m;
        #[cfg(target_arch = "x86_64")]
        // AVX2-ядро пишет альфу 255 жёстко: на кадре С АЛЬФОЙ считаем
        // скаляром (там и так не горячий путь — титры, а не монтаж 4K)
        if plain && simd && w > 2 && !keep_alpha {
            // SAFETY: уровень проверен; строки y-1..y+1 существуют, out полон
            x_from = unsafe { crate::video_simd::sobel_row_avx2(&mut out, &luma, w, y, th, w - 1) };
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
            if keep_alpha {
                out[i + 3] = out[i + 3].max(v);
            }
        }
    }
    scratch.put(f.rgba);
    Frame { w: f.w, h: f.h, rgba: out, has_alpha: keep_alpha }
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
fn noise(
    mut f: Frame,
    amount: f64,
    size: u32,
    color: bool,
    burst_index: i64,
    res_scale: u32,
) -> Frame {
    let k = (amount.clamp(0.0, 1.0) * 32.0) as i64;
    if k == 0 {
        return f;
    }
    let seed = noise_seed(burst_index);
    let w = f.w;
    let size = size.clamp(1, 64);
    // Р.45 (п.313): ячейка ×K — floor((K·p+i)/(K·size)) == floor(p/size),
    // узор зерна ПОБИТНО совпадает с 1×. Ветка формулы (особая при
    // size==1 && !color) выбирается по ПОЛЬЗОВАТЕЛЬСКОМУ size — у формул
    // разное зерно, и K не смеет молча подменять одну другой.
    let cell = size * res_scale;
    if size > 1 || color {
        let off_at = |x: u32, y: u32, ch: u32| -> i32 {
            let h = noise_hash(x ^ noise_hash(y ^ noise_hash(ch ^ seed)));
            let n = ((h >> 16) & 0x7fff) as i64 - 16384;
            (n * k / 32767) as i32
        };
        for (p, px) in f.rgba.chunks_exact_mut(4).enumerate() {
            let (x, y) = (p as u32 % w / cell, p as u32 / w / cell);
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
    if res_scale > 1 {
        // особая формула на K×-растре: координаты делятся на K — узор
        // зерна 1× воспроизводится поблочно (AVX2-путь остаётся за K=1)
        for (p, px) in f.rgba.chunks_exact_mut(4).enumerate() {
            let (x, y) = (p as u32 % w / res_scale, p as u32 / w / res_scale);
            let h = noise_hash(x ^ noise_hash(y ^ seed));
            let n = ((h >> 16) & 0x7fff) as i64 - 16384;
            let off = (n * k / 32767) as i32;
            for c in px.iter_mut().take(3) {
                *c = (*c as i32 + off).clamp(0, 255) as u8;
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
    res_scale: u32,
) -> Frame {
    // Р.45 (п.313): сдвиг ×K, весь СТРОЧНЫЙ контекст (скан-линии и хэш
    // дрожания) — от логической строки y/K: скан-условие само красит K
    // строк блока, line_off и scan_step остаются ЛОГИЧЕСКИМИ; амплитуда
    // дрожания jx масштабируется сама (она в долях ширины). K=1 — y/1.
    let rs = res_scale as usize;
    let shift = rgb_shift.clamp(0, 12) as i64 * res_scale as i64;
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
        let scan = if step > 0 && (y / rs + line_off).is_multiple_of(step) { 1i64 } else { 0 };
        // дрожание строки: сдвиг всей строки на долю ширины
        // ЦЕЛОЧИСЛЕННО: у GPU round() округляет половины к чётному, и на
        // дробном пути строка иногда уезжала на пиксель мимо CPU (t181).
        // Квантование параметра одно на оба пути.
        let jx = if jq == 0 {
            0i64
        } else {
            let hsh = noise_hash((y / rs) as u32 ^ noise_hash(seed));
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
                f.rgba[i + 2] = (row[x * 4 + 2] as f64 * (1.0 - bleed) + b * bleed).round() as u8;
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
    res_scale: u32,
) -> Frame {
    let s = strength.clamp(0.0, 2.0) * 0.35;
    if s <= 0.0 {
        return f;
    }
    // радиус 4, порог 0 и белый оттенок — прежний путь байт-в-байт
    // Р.45 (п.313): делитель мипа ×K — мип тот же, вид инвариантен
    let r = radius.clamp(1, 32) * res_scale;
    let th = threshold.min(255) as i32;
    let plain = th == 0 && tint == [255, 255, 255];
    let (dw, dh) = ((f.w / r).max(1), (f.h / r).max(1));
    // ПРАВИЛО АЛЬФЫ (п.418): свет из прозрачных мест не идёт. Мипим
    // КОПИЮ премультиплицированного кадра — и добавляем её как есть,
    // без обратной конверсии: премультиплицированный RGB и есть вклад
    // света. База сложения остаётся прямой. На непрозрачном кадре копия
    // побайтово равна оригиналу — прежний путь цел.
    let pm = f.has_alpha;
    let mut pmbuf = None;
    if pm {
        let mut b = scratch.take(f.rgba.len());
        b.copy_from_slice(&f.rgba);
        premultiply(&mut b);
        pmbuf = Some(Frame { w: f.w, h: f.h, rgba: b, has_alpha: true });
    }
    let mip_src = pmbuf.as_ref().unwrap_or(&f);
    let mut small = scratch.take((dw * dh * 4) as usize);
    bilinear_into(mip_src, 0.0, 0.0, f.w as f64, f.h as f64, dw, dh, &mut small);
    if let Some(fr) = pmbuf {
        scratch.put(fr.rgba);
    }
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
    for (o, u) in f.rgba.chunks_exact_mut(4).zip(up.chunks_exact(4)).skip(done) {
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

// Мозаика EventGrid с Р.44 считается слитым гео-проходом (`crate::geom`,
// Fold-примитив): тайл-локальная фаза воспроизводит прежнее ядро дословно —
// голден t26 «eventgrid» не изменился. Гард «сетка крупнее кадра» (п.306)
// живёт в builder'е программы.
