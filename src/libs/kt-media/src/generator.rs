//! Генератор медиа (Р.52, F-TXT-003): третий вид источника без файла —
//! после `SyntheticSource` (тесты) и `PlaceholderSource` (пропавший
//! файл). Картинку считает сам, параметры зашиты при создании: они —
//! функция МЕДИА (`gen:`-путь, словарь — `kt_model::GeneratorKind`),
//! правка параметров = пересоздание источника (`Player::replace_media`).
//!
//! Контракты, на которых всё стоит:
//! - кадр РОВНО запрошенного Quality — композит молча выбрасывает слой
//!   чужого размера (kt-engine/video.rs, `fits`);
//! - кадр — детерминированная функция (параметры, pts, Quality): ключи
//!   кэшей движка и GPU-аплоада не переживут ничего другого (инвариант 11);
//! - «нет звука» = `has_audio: false` И `Ok(AudioBurst::silence)` —
//!   `Err` копится в negative-кэше воркера ложными underrun'ами.

use crate::{AudioBurst, MediaError, MediaInfo, MediaSource, Quality, VideoFrame};
use kt_model::GeneratorKind;
use kt_time::Tick;

/// «Запас деталей» векторного источника: генератор рисует резко в любой
/// размер, и его натив — это разрешение на ступень декода под зумящей
/// FX-цепочкой (`clip_decode_scale`: min(запрос цепочки, натив/Quality,
/// потолок буферов)). Число равно `kt_fx::plan::MAX_INTER_DIM` — выше
/// промежуточных буферов всё равно не бывает; kt-media от kt-fx не
/// зависит, поэтому константа своя, с этим комментарием-привязкой.
/// В «Max (по источникам)» меню рендера псевдо-пути не участвуют.
pub const GEN_NATIVE: u32 = 4096;

/// Размер кадра для `quality = None` («полное разрешение источника» —
/// у генератора его не существует). Продукт всегда шлёт `Some`; ветка
/// нужна только чтобы контракт трейта не имел дыры.
const DEFAULT_Q: Quality = Quality { width: 1920, height: 1080 };

/// Источник-генератор: кадр считается из `GeneratorKind`.
pub struct GeneratorSource {
    kind: GeneratorKind,
    info: MediaInfo,
    /// Кадр видов, не зависящих от времени, строится один раз на
    /// запрошенное качество (паттерн `PlaceholderSource`).
    cache: Option<(Quality, Box<[u8]>)>,
}

impl GeneratorSource {
    pub fn new(kind: GeneratorKind) -> GeneratorSource {
        let info = Self::info_for(&kind);
        GeneratorSource { kind, info, cache: None }
    }

    /// Паспорт источника для вида — ЕДИНСТВЕННОЕ место, где он считается
    /// (Р.53, п.410). По нему же фабрика решает, можно ли перенастроить
    /// живой источник: `retune` обязан не менять `info()`.
    pub fn info_for(kind: &GeneratorKind) -> MediaInfo {
        // единственный звучащий вид — SMPTE с включённым тоном
        let has_audio = matches!(
            kind,
            GeneratorKind::SmpteBars { tone: true, .. } | GeneratorKind::Leader { beep: true, .. }
        );
        MediaInfo {
            // «длительности нет»: UI при броске подставит свой дефолт
            // (5 с), трим не ограничен, EOF не бывает.
            duration: Tick(0),
            has_video: true,
            // Тишину источник всё равно отдаёт (см. заголовок), а
            // аудио-КЛИП place_media_at создаёт только по этому флагу.
            has_audio,
            // своей сетки кадров нет: движок планирует по сетке ПРОЕКТА
            fps: None,
            width: GEN_NATIVE,
            height: GEN_NATIVE,
        }
    }

    pub fn kind(&self) -> &GeneratorKind {
        &self.kind
    }

    /// Буфер кадра ДЛЯ ВЫДАЧИ. Живые виды (шум, тайм-код) рисуются сразу
    /// в возвращаемый буфер — кэш по Quality им не годится; статические
    /// рисуются один раз на качество и копируются (Р.53, п.413: копий на
    /// запрос было ДВЕ — в кэш и из кэша, стало одна).
    fn frame_bytes(&mut self, pts: Tick, q: Quality) -> Box<[u8]> {
        if depends_on_time(&self.kind) {
            return render_at(&self.kind, pts, q.width, q.height);
        }
        if self.cache.as_ref().is_none_or(|(cq, _)| *cq != q) {
            self.cache = Some((q, render_at(&self.kind, Tick(0), q.width, q.height)));
        }
        self.cache.as_ref().expect("frame was just built").1.clone()
    }
}

fn depends_on_time(kind: &GeneratorKind) -> bool {
    matches!(
        kind,
        GeneratorKind::NoiseField { .. }
            | GeneratorKind::Timecode { .. }
            | GeneratorKind::Leader { .. }
    )
}

/// Честная ли у вида альфа — ИСЧЕРПЫВАЮЩИЙ match (Р.53, п.419): новый
/// вид не соберётся, пока про его альфу не сказано. До этого правило
/// было строкой `matches!(…Text…)`, мимо которой новый прозрачный вид
/// пролез бы молча и лёг в композит чёрной плитой.
pub fn kind_has_alpha(kind: &GeneratorKind) -> bool {
    match kind {
        GeneratorKind::Text { .. }
        | GeneratorKind::Transparent
        | GeneratorKind::Rect { .. }
        | GeneratorKind::Ellipse { .. }
        | GeneratorKind::Line { .. } => true,
        GeneratorKind::Solid { .. }
        | GeneratorKind::Gradient { .. }
        | GeneratorKind::Corners { .. }
        | GeneratorKind::Checker { .. }
        | GeneratorKind::NoiseField { .. }
        | GeneratorKind::SmpteBars { .. }
        | GeneratorKind::Timecode { .. }
        | GeneratorKind::Leader { .. }
        | GeneratorKind::TestPattern { .. }
        | GeneratorKind::WindowPattern { .. } => false,
    }
}

