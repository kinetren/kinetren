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
        // единственный звучащий вид — SMPTE с включённым тоном
        let has_audio = matches!(kind, GeneratorKind::SmpteBars { tone: true, .. });
        GeneratorSource {
            kind,
            info: MediaInfo {
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
            },
            cache: None,
        }
    }

    pub fn kind(&self) -> &GeneratorKind {
        &self.kind
    }

    fn frame_rgba(&mut self, pts: Tick, q: Quality) -> std::borrow::Cow<'_, [u8]> {
        if depends_on_time(&self.kind) {
            // живые виды (шум, тайм-код) рисуются на каждый запрос —
            // кэш по Quality им не годится, а кадр дешёвый
            return std::borrow::Cow::Owned(render_at(&self.kind, pts, q.width, q.height).into());
        }
        if self.cache.as_ref().is_none_or(|(cq, _)| *cq != q) {
            self.cache = Some((q, render_at(&self.kind, Tick(0), q.width, q.height)));
        }
        std::borrow::Cow::Borrowed(&self.cache.as_ref().expect("frame was just built").1)
    }
}

fn depends_on_time(kind: &GeneratorKind) -> bool {
    matches!(kind, GeneratorKind::NoiseField { .. } | GeneratorKind::Timecode { .. })
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
            let corners =
                [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)].map(|(x, y): (f64, f64)| x * dx + y * dy);
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
                    buf[o..o + 4].copy_from_slice(if (cx + cy).is_multiple_of(2) { &a } else { &b });
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
            let label = format!(
                "{:02}:{:02}:{:02}:{:02}",
                secs / 3600,
                secs % 3600 / 60,
                secs % 60,
                ff
            );
            let mut c = crate::pixfont::Canvas::new(w.max(1), h.max(1), [0, 0, 0, 255]);
            let chars = label.chars().count();
            let s = ((c.w * 4 / 5) / crate::pixfont::text_width(chars, 1).max(1)).clamp(1, 40);
            let tx = (c.w - crate::pixfont::text_width(chars, s)) / 2;
            let ty = (c.h - 5 * s) / 2;
            c.text(tx, ty, s, &label, [235, 235, 235, 255]);
            c.buf.into_boxed_slice()
        }
    }
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
            let amp = 10f64.powf(tone_db / 20.0);
            let mut b = AudioBurst::silence(idx);
            let base = idx * kt_time::AUDIO_BURST_FRAMES as i64;
            for f in 0..kt_time::AUDIO_BURST_FRAMES as i64 {
                let n = base + f;
                let v = ((n as f64) * 1000.0 * std::f64::consts::TAU / 48_000.0).sin() * amp;
                let v = v as f32;
                b.data[(f * 2) as usize] = v;
                b.data[(f * 2 + 1) as usize] = v;
            }
            return Ok(b);
        }
        Ok(AudioBurst::silence(idx))
    }

    fn read_video_frame(
        &mut self,
        pts: Tick,
        quality: Option<Quality>,
    ) -> Result<VideoFrame, MediaError> {
        let q = quality.unwrap_or(DEFAULT_Q);
        let rgba = self.frame_rgba(pts, q).to_vec().into_boxed_slice();
        // у титра честная альфа: он живёт ПОВЕРХ других слоёв
        let has_alpha = matches!(self.kind, GeneratorKind::Text { .. });
        Ok(VideoFrame { pts, width: q.width, height: q.height, rgba, has_alpha })
    }

    fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        Some(Tick(pts.0.max(0)))
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
        let z = GeneratorSource::new(mk(0.0, true))
            .read_video_frame(Tick(3000), Some(q))
            .unwrap();
        assert!(z.rgba.chunks_exact(4).all(|p| p[0] == 128));

        // независимость от Quality: половинный кадр — те же клетки
        let half = s
            .read_video_frame(Tick(3000), Some(Quality { width: 960, height: 540 }))
            .unwrap();
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
        let mut loud = GeneratorSource::new(GeneratorKind::SmpteBars { tone: true, tone_db: -20.0 });
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
