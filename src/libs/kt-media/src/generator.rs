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
        GeneratorSource {
            kind,
            info: MediaInfo {
                // «длительности нет»: UI при броске подставит свой дефолт
                // (5 с), трим не ограничен, EOF не бывает.
                duration: Tick(0),
                has_video: true,
                // Тишину источник всё равно отдаёт (см. заголовок), но
                // аудио-КЛИПА у генератора нет: place_media_at создаёт
                // клипы по этим флагам.
                has_audio: false,
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

    fn frame_rgba(&mut self, q: Quality) -> &[u8] {
        if self.cache.as_ref().is_none_or(|(cq, _)| *cq != q) {
            self.cache = Some((q, render(&self.kind, q.width, q.height)));
        }
        &self.cache.as_ref().expect("frame was just built").1
    }
}

/// Кадр вида в точности `w`×`h`. Виды, зависящие от времени (таймкод,
/// шум), получат pts отдельным аргументом, когда появятся (С5) — и
/// перестанут ходить через кэш по Quality.
fn render(kind: &GeneratorKind, w: u32, h: u32) -> Box<[u8]> {
    match kind {
        GeneratorKind::Solid { color_r, color_g, color_b } => {
            let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
            for px in buf.chunks_exact_mut(4) {
                px.copy_from_slice(&[*color_r, *color_g, *color_b, 255]);
            }
            buf.into_boxed_slice()
        }
        GeneratorKind::Text { .. } => kt_text::render_rgba(&text_params(kind), w, h),
    }
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
        Ok(AudioBurst::silence(idx))
    }

    fn read_video_frame(
        &mut self,
        pts: Tick,
        quality: Option<Quality>,
    ) -> Result<VideoFrame, MediaError> {
        let q = quality.unwrap_or(DEFAULT_Q);
        let rgba = self.frame_rgba(q).to_vec().into_boxed_slice();
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