/// Кадр вида в точности `w`×`h`; `pts` — тик МАТЕРИАЛА (важен только
/// живым видам: шуму и тайм-коду — сдвиг клипа их не меняет).
fn render_at(kind: &GeneratorKind, pts: Tick, w: u32, h: u32) -> Box<[u8]> {
    let n = (w as usize) * (h as usize);
    match kind {
        GeneratorKind::Solid { color_r, color_g, color_b } => {
            let mut buf = vec![0u8; n * 4];
            for px in buf.chunks_exact_mut(4) {
                px.copy_from_slice(&[*color_r, *color_g, *color_b, 255]);
            }
            buf.into_boxed_slice()
        }
        GeneratorKind::Text { .. } => kt_text::render_rgba(&text_params(kind), w, h),
        GeneratorKind::Gradient { angle_deg, a_r, a_g, a_b, b_r, b_g, b_b } => {
            // проекция нормированных координат на направление угла,
            // растянутая на весь кадр: при любом угле оба цвета видны
            let rad = angle_deg.to_radians();
            let (dx, dy) = (rad.cos(), rad.sin());
            let corners = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
                .map(|(x, y): (f64, f64)| x * dx + y * dy);
            let lo = corners.iter().cloned().fold(f64::INFINITY, f64::min);
            let hi = corners.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let span = (hi - lo).max(1e-9);
            let mut buf = vec![0u8; n * 4];
            for y in 0..h as usize {
                let ny = (y as f64 + 0.5) / h as f64;
                for x in 0..w as usize {
                    let nx = (x as f64 + 0.5) / w as f64;
                    let t = ((nx * dx + ny * dy - lo) / span).clamp(0.0, 1.0);
                    let mix = |a: u8, b: u8| {
                        (a as f64 + (b as f64 - a as f64) * t).round().clamp(0.0, 255.0) as u8
                    };
                    let o = (y * w as usize + x) * 4;
                    buf[o] = mix(*a_r, *b_r);
                    buf[o + 1] = mix(*a_g, *b_g);
                    buf[o + 2] = mix(*a_b, *b_b);
                    buf[o + 3] = 255;
                }
            }
            buf.into_boxed_slice()
        }
        GeneratorKind::Corners {
            tl_r,
            tl_g,
            tl_b,
            tr_r,
            tr_g,
            tr_b,
            bl_r,
            bl_g,
            bl_b,
            br_r,
            br_g,
            br_b,
        } => {
            let mut buf = vec![0u8; n * 4];
            let bilerp = |tl: u8, tr: u8, bl: u8, br: u8, fx: f64, fy: f64| {
                let top = tl as f64 + (tr as f64 - tl as f64) * fx;
                let bot = bl as f64 + (br as f64 - bl as f64) * fx;
                (top + (bot - top) * fy).round().clamp(0.0, 255.0) as u8
            };
            for y in 0..h as usize {
                let fy = (y as f64 + 0.5) / h as f64;
                for x in 0..w as usize {
                    let fx = (x as f64 + 0.5) / w as f64;
                    let o = (y * w as usize + x) * 4;
                    buf[o] = bilerp(*tl_r, *tr_r, *bl_r, *br_r, fx, fy);
                    buf[o + 1] = bilerp(*tl_g, *tr_g, *bl_g, *br_g, fx, fy);
                    buf[o + 2] = bilerp(*tl_b, *tr_b, *bl_b, *br_b, fx, fy);
                    buf[o + 3] = 255;
                }
            }
            buf.into_boxed_slice()
        }
        GeneratorKind::Checker { cols, a_r, a_g, a_b, b_r, b_g, b_b } => {
            // клетка задаётся СЧЁТОМ колонок — картинка не зависит от
            // Quality (и от ступени декода): клетки квадратные по ширине
            let cell = (w as f64 / (*cols).max(1) as f64).max(1.0);
            let a = [*a_r, *a_g, *a_b, 255];
            let b = [*b_r, *b_g, *b_b, 255];
            let mut buf = vec![0u8; n * 4];
            for y in 0..h as usize {
                let cy = (y as f64 / cell) as u64;
                for x in 0..w as usize {
                    let cx = (x as f64 / cell) as u64;
                    let o = (y * w as usize + x) * 4;
                    buf[o..o + 4].copy_from_slice(if (cx + cy).is_multiple_of(2) {
                        &a
                    } else {
                        &b
                    });
                }
            }
            buf.into_boxed_slice()
        }
        GeneratorKind::NoiseField { amount, mono } => {
            // counter-based (та же lowbias32, что у Noise-эффекта в
            // kt-fx): зерно = f(клетка БАЗОВОЙ сетки 1920×1080, pts
            // МАТЕРИАЛА) — картинка одинакова при любом Quality/ступени,
            // а сдвиг клипа зерно не меняет (инвариант 11 в редакции
            // генератора). Полный диапазон яркости — «серое поле».
            let frame_seed = lowbias32(pts.0 as u32 ^ 0x9e37_79b9);
            let mut buf = vec![0u8; n * 4];
            for y in 0..h as usize {
                let ny = (y as u64 * 1080 / h.max(1) as u64) as u32;
                let row_seed = lowbias32(ny ^ frame_seed.rotate_left(11));
                for x in 0..w as usize {
                    let nx = (x as u64 * 1920 / w.max(1) as u64) as u32;
                    let s = lowbias32(nx ^ row_seed);
                    let mixc = |v: u8| {
                        (128.0 + (v as f64 - 128.0) * amount).round().clamp(0.0, 255.0) as u8
                    };
                    let o = (y * w as usize + x) * 4;
                    if *mono {
                        let v = mixc(s as u8);
                        buf[o] = v;
                        buf[o + 1] = v;
                        buf[o + 2] = v;
                    } else {
                        buf[o] = mixc(s as u8);
                        buf[o + 1] = mixc((s >> 8) as u8);
                        buf[o + 2] = mixc((s >> 16) as u8);
                    }
                    buf[o + 3] = 255;
                }
            }
            buf.into_boxed_slice()
        }
        GeneratorKind::SmpteBars { .. } => {
            // упрощённая таблица: верхние 2/3 — семь полос 75 %, нижняя
            // треть — опорные блоки (−I, белый 100 %, +Q, чёрный, PLUGE);
            // значения детерминированы и заморожены тестом
            const TOP: [[u8; 3]; 7] = [
                [191, 191, 191],
                [191, 191, 0],
                [0, 191, 191],
                [0, 191, 0],
                [191, 0, 191],
                [191, 0, 0],
                [0, 0, 191],
            ];
            const BOTTOM: [[u8; 3]; 5] =
                [[0, 33, 76], [255, 255, 255], [50, 0, 106], [0, 0, 0], [16, 16, 16]];
            let mut buf = vec![0u8; n * 4];
            let split = (h as usize * 2) / 3;
            for y in 0..h as usize {
                let (bands, m): (&[[u8; 3]], usize) =
                    if y < split { (&TOP, 7) } else { (&BOTTOM, 5) };
                for x in 0..w as usize {
                    let i = (x * m / w.max(1) as usize).min(m - 1);
                    let c = bands[i];
                    let o = (y * w as usize + x) * 4;
                    buf[o] = c[0];
                    buf[o + 1] = c[1];
                    buf[o + 2] = c[2];
                    buf[o + 3] = 255;
                }
            }
            buf.into_boxed_slice()
        }
        GeneratorKind::Timecode { fps } => {
            let fps = (*fps).max(1) as i64;
            let t = pts.0.max(0);
            let secs = t / 90_000;
            let ff = (t % 90_000) * fps / 90_000;
            let label =
                format!("{:02}:{:02}:{:02}:{:02}", secs / 3600, secs % 3600 / 60, secs % 60, ff);
            let mut c = crate::pixfont::Canvas::new(w.max(1), h.max(1), [0, 0, 0, 255]);
            let chars = label.chars().count();
            let s = ((c.w * 4 / 5) / crate::pixfont::text_width(chars, 1).max(1)).clamp(1, 40);
            let tx = (c.w - crate::pixfont::text_width(chars, s)) / 2;
            let ty = (c.h - 5 * s) / 2;
            c.text(tx, ty, s, &label, [235, 235, 235, 255]);
            c.buf.into_boxed_slice()
        }
        // --- Р.53 (пп.419-422): фигуры, подложка, отсчёт, таблицы ------
        GeneratorKind::Transparent => vec![0u8; n * 4].into_boxed_slice(),
        GeneratorKind::Rect { cx, cy, w: rw, h: rh, radius, feather, .. } => {
            let (r, g, b, a) = shape_color(kind);
            let (fw, fh) = (w.max(1) as i64, h.max(1) as i64);
            // всё — в целых 1/256 px: float живёт ТОЛЬКО в преамбуле
            // кадра (правило kt-media/CLAUDE.md), пиксельный цикл целый
            let q = |v: f64| (v * 256.0).round() as i64;
            let (ccx, ccy) = (q(cx * fw as f64), q(cy * fh as f64));
            let (hx, hy) = (q(rw * fw as f64 / 2.0), q(rh * fh as f64 / 2.0));
            let rad = q(radius * fh as f64).clamp(0, hx.min(hy));
            let fe = q(feather * fh as f64).max(1);
            let mut buf = vec![0u8; n * 4];
            for y in 0..fh {
                let py = (y * 256 + 128) - ccy;
                for x in 0..fw {
                    let px = (x * 256 + 128) - ccx;
                    // знаковое расстояние до скруглённого прямоугольника
                    let dx = px.abs() - (hx - rad);
                    let dy = py.abs() - (hy - rad);
                    let d = if dx > 0 && dy > 0 {
                        isqrt(dx * dx + dy * dy) - rad
                    } else {
                        dx.max(dy) - rad
                    };
                    let cov = coverage(d, fe);
                    let o = ((y * fw + x) * 4) as usize;
                    buf[o] = r;
                    buf[o + 1] = g;
                    buf[o + 2] = b;
                    buf[o + 3] = ((a as i64 * cov) / 255) as u8;
                }
            }
            buf.into_boxed_slice()
        }
        GeneratorKind::Ellipse { cx, cy, w: rw, h: rh, feather, .. } => {
            let (r, g, b, a) = shape_color(kind);
            let (fw, fh) = (w.max(1) as i64, h.max(1) as i64);
            let q = |v: f64| (v * 256.0).round() as i64;
            let (ccx, ccy) = (q(cx * fw as f64), q(cy * fh as f64));
            let (hx, hy) = (q(rw * fw as f64 / 2.0).max(1), q(rh * fh as f64 / 2.0).max(1));
            let fe = q(feather * fh as f64).max(1);
            let mut buf = vec![0u8; n * 4];
            for y in 0..fh {
                let py = (y * 256 + 128) - ccy;
                for x in 0..fw {
                    let px = (x * 256 + 128) - ccx;
                    // нормированное расстояние ×min(hx,hy) — приближение
                    // знакового расстояния, точное на окружности
                    let m = hx.min(hy);
                    let ex = px * m / hx;
                    let ey = py * m / hy;
                    let d = isqrt(ex * ex + ey * ey) - m;
                    let cov = coverage(d, fe);
                    let o = ((y * fw + x) * 4) as usize;
                    buf[o] = r;
                    buf[o + 1] = g;
                    buf[o + 2] = b;
                    buf[o + 3] = ((a as i64 * cov) / 255) as u8;
                }
            }
            buf.into_boxed_slice()
        }
        GeneratorKind::Line { x1, y1, x2, y2, thickness, feather, .. } => {
            let (r, g, b, a) = shape_color(kind);
            let (fw, fh) = (w.max(1) as i64, h.max(1) as i64);
            let q = |v: f64| (v * 256.0).round() as i64;
            let (ax, ay) = (q(x1 * fw as f64), q(y1 * fh as f64));
            let (bx, by) = (q(x2 * fw as f64), q(y2 * fh as f64));
            let half = q(thickness * fh as f64 / 2.0).max(1);
            let fe = q(feather * fh as f64).max(1);
            let (vx, vy) = (bx - ax, by - ay);
            let len2 = vx * vx + vy * vy;
            let mut buf = vec![0u8; n * 4];
            for y in 0..fh {
                let py = (y * 256 + 128) - ay;
                for x in 0..fw {
                    let px = (x * 256 + 128) - ax;
                    // проекция на отрезок с клампом — капсула
                    let t = if len2 == 0 {
                        0
                    } else {
                        ((px * vx + py * vy) * 256 / len2).clamp(0, 256)
                    };
                    let qx = px - vx * t / 256;
                    let qy = py - vy * t / 256;
                    let d = isqrt(qx * qx + qy * qy) - half;
                    let cov = coverage(d, fe);
                    let o = ((y * fw + x) * 4) as usize;
                    buf[o] = r;
                    buf[o + 1] = g;
                    buf[o + 2] = b;
                    buf[o + 3] = ((a as i64 * cov) / 255) as u8;
                }
            }
            buf.into_boxed_slice()
        }
        GeneratorKind::Leader { seconds, fps, .. } => {
            leader_frame(pts, *seconds, (*fps).max(1), w.max(1), h.max(1))
        }
        GeneratorKind::TestPattern { table } => table_frame(*table, w.max(1), h.max(1)),
        GeneratorKind::WindowPattern { area, level } => {
            let (fw, fh) = (w.max(1) as i64, h.max(1) as i64);
            let lv = (level.clamp(0.0, 1.0) * 255.0).round() as u8;
            // доля ПЛОЩАДИ → сторона в долях кадра: окно подобно кадру
            let side = area.clamp(0.0, 1.0).sqrt();
            let (hw, hh) = (
                ((side * fw as f64) / 2.0).round() as i64,
                ((side * fh as f64) / 2.0).round() as i64,
            );
            let mut buf = vec![0u8; n * 4];
            for y in 0..fh {
                let inside_y = (y - fh / 2).abs() < hh;
                for x in 0..fw {
                    let v = if inside_y && (x - fw / 2).abs() < hw { lv } else { 0 };
                    let o = ((y * fw + x) * 4) as usize;
                    buf[o] = v;
                    buf[o + 1] = v;
                    buf[o + 2] = v;
                    buf[o + 3] = 255;
                }
            }
            buf.into_boxed_slice()
        }
    }
}

