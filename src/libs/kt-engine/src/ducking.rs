//! Автодакинг (Р.48, п.341): музыка сама приглушается под голосом.
//!
//! Это СНИМОК, а не живой сайдчейн: по явной команде движок считает
//! огибающую дорожки-источника и отдаёт интервалы, где она звучит.
//! Кривую громкости по этим интервалам строит `kt_model::ops` — так
//! правка остаётся обычной командой модели (один undo-шаг), а звук не
//! начинает зависеть от другой дорожки в реальном времени (это было бы
//! XL-работа и удар по инварианту «выход события — функция события»).
//!
//! Огибающая считается ЧЕСТНЫМ рендером: гейны, фейды, переходы и FX
//! источника уже учтены — иначе «голос» под ревербом или с кривой
//! громкости давал бы не те границы.

use kt_model::{ClipKind, Project};

/// Параметры анализа. Всё в понятных монтажёру единицах — их же показывает
/// диалог.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DuckParams {
    /// Порог «источник звучит», дБFS.
    pub threshold_db: f64,
    /// Насколько приглушать музыку, дБ (глубина дакинга).
    pub depth_db: f64,
    /// Время ухода вниз и возврата, мс — наклоны трапеции кривой.
    pub attack_ms: f64,
    pub release_ms: f64,
}

impl Default for DuckParams {
    fn default() -> DuckParams {
        DuckParams { threshold_db: -35.0, depth_db: 9.0, attack_ms: 120.0, release_ms: 400.0 }
    }
}

/// Окно RMS и шаг анализа: 20 мс окно / 10 мс шаг — речь ловится
/// целиком, а короткий щелчок не открывает дакинг на всю фразу.
const WIN: i64 = 960;
const HOP: i64 = 480;
/// Паузы короче этого сливаются: между словами дакинг не должен «икать».
const MERGE_GAP_FRAMES: i64 = 14_400; // 300 мс
/// Спаны короче этого отбрасываются (щелчок, вдох).
const MIN_SPAN_FRAMES: i64 = 7_200; // 150 мс

/// Интервалы (кадры таймлайна), в которых дорожка-источник звучит громче
/// порога. Детерминированная функция проекта и параметров.
pub fn duck_spans(
    project: &Project,
    fetcher: &mut dyn crate::AudioFetcher,
    src_lane: u32,
    params: DuckParams,
) -> Vec<std::ops::Range<i64>> {
    // проект из одной дорожки: рендерим её так, как она звучит в миксе
    let mut solo = project.clone();
    solo.clips.retain(|c| c.kind == ClipKind::Audio && c.lane == src_lane);
    // переход живёт, только если ОБА его конца остались на дорожке
    let alive =
        |id: Option<kt_model::ClipId>| id.is_none_or(|i| solo.clips.iter().any(|c| c.id == i));
    let kept: Vec<_> =
        solo.transitions.iter().filter(|t| alive(t.a) && alive(t.b)).cloned().collect();
    solo.transitions = kept;
    if solo.clips.is_empty() {
        return Vec::new();
    }
    let from = solo.clips.iter().map(|c| c.start.to_audio_frames()).min().unwrap_or(0).max(0);
    let to = solo.clips.iter().map(|c| (c.start + c.duration).to_audio_frames()).max().unwrap_or(0);
    if to <= from {
        return Vec::new();
    }
    let renderer = crate::AudioRenderer::new(&solo);
    let thr = 10f64.powf(params.threshold_db / 20.0);

    // рендерим чанками и считаем RMS скользящим окном
    const CHUNK: i64 = 48_000;
    let ch = kt_time::AUDIO_CHANNELS;
    let mut loud: Vec<(i64, bool)> = Vec::new();
    let mut pos = from;
    let mut tail: Vec<f32> = Vec::new(); // хвост предыдущего чанка на окно
    while pos < to {
        let n = (to - pos).min(CHUNK);
        let mut buf = vec![0f32; n as usize * ch];
        renderer.render_frames(fetcher, pos, &mut buf);
        // объединяем с хвостом, чтобы окно не рвалось на границе чанка
        let head = tail.len() as i64 / ch as i64;
        let mut data = std::mem::take(&mut tail);
        data.extend_from_slice(&buf);
        let total = data.len() as i64 / ch as i64;
        let mut w = 0i64;
        while w + WIN <= total {
            let a = (w * ch as i64) as usize;
            let b = ((w + WIN) * ch as i64) as usize;
            let e: f64 = data[a..b].iter().map(|v| (*v as f64) * (*v as f64)).sum();
            let rms = (e / (WIN * ch as i64) as f64).sqrt();
            loud.push((pos - head + w, rms >= thr));
            w += HOP;
        }
        // хвост — последнее неполное окно
        let keep = (total - w).max(0);
        tail = data[((total - keep) * ch as i64) as usize..].to_vec();
        pos += n;
    }

    // булевы окна → интервалы
    let mut spans: Vec<std::ops::Range<i64>> = Vec::new();
    for (at, on) in loud {
        if !on {
            continue;
        }
        let (s, e) = (at, at + WIN);
        match spans.last_mut() {
            Some(last) if s <= last.end => last.end = last.end.max(e),
            _ => spans.push(s..e),
        }
    }
    // слияние коротких пауз и отброс коротких спанов — иначе кривая
    // «икает» между словами и плодит ключи
    let mut merged: Vec<std::ops::Range<i64>> = Vec::new();
    for sp in spans {
        match merged.last_mut() {
            Some(last) if sp.start - last.end <= MERGE_GAP_FRAMES => last.end = sp.end,
            _ => merged.push(sp),
        }
    }
    merged.retain(|sp| sp.end - sp.start >= MIN_SPAN_FRAMES);
    merged
}