/// Синус 1 кГц на уровне `db`, sample-accurate от ГЛОБАЛЬНОГО номера
/// сэмпла (байт-в-байт, как `AudioPattern::Sine`). `gate` решает, звучит
/// ли конкретный сэмпл: у SMPTE — всегда, у отсчёта — по воротам.
/// Один код на оба вида: второй синус разошёлся бы фазой.
fn tone_1k(idx: i64, db: f64, gate: impl Fn(i64) -> bool) -> AudioBurst {
    let amp = 10f64.powf(db / 20.0);
    let mut b = AudioBurst::silence(idx);
    let base = idx * kt_time::AUDIO_BURST_FRAMES as i64;
    for f in 0..kt_time::AUDIO_BURST_FRAMES as i64 {
        let n = base + f;
        if !gate(n) {
            continue;
        }
        let v = ((n as f64) * 1000.0 * std::f64::consts::TAU / 48_000.0).sin() * amp;
        let v = v as f32;
        b.data[(f * 2) as usize] = v;
        b.data[(f * 2 + 1) as usize] = v;
    }
    b
}

/// Целочисленный квадратный корень (Ньютон на u64): аналитическое
/// дистанс-поле фигур считается В ЦЕЛЫХ 1/256 px, и `f64::sqrt` в
/// пиксельном цикле нарушил бы это правило ради той же точности.
fn isqrt(v: i64) -> i64 {
    if v <= 0 {
        return 0;
    }
    let mut x = v as u64;
    let mut r = x;
    let mut t = x.div_ceil(2);
    while t < r {
        r = t;
        t = (r + x / r) / 2;
    }
    x = r;
    x as i64
}

/// Покрытие пикселя по знаковому расстоянию `d` (1/256 px) и ширине
/// рампы `fe`: внутри — 255, снаружи — 0, между — линейно. Та же рампа,
/// что у обводки титра (kt-text), поэтому края фигуры и края глифа
/// выглядят одинаково.
fn coverage(d: i64, fe: i64) -> i64 {
    let half = fe.max(1) / 2;
    if d <= -half {
        255
    } else if d >= half {
        0
    } else {
        255 - ((d + half) * 255) / fe.max(1)
    }
}

/// Цвет фигуры — одним местом на три вида (у всех трёх поля названы
/// одинаково, но варианты enum разные).
fn shape_color(kind: &GeneratorKind) -> (u8, u8, u8, u8) {
    match kind {
        GeneratorKind::Rect { color_r, color_g, color_b, color_a, .. }
        | GeneratorKind::Ellipse { color_r, color_g, color_b, color_a, .. }
        | GeneratorKind::Line { color_r, color_g, color_b, color_a, .. } => {
            (*color_r, *color_g, *color_b, *color_a)
        }
        _ => (255, 255, 255, 255),
    }
}

/// Инженерная таблица (Р.53, п.422). Числа заморожены тестом t531.
///
/// Полосы 75 % — восемь вертикальных колонок в порядке убывания яркости
/// (белый, жёлтый, голубой, зелёный, пурпурный, красный, синий, чёрный);
/// 75 % восьмибитной шкалы — 191. `[НЕ ПРОВЕРЕНО]` против бумажного
/// EBU 3213: первоисточник платный, значения взяты по общепринятой
/// таблице и заморожены здесь.
fn table_frame(t: kt_model::TestTable, w: u32, h: u32) -> Box<[u8]> {
    use kt_model::TestTable as T;
    let (fw, fh) = (w as usize, h as usize);
    let mut buf = vec![0u8; fw * fh * 4];
    match t {
        T::Ebu75 | T::Ebu100 => {
            let v: u8 = if t == T::Ebu75 { 191 } else { 255 };
            let bars: [[u8; 3]; 8] = [
                [v, v, v],
                [v, v, 0],
                [0, v, v],
                [0, v, 0],
                [v, 0, v],
                [v, 0, 0],
                [0, 0, v],
                [0, 0, 0],
            ];
            for y in 0..fh {
                for x in 0..fw {
                    let c = bars[(x * 8 / fw).min(7)];
                    let o = (y * fw + x) * 4;
                    buf[o..o + 3].copy_from_slice(&c);
                    buf[o + 3] = 255;
                }
            }
        }
        T::Grayscale | T::Steps10 => {
            for y in 0..fh {
                for x in 0..fw {
                    let g = if t == T::Grayscale {
                        (x * 255 / (fw - 1).max(1)) as u8
                    } else {
                        let step = (x * 10 / fw).min(9);
                        (step * 255 / 9) as u8
                    };
                    let o = (y * fw + x) * 4;
                    buf[o] = g;
                    buf[o + 1] = g;
                    buf[o + 2] = g;
                    buf[o + 3] = 255;
                }
            }
        }
        T::RampYcbcr => {
            // плоскость цветности BT.709 при постоянной яркости: Cb по X,
            // Cr по Y. Коэффициенты — в преамбуле (таблицы по столбцам и
            // строкам), пиксельный цикл целочисленный.
            let cb: Vec<i32> =
                (0..fw).map(|x| (16 + (x * 224 / (fw - 1).max(1))) as i32 - 128).collect();
            let cr: Vec<i32> =
                (0..fh).map(|y| (16 + (y * 224 / (fh - 1).max(1))) as i32 - 128).collect();
            const Y: i32 = 128;
            for (y, cr_y) in cr.iter().enumerate() {
                let (r_cr, g_cr) = ((1_5748 * cr_y) / 1_0000, (-4681 * cr_y) / 1_0000);
                for (x, cb_x) in cb.iter().enumerate() {
                    let g_cb = (-1873 * cb_x) / 1_0000;
                    let b_cb = (1_8556 * cb_x) / 1_0000;
                    let o = (y * fw + x) * 4;
                    buf[o] = (Y + r_cr).clamp(0, 255) as u8;
                    buf[o + 1] = (Y + g_cb + g_cr).clamp(0, 255) as u8;
                    buf[o + 2] = (Y + b_cb).clamp(0, 255) as u8;
                    buf[o + 3] = 255;
                }
            }
        }
    }
    buf.into_boxed_slice()
}

/// Кадр обратного отсчёта (Р.53, п.421): крест, окружность, бегущий
/// сектор и цифра секунды. Время — pts МАТЕРИАЛА, как у тайм-кода.
///
/// Последний кадр перед нулём — «2-pop»: белое поле длиной РОВНО `1/fps`.
fn leader_frame(pts: Tick, seconds: u32, fps: u32, w: u32, h: u32) -> Box<[u8]> {
    let total = seconds.max(1) as i64 * 90_000;
    let t = pts.0.clamp(0, total);
    let left = total - t; // сколько осталось до нуля, тиков
    let frame_ticks = 90_000 / fps as i64;
    let (fw, fh) = (w as i64, h as i64);
    let mut c = crate::pixfont::Canvas::new(w, h, [16, 16, 16, 255]);
    // 2-pop: ровно один кадр белого поля перед нулём
    if left <= frame_ticks {
        for px in c.buf.chunks_exact_mut(4) {
            px.copy_from_slice(&[235, 235, 235, 255]);
        }
        return c.buf.into_boxed_slice();
    }
    let (cx, cy) = (fw / 2, fh / 2);
    let radius = fw.min(fh) * 2 / 5;
    let white = [220u8, 220, 220, 255];
    // крест во весь кадр: две полосы толщиной в 1/200 меньшей стороны
    let bar = (fw.min(fh) / 200).max(1);
    for y in 0..fh {
        for x in 0..fw {
            let on_cross = (x - cx).abs() <= bar || (y - cy).abs() <= bar;
            // окружность: кольцо той же толщины
            let d = isqrt((x - cx) * (x - cx) + (y - cy) * (y - cy));
            let on_ring = (d - radius).abs() <= bar;
            // бегущий сектор: знак векторного произведения (без atan2)
            let phase = ((left % 90_000) * 4) / 90_000; // четверть, 0..3
            let rem = (left % 90_000) * 4 - phase * 90_000; // 0..90_000
            let (dx, dy) = (x - cx, y - cy);
            let swept = d <= radius && sector_covers(dx, dy, phase, rem);
            let o = ((y * fw + x) * 4) as usize;
            if on_cross || on_ring {
                c.buf[o..o + 4].copy_from_slice(&white);
            } else if swept {
                c.buf[o..o + 4].copy_from_slice(&[60, 60, 60, 255]);
            }
        }
    }
    // цифра секунды: сколько ПОЛНЫХ секунд осталось, минимум 1
    let digit = ((left + 89_999) / 90_000).max(1).to_string();
    let chars = digit.chars().count();
    let s = ((c.h / 3) / 5).clamp(1, 60);
    let tx = (c.w - crate::pixfont::text_width(chars, s)) / 2;
    let ty = (c.h - 5 * s) / 2;
    c.text(tx, ty, s, &digit, [245, 245, 245, 255]);
    c.buf.into_boxed_slice()
}

/// Накрыт ли луч (dx, dy) бегущим сектором. Целые знаки векторных
/// произведений вместо atan2 (правило «float только в преамбуле»):
/// сектор растёт по часовой стрелке от «12 часов».
fn sector_covers(dx: i64, dy: i64, phase: i64, rem: i64) -> bool {
    // квадрант точки, считая от «12 часов» по часовой: 0 = вправо-вверх
    let quad = match (dx >= 0, dy < 0) {
        (true, true) => 0,
        (true, false) => 1,
        (false, false) => 2,
        (false, true) => 3,
    };
    if quad < phase {
        return true;
    }
    if quad > phase {
        return false;
    }
    // внутри текущего квадранта: доля пройденного по хорде — сравнение
    // |поперёк| и |вдоль| через векторное произведение с границей
    let (a, b) = match quad {
        0 => (dx, -dy),
        1 => (dy, dx),
        2 => (-dx, dy),
        _ => (-dy, -dx),
    };
    a * 90_000 >= rem * (a + b).max(1)
}

/// lowbias32 (Chris Wellons) — тот же avalanche-хэш, что в kt-fx::video
/// (`noise_hash`): целочисленный, детерминированный, без состояния.
fn lowbias32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

/// Раскладка титра в пикселях кадра `w`×`h` — для рамки драга в превью
/// (Р.52 С6). Возвращает `None` для не-титра. Идёт через ту же
/// `text_params`: окно, кадр и рамка обязаны видеть одни значения.
pub fn title_layout(kind: &GeneratorKind, w: u32, h: u32) -> Option<kt_text::LayoutInfo> {
    matches!(kind, GeneratorKind::Text { .. })
        .then(|| kt_text::layout_info(&text_params(kind), w, h))
}

/// ЕДИНСТВЕННАЯ функция перевода хранимого титра (kt-model) во вход
/// растеризатора (kt-text) — второго парсера не бывает (kt-text/CLAUDE.md):
/// окно и кадр обязаны видеть одни и те же значения.
fn text_params(kind: &GeneratorKind) -> kt_text::TextParams {
    let GeneratorKind::Text {
        text,
        family,
        bold,
        size,
        line_spacing,
        tracking,
        align,
        anchor,
        pos_x,
        pos_y,
        fill_r,
        fill_g,
        fill_b,
        fill_a,
        stroke_w,
        stroke_r,
        stroke_g,
        stroke_b,
        stroke_a,
        shadow_dx,
        shadow_dy,
        shadow_blur,
        shadow_r,
        shadow_g,
        shadow_b,
        shadow_a,
        plate_pad_x,
        plate_pad_y,
        plate_r,
        plate_g,
        plate_b,
        plate_a,
        order,
    } = kind
    else {
        unreachable!("text_params is only called for GeneratorKind::Text");
    };
    let align = match align {
        kt_model::TextAlign::Left => kt_text::Align::Left,
        kt_model::TextAlign::Center => kt_text::Align::Center,
        kt_model::TextAlign::Right => kt_text::Align::Right,
    };
    let anchor = match anchor {
        kt_model::TextAnchor::TopLeft => kt_text::Anchor::TopLeft,
        kt_model::TextAnchor::TopCenter => kt_text::Anchor::TopCenter,
        kt_model::TextAnchor::TopRight => kt_text::Anchor::TopRight,
        kt_model::TextAnchor::CenterLeft => kt_text::Anchor::CenterLeft,
        kt_model::TextAnchor::Center => kt_text::Anchor::Center,
        kt_model::TextAnchor::CenterRight => kt_text::Anchor::CenterRight,
        kt_model::TextAnchor::BottomLeft => kt_text::Anchor::BottomLeft,
        kt_model::TextAnchor::BottomCenter => kt_text::Anchor::BottomCenter,
        kt_model::TextAnchor::BottomRight => kt_text::Anchor::BottomRight,
    };
    let layer = |l: &kt_model::TextLayer| match l {
        kt_model::TextLayer::Plate => kt_text::Layer::Plate,
        kt_model::TextLayer::Shadow => kt_text::Layer::Shadow,
        kt_model::TextLayer::Stroke => kt_text::Layer::Stroke,
        kt_model::TextLayer::Fill => kt_text::Layer::Fill,
    };
    kt_text::TextParams {
        text: text.clone(),
        family: family.clone(),
        style: if *bold { kt_text::FaceStyle::Bold } else { kt_text::FaceStyle::Regular },
        size: *size,
        line_spacing: *line_spacing,
        tracking: *tracking,
        align,
        anchor,
        pos: (*pos_x, *pos_y),
        fill: [*fill_r, *fill_g, *fill_b, *fill_a],
        stroke_w: *stroke_w,
        stroke: [*stroke_r, *stroke_g, *stroke_b, *stroke_a],
        shadow_dx: *shadow_dx,
        shadow_dy: *shadow_dy,
        shadow_blur: *shadow_blur,
        shadow: [*shadow_r, *shadow_g, *shadow_b, *shadow_a],
        plate_pad: (*plate_pad_x, *plate_pad_y),
        plate: [*plate_r, *plate_g, *plate_b, *plate_a],
        order: [layer(&order[0]), layer(&order[1]), layer(&order[2]), layer(&order[3])],
    }
}

impl MediaSource for GeneratorSource {
    fn info(&self) -> &MediaInfo {
        &self.info
    }

    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        // тон SMPTE: 1 кГц синус на уровне tone_db, sample-accurate от
        // глобального номера сэмпла (байт-в-байт, как AudioPattern::Sine)
        if let GeneratorKind::SmpteBars { tone: true, tone_db } = &self.kind {
            return Ok(tone_1k(idx, *tone_db, |_| true));
        }
        // Биип отсчёта (Р.53, п.421) — ТОТ ЖЕ код, что тон SMPTE, только
        // с воротами: звучит ровно на целых секундах, длиной 1/fps, и
        // молчит в остальное время. Фаза берётся от ГЛОБАЛЬНОГО номера
        // сэмпла, поэтому seek не щёлкает (как у тона).
        if let GeneratorKind::Leader { seconds, fps, beep: true, beep_db } = &self.kind {
            let total = (*seconds).max(1) as i64 * 90_000;
            let step = 90_000 / (*fps).max(1) as i64;
            return Ok(tone_1k(idx, *beep_db, |n| {
                // номер сэмпла → тик материала (48 кГц → 90 кГц, точно)
                let t = n * 90_000 / 48_000;
                if t < 0 || t > total {
                    return false;
                }
                // биип в НАЧАЛЕ каждой секунды материала, длиной 1/fps,
                // и он же на «2-pop» (конец отсчёта — тоже целая секунда)
                t % 90_000 < step || total - t <= step
            }));
        }
        Ok(AudioBurst::silence(idx))
    }

    fn read_video_frame(
        &mut self,
        pts: Tick,
        quality: Option<Quality>,
    ) -> Result<VideoFrame, MediaError> {
        let q = quality.unwrap_or(DEFAULT_Q);
        let rgba = self.frame_bytes(pts, q);
        let has_alpha = kind_has_alpha(&self.kind);
        Ok(VideoFrame { pts, width: q.width, height: q.height, rgba, has_alpha })
    }

    fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        Some(Tick(pts.0.max(0)))
    }

    /// Перенастройка на месте (Р.53, п.410): поток воркера жив, кадры
    /// старых параметров выбрасывает он сам. Отказ — когда паспорт
    /// источника изменился бы (SMPTE со включённым тоном становится
    /// звучащим медиа, а это уже другой набор клипов) или путь не
    /// разбирается.
    fn retune(&mut self, path: &str) -> bool {
        let Some(kind) = kt_model::parse_gen_path(path) else { return false };
        if Self::info_for(&kind) != self.info {
            return false;
        }
        self.kind = kind;
        self.cache = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(r: u8, g: u8, b: u8) -> GeneratorSource {
        GeneratorSource::new(GeneratorKind::Solid { color_r: r, color_g: g, color_b: b })
    }

    /// t471 (родня t208): кадр генератора детерминирован, ровно
    /// запрошенного размера, не зависит от времени, цвет — заданный.
    #[test]
    fn t471_generator_frame_is_deterministic_and_exact() {
        let q = Quality { width: 320, height: 180 };
        let mut s = solid(255, 136, 0);
        let f1 = s.read_video_frame(Tick(0), Some(q)).unwrap();
        let f2 = s.read_video_frame(Tick(90_000), Some(q)).unwrap();
        assert_eq!((f1.width, f1.height), (320, 180), "кадр обязан быть ровно Quality");
        assert_eq!(f1.rgba, f2.rgba, "кадр не должен зависеть от времени");
        assert_eq!(f2.pts.0, 90_000, "pts обязан быть запрошенный");
        assert!(!f1.has_alpha);
        assert!(f1.rgba.chunks_exact(4).all(|p| p == [255, 136, 0, 255]), "цвет не тот");

        // другой размер — другой буфер, тоже точный
        let q2 = Quality { width: 64, height: 36 };
        let f3 = s.read_video_frame(Tick(0), Some(q2)).unwrap();
        assert_eq!((f3.width, f3.height), (64, 36));
        assert_eq!(f3.rgba.len(), 64 * 36 * 4);

        // два независимых источника с теми же параметрами — байт-в-байт
        let g = solid(255, 136, 0).read_video_frame(Tick(7), Some(q)).unwrap();
        assert_eq!(g.rgba, f1.rgba);
        // другие параметры — другой кадр
        let other = solid(0, 0, 1).read_video_frame(Tick(7), Some(q)).unwrap();
        assert_ne!(other.rgba, f1.rgba);
    }

    /// t471: паспорт источника — «длительности нет», натив-разрешение на
    /// ступень декода, сетка проекта.
    #[test]
    fn t471_generator_info_contract() {
        let s = solid(0, 0, 0);
        let i = s.info();
        assert_eq!(i.duration.0, 0, "у генератора нет длительности");
        assert!(i.has_video && !i.has_audio);
        assert_eq!(i.fps, None, "своей сетки кадров нет");
        assert_eq!((i.width, i.height), (GEN_NATIVE, GEN_NATIVE));
        assert_eq!(s.keyframe_before(Tick(-5)), Some(Tick(0)));
        assert_eq!(s.keyframe_before(Tick(90_000)), Some(Tick(90_000)));
    }

    /// t498 (Р.53, п.410): генератор перенастраивается НА МЕСТЕ. Кадр
    /// после `retune` байт-в-байт равен кадру свежего источника (кэш
    /// сброшен), отказ не меняет ничего.
    #[test]
    fn t498_generator_retunes_in_place() {
        let q = Quality { width: 64, height: 36 };
        let orange = GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 };
        let mut s =
            GeneratorSource::new(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 });
        // прогреваем внутренний кэш кадра старыми параметрами
        let before = s.read_video_frame(Tick(0), Some(q)).unwrap();
        assert_eq!(&before.rgba[..4], &[0, 0, 0, 255]);

        assert!(s.retune(&kt_model::format_gen_path(&orange)), "solid → solid перенастраивается");
        let after = s.read_video_frame(Tick(0), Some(q)).unwrap();
        let fresh =
            GeneratorSource::new(orange.clone()).read_video_frame(Tick(0), Some(q)).unwrap();
        assert_eq!(after.rgba, fresh.rgba, "кэш старых параметров обязан быть сброшен");

        // отказы: битый путь, чужая схема, смена паспорта (тон SMPTE)
        let mut smpte =
            GeneratorSource::new(GeneratorKind::SmpteBars { tone: false, tone_db: -20.0 });
        let paspt = smpte.info().clone();
        let with_tone =
            kt_model::format_gen_path(&GeneratorKind::SmpteBars { tone: true, tone_db: -20.0 });
        assert!(!smpte.retune(&with_tone), "включение тона меняет паспорт — только пересоздание");
        assert_eq!(smpte.info(), &paspt, "отказ не меняет ничего");
        assert!(!smpte.retune("gen:unknown"));
        assert!(!smpte.retune("media/clip.mp4"));
        assert_eq!(smpte.info(), &paspt);

        // смена ВИДА при том же паспорте законна: клипы те же
        let mut s2 = GeneratorSource::new(orange);
        assert!(s2.retune("gen:checker"), "solid → checker: паспорт тот же");
    }

    /// t507 (Р.53, п.413): статический кадр рисуется ОДИН раз на
    /// качество и отдаётся одной копией. До этого пункта копий на запрос
    /// было две — в кэш и из кэша, то есть 16 МБ лишнего memcpy на
    /// каждый кадр 1080p.
    #[test]
    fn t507_generator_frame_is_rendered_once_per_quality() {
        let q1 = Quality { width: 64, height: 36 };
        let q2 = Quality { width: 32, height: 18 };
        let mut s =
            GeneratorSource::new(GeneratorKind::Solid { color_r: 9, color_g: 9, color_b: 9 });
        let a = s.read_video_frame(Tick(0), Some(q1)).unwrap();
        let b = s.read_video_frame(Tick(90_000), Some(q1)).unwrap();
        assert_eq!(a.rgba, b.rgba, "статический кадр не зависит от времени");
        // буферы РАЗНЫЕ (владение отдаётся наружу), но содержимое одно
        assert!(!std::ptr::eq(a.rgba.as_ptr(), b.rgba.as_ptr()));

        // смена качества туда-обратно не портит содержимое
        let small = s.read_video_frame(Tick(0), Some(q2)).unwrap();
        assert_eq!(small.rgba.len(), (q2.width * q2.height * 4) as usize);
        let again = s.read_video_frame(Tick(0), Some(q1)).unwrap();
        assert_eq!(again.rgba, a.rgba);

        // живой вид (шум) кэш по качеству не использует вовсе
        let mut n = GeneratorSource::new(GeneratorKind::NoiseField { amount: 1.0, mono: true });
        let f1 = n.read_video_frame(Tick(0), Some(q1)).unwrap();
        let f2 = n.read_video_frame(Tick(3000), Some(q1)).unwrap();
        assert_ne!(f1.rgba, f2.rgba, "шум обязан зависеть от времени");
    }

    /// t491 (Р.52 С5): шум counter-based — кадр = f(pts, x, y): повтор
    /// байт-в-байт, разные pts — разные кадры, mono серый, amount 0 —
    /// ровно 128; сетка БАЗОВАЯ (1920×1080): кадр половинного Quality —
    /// это те же клетки (независимость от Quality и ступени).
    #[test]
    fn t491_noise_field_is_counter_based() {
        let mk = |amount: f64, mono: bool| GeneratorKind::NoiseField { amount, mono };
        let q = Quality { width: 1920, height: 1080 };
        let mut s = GeneratorSource::new(mk(1.0, true));
        let a = s.read_video_frame(Tick(3000), Some(q)).unwrap();
        let b = s.read_video_frame(Tick(3000), Some(q)).unwrap();
        assert_eq!(a.rgba, b.rgba, "тот же pts — те же байты");
        let c = s.read_video_frame(Tick(6000), Some(q)).unwrap();
        assert_ne!(a.rgba, c.rgba, "другой кадр материала — другое зерно");
        assert!(a.rgba.chunks_exact(4).all(|p| p[0] == p[1] && p[1] == p[2]), "mono серый");
        // полный диапазон: и тёмные, и светлые значения есть
        assert!(a.rgba.chunks_exact(4).any(|p| p[0] < 32));
        assert!(a.rgba.chunks_exact(4).any(|p| p[0] > 224));

        // amount 0 — ровное поле 128
        let z = GeneratorSource::new(mk(0.0, true)).read_video_frame(Tick(3000), Some(q)).unwrap();
        assert!(z.rgba.chunks_exact(4).all(|p| p[0] == 128));

        // независимость от Quality: половинный кадр — те же клетки
        let half =
            s.read_video_frame(Tick(3000), Some(Quality { width: 960, height: 540 })).unwrap();
        for (hx, hy) in [(10usize, 7usize), (123, 400), (900, 100)] {
            let hpx = &half.rgba[(hy * 960 + hx) * 4];
            let fpx = &a.rgba[((hy * 2) * 1920 + hx * 2) * 4];
            assert_eq!(hpx, fpx, "({hx},{hy}): зерно обязано совпасть с полным кадром");
        }
    }

    /// t492 (Р.52 С5): SMPTE — has_audio живёт от параметра `tone`; тон
    /// 1 кГц байт-в-байт и слушается уровня; верхняя полоса — 75 % серый.
    #[test]
    fn t492_smpte_tone_and_bars() {
        let silent = GeneratorSource::new(GeneratorKind::SmpteBars { tone: false, tone_db: -20.0 });
        assert!(!silent.info().has_audio, "без тона аудио-клип не создаётся");
        let mut loud =
            GeneratorSource::new(GeneratorKind::SmpteBars { tone: true, tone_db: -20.0 });
        assert!(loud.info().has_audio);
        let a = loud.read_audio_burst(3).unwrap();
        let b = loud.read_audio_burst(3).unwrap();
        assert_eq!(a.data.as_slice(), b.data.as_slice(), "тон байт-в-байт");
        let peak = a.data.iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!((peak - 0.1).abs() < 0.01, "−20 dBFS ≈ 0.1: {peak}");
        let mut hot = GeneratorSource::new(GeneratorKind::SmpteBars { tone: true, tone_db: -6.0 });
        let h = hot.read_audio_burst(3).unwrap();
        let hpeak = h.data.iter().fold(0f32, |m, v| m.max(v.abs()));
        assert!(hpeak > peak * 3.0, "уровень слушается параметра");

        let f = GeneratorSource::new(GeneratorKind::SmpteBars { tone: false, tone_db: -20.0 })
            .read_video_frame(Tick(0), Some(Quality { width: 700, height: 300 }))
            .unwrap();
        assert_eq!(&f.rgba[..4], &[191, 191, 191, 255], "первая полоса — 75 % серый");
    }

    /// t493 (Р.52 С5): тайм-код — кадр = f(pts материала) и параметра fps.
    #[test]
    fn t493_timecode_frames_follow_material() {
        let q = Quality { width: 640, height: 360 };
        let mut s = GeneratorSource::new(GeneratorKind::Timecode { fps: 30 });
        let a = s.read_video_frame(Tick(0), Some(q)).unwrap();
        let a2 = s.read_video_frame(Tick(0), Some(q)).unwrap();
        assert_eq!(a.rgba, a2.rgba);
        let b = s.read_video_frame(Tick(90_000), Some(q)).unwrap();
        assert_ne!(a.rgba, b.rgba, "секунда сменилась — цифры другие");
        // внутри одного кадра (30 fps => 3000 тиков) картинка одна
        let c = s.read_video_frame(Tick(90_000 + 2_999), Some(q)).unwrap();
        assert_eq!(b.rgba, c.rgba, "внутри кадра тайм-код не дрожит");
        // fps — параметр: тот же pts, другой счётчик кадров
        let mut s60 = GeneratorSource::new(GeneratorKind::Timecode { fps: 60 });
        let d = s60.read_video_frame(Tick(90_000 + 2_999), Some(q)).unwrap();
        assert_ne!(c.rgba, d.rgba, "fps меняет поле FF");
    }

    /// t484 (Р.52 С2): титр-источник — кадр РОВНО Quality, с настоящей
    /// АЛЬФОЙ (титр живёт поверх других слоёв), детерминирован, от pts
    /// не зависит; стиль меняет байты; кадр рисует kt-text (вендоренный
    /// шрифт — системные тут ни при чём).
    #[test]
    fn t484_text_source_renders_title() {
        let title = |bold: bool| {
            let mut k = GeneratorKind::default_text();
            if let GeneratorKind::Text { text, bold: b, .. } = &mut k {
                *text = "Kinotrain\nР.52".into();
                *b = bold;
            }
            GeneratorSource::new(k)
        };
        let q = Quality { width: 320, height: 180 };
        let mut s = title(false);
        let f1 = s.read_video_frame(Tick(0), Some(q)).unwrap();
        let f2 = s.read_video_frame(Tick(90_000), Some(q)).unwrap();
        assert_eq!((f1.width, f1.height), (320, 180), "кадр обязан быть ровно Quality");
        assert!(f1.has_alpha, "у титра честная альфа");
        assert_eq!(f1.rgba, f2.rgba, "кадр не зависит от времени");
        let ink = f1.rgba.chunks_exact(4).filter(|px| px[3] > 0).count();
        assert!(ink > 200, "надписи на кадре нет: {ink}");
        // фон прозрачен, а не чёрен: непокрытые пиксели с нулевой альфой
        assert!(f1.rgba.chunks_exact(4).any(|px| px[3] == 0));
        // жирный стиль — другие байты (второе начертание реально грузится)
        let fb = title(true).read_video_frame(Tick(0), Some(q)).unwrap();
        assert_ne!(fb.rgba, f1.rgba, "Bold обязан отличаться от Regular");
        // паспорт тот же, что у прочих генераторов
        let i = title(false).info().clone();
        assert!(i.has_video && !i.has_audio);
        assert_eq!((i.width, i.height), (GEN_NATIVE, GEN_NATIVE));
        assert_eq!(i.duration.0, 0);
    }

    // ---- Р.53 (пп.419-422): фигуры, подложка, отсчёт, таблицы --------

    /// t526: все новые виды детерминированы и отдают РОВНО Quality.
    /// Инвариант 11 — общий на все генераторы, и новый вид обязан в него
    /// попасть, а не «наверное, тоже».
    #[test]
    fn t526_new_kinds_are_deterministic_and_exact() {
        let q = Quality { width: 96, height: 54 };
        for k in GeneratorKind::all() {
            let mut a = GeneratorSource::new(k.clone());
            let mut b = GeneratorSource::new(k.clone());
            for pts in [Tick(0), Tick(45_000), Tick(300_000)] {
                let fa = a.read_video_frame(pts, Some(q)).unwrap();
                let fb = b.read_video_frame(pts, Some(q)).unwrap();
                assert_eq!((fa.width, fa.height), (96, 54), "{}: кадр не Quality", k.tag());
                assert_eq!(fa.rgba, fb.rgba, "{} @{}: кадр не детерминирован", k.tag(), pts.0);
            }
        }
    }

    /// t527: у ФИГУР и подложки честная альфа, у остальных её нет, и
    /// правило это ИСЧЕРПЫВАЮЩЕЕ (новый вид не соберётся без ответа).
    #[test]
    fn t527_alpha_is_declared_by_every_kind() {
        let q = Quality { width: 64, height: 36 };
        for k in GeneratorKind::all() {
            let want = kind_has_alpha(&k);
            let mut s = GeneratorSource::new(k.clone());
            let f = s.read_video_frame(Tick(0), Some(q)).unwrap();
            assert_eq!(f.has_alpha, want, "{}: кадр и правило альфы разошлись", k.tag());
        }
        // подложка — РОВНО нули: держатель места не красит ничего
        let mut t = GeneratorSource::new(GeneratorKind::Transparent);
        let f = t.read_video_frame(Tick(0), Some(q)).unwrap();
        assert!(f.rgba.iter().all(|v| *v == 0), "подложка обязана быть пустой");
        assert!(f.has_alpha);
    }

    /// t528: прямоугольник стоит там, где сказано, и за его краем
    /// прозрачно. Утверждение — В ПИКСЕЛЯХ, а не «поле выставилось».
    #[test]
    fn t528_rect_covers_what_it_says() {
        let q = Quality { width: 200, height: 100 };
        let k = kt_model::parse_gen_path("gen:rect:cx=0.5:cy=0.5:w=0.5:h=0.5:c=ff8800ff")
            .expect("путь фигуры");
        let mut s = GeneratorSource::new(k);
        let f = s.read_video_frame(Tick(0), Some(q)).unwrap();
        let at = |x: u32, y: u32| {
            let o = ((y * 200 + x) * 4) as usize;
            [f.rgba[o], f.rgba[o + 1], f.rgba[o + 2], f.rgba[o + 3]]
        };
        assert_eq!(at(100, 50), [255, 136, 0, 255], "центр обязан быть залит");
        // половина ширины = 50 px от центра: 100±50 — край
        assert_eq!(at(60, 50)[3], 255, "внутри прямоугольника");
        assert_eq!(at(10, 50)[3], 0, "слева от фигуры обязано быть прозрачно");
        assert_eq!(at(190, 50)[3], 0, "справа от фигуры обязано быть прозрачно");
        assert_eq!(at(100, 5)[3], 0, "сверху обязано быть прозрачно");
    }

    /// t529: отрезок — КАПСУЛА между концами: под линией непрозрачно,
    /// в стороне прозрачно, а за концом — тоже прозрачно (кламп проекции).
    #[test]
    fn t529_line_is_a_capsule_between_its_ends() {
        let q = Quality { width: 200, height: 100 };
        let k = kt_model::parse_gen_path("gen:line:x1=0.25:y1=0.5:x2=0.75:y2=0.5:th=0.1")
            .expect("путь отрезка");
        let mut s = GeneratorSource::new(k);
        let f = s.read_video_frame(Tick(0), Some(q)).unwrap();
        let a = |x: u32, y: u32| f.rgba[(((y * 200 + x) * 4) + 3) as usize];
        assert_eq!(a(100, 50), 255, "середина отрезка");
        assert_eq!(a(60, 50), 255, "у левого конца");
        assert_eq!(a(10, 50), 0, "левее начала — пусто");
        assert_eq!(a(190, 50), 0, "правее конца — пусто");
        assert_eq!(a(100, 10), 0, "в стороне от линии — пусто");
    }

    /// t530: обратный отсчёт идёт по МАТЕРИАЛУ, цифра убывает, а
    /// последний кадр перед нулём — белое поле «2-pop» длиной ровно 1/fps.
    #[test]
    fn t530_leader_counts_down_and_pops() {
        let q = Quality { width: 120, height: 90 };
        let k = kt_model::parse_gen_path("gen:leader:n=3:fps=30").expect("путь отсчёта");
        let mut s = GeneratorSource::new(k);
        let mean = |f: &VideoFrame| {
            f.rgba.chunks_exact(4).map(|p| p[0] as u64).sum::<u64>() / (f.rgba.len() / 4) as u64
        };
        // кадры разных секунд отличаются (цифра и сектор едут)
        let a = s.read_video_frame(Tick(0), Some(q)).unwrap();
        let b = s.read_video_frame(Tick(90_000), Some(q)).unwrap();
        assert_ne!(a.rgba, b.rgba, "отсчёт обязан идти по материалу");
        // 2-pop: последние 1/30 с — светлое поле, до них темно
        let total = 3 * 90_000;
        let pop = s.read_video_frame(Tick(total - 100), Some(q)).unwrap();
        let before = s.read_video_frame(Tick(total - 90_000 / 30 - 100), Some(q)).unwrap();
        assert!(mean(&pop) > 200, "2-pop обязан быть белым полем: {}", mean(&pop));
        assert!(mean(&before) < 100, "кадром раньше белого поля быть не должно");
    }

    /// t531: числа инженерных таблиц ЗАМОРОЖЕНЫ. Таблица без
    /// зафиксированных значений — картинка, а не измерительный сигнал.
    #[test]
    fn t531_table_numbers_are_frozen() {
        use kt_model::TestTable;
        let q = Quality { width: 160, height: 40 };
        // середина полосы номер `i` из `n` — центр площадки, а не край
        let band = |t: TestTable, i: u32, n: u32| {
            let mut s = GeneratorSource::new(GeneratorKind::TestPattern { table: t });
            let f = s.read_video_frame(Tick(0), Some(q)).unwrap();
            let x = (i * 2 + 1) * 160 / (n * 2);
            let o = ((20 * 160 + x) * 4) as usize;
            [f.rgba[o], f.rgba[o + 1], f.rgba[o + 2]]
        };
        let bar = |t: TestTable, i: u32| band(t, i, 8);
        // 75 % восьмибитной шкалы — 191; порядок убывания яркости
        assert_eq!(bar(TestTable::Ebu75, 0), [191, 191, 191], "белая полоса 75 %");
        assert_eq!(bar(TestTable::Ebu75, 1), [191, 191, 0], "жёлтая");
        assert_eq!(bar(TestTable::Ebu75, 2), [0, 191, 191], "голубая");
        assert_eq!(bar(TestTable::Ebu75, 7), [0, 0, 0], "чёрная");
        assert_eq!(bar(TestTable::Ebu100, 0), [255, 255, 255], "белая полоса 100 %");
        assert_eq!(bar(TestTable::Ebu100, 6), [0, 0, 255], "синяя на полной");
        // ступени: десять ровных площадок от 0 до 255
        assert_eq!(band(TestTable::Steps10, 0, 10), [0, 0, 0]);
        assert_eq!(band(TestTable::Steps10, 7, 10), [198, 198, 198], "седьмая из десяти ступеней");
        assert_eq!(band(TestTable::Steps10, 9, 10), [255, 255, 255]);
        // шкала серого — непрерывная и монотонная по X
        let mut g =
            GeneratorSource::new(GeneratorKind::TestPattern { table: TestTable::Grayscale });
        let f = g.read_video_frame(Tick(0), Some(q)).unwrap();
        let v = |x: u32| f.rgba[((20 * 160 + x) * 4) as usize];
        assert_eq!(v(0), 0);
        assert_eq!(v(159), 255);
        assert!((1..160).all(|x| v(x) >= v(x - 1)), "шкала серого обязана быть монотонной");
    }

    /// t532: окно уровня занимает заданную ДОЛЮ ПЛОЩАДИ кадра. Это и есть
    /// смысл сигнала: по нему меряют APL и разогрев панели.
    #[test]
    fn t532_level_window_covers_the_declared_area() {
        let q = Quality { width: 400, height: 200 };
        for (area, tol) in [(0.1_f64, 0.01), (0.25, 0.01), (1.0, 0.01)] {
            let path = format!("gen:window:ar={area}");
            let k = kt_model::parse_gen_path(&path).expect("путь окна");
            let mut s = GeneratorSource::new(k);
            let f = s.read_video_frame(Tick(0), Some(q)).unwrap();
            let lit = f.rgba.chunks_exact(4).filter(|p| p[0] > 0).count() as f64;
            let frac = lit / (400.0 * 200.0);
            assert!((frac - area).abs() < tol, "{path}: площадь {frac:.3} вместо {area}");
        }
    }

    /// t533: биип отсчёта — ТОТ ЖЕ тон 1 кГц, что у SMPTE, и он звучит
    /// только на целых секундах. Без ворот отсчёт гудел бы непрерывно.
    #[test]
    fn t533_leader_beep_is_gated_to_whole_seconds() {
        let silent =
            GeneratorSource::new(kt_model::parse_gen_path("gen:leader:n=3").expect("путь"));
        assert!(!silent.info().has_audio, "без биипа аудио-клип не создаётся");
        let mut loud =
            GeneratorSource::new(kt_model::parse_gen_path("gen:leader:n=3:beep=1").expect("путь"));
        assert!(loud.info().has_audio);
        let peak = |s: &mut GeneratorSource, idx: i64| {
            s.read_audio_burst(idx).unwrap().data.iter().fold(0f32, |m, v| m.max(v.abs()))
        };
        // burst 1024 кадра = 1920 тиков; нулевая секунда материала —
        // ворота открыты (до конца остаётся ровно 3 с)
        assert!(peak(&mut loud, 0) > 0.01, "на целой секунде биип обязан звучать");
        // середина между секундами — тишина
        let mid = 48_000 / 2 / 1024;
        assert_eq!(peak(&mut loud, mid), 0.0, "между секундами обязана быть тишина");
    }

    /// t472: «нет звука» — это тишина `Ok`, никогда не `Err` (иначе
    /// negative-кэш воркера считает ложные underrun'ы).
    #[test]
    fn t472_generator_audio_is_silence_not_err() {
        let mut s = solid(10, 20, 30);
        for idx in [0_i64, 7, 1_000_000] {
            let b = s.read_audio_burst(idx).unwrap();
            assert_eq!(b.idx, idx);
            assert!(b.data.iter().all(|v| *v == 0.0), "генератор обязан молчать");
        }
    }
}
