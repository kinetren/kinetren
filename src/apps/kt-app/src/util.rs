//! Вспомогательное: источники, пути, фоновый индекс-воркер.

use kt_media::MediaSource;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::MediaId;
use kt_time::Tick;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

pub type Peaks = Vec<(Vec<(i16, i16)>, Vec<(i16, i16)>)>; // [level] -> (L, R) по блокам

pub fn synth_pattern(path: &str) -> Option<AudioPattern> {
    let rest = path.strip_prefix("synth:")?;
    let mut it = rest.split(':');
    match it.next()? {
        "silence" => Some(AudioPattern::Silence),
        "ramp" => Some(AudioPattern::Ramp),
        "impulse" => Some(AudioPattern::Impulse),
        "sine" => Some(AudioPattern::Sine { hz: it.next()?.parse().ok()? }),
        "dc" => Some(AudioPattern::Dc { value: it.next()?.parse().ok()? }),
        _ => None,
    }
}

pub fn open_source(path: &str) -> Result<Box<dyn MediaSource>, String> {
    open_source_indexed(path, None)
}

/// Открытие через sidecar индекс-БД проекта: если файл не менялся,
/// пакетный индекс берётся из БД и полный скан-проход пропускается
/// (быстрое открытие больших файлов, тест 34). Соединение короткоживущее —
/// WAL допускает параллельных читателей/писателей из других потоков.
pub fn open_source_indexed(
    path: &str,
    db_path: Option<&Path>,
) -> Result<Box<dyn MediaSource>, String> {
    if let Some(p) = synth_pattern(path) {
        return Ok(Box::new(SyntheticSource::new(Tick(90_000 * 3600), p)));
    }
    let mut db = db_path.and_then(|p| kt_media::index::MediaIndexDb::open(p).ok());
    kt_media::ffmpeg::FfmpegSource::open_indexed(path, db.as_mut())
        .map(|s| Box::new(s) as _)
        .map_err(|e| e.to_string())
}

/// Источник или ПЛЕЙСХОЛДЕР вместо него (Р.33 п.10): недоступный файл не
/// должен мешать открыть проект, играть и рендерить. Возвращает вторым
/// значением текст ошибки — вызывающий решает, как о ней сказать.
///
/// Сам `open_source_indexed` не трогаем: его «ошибка = ошибка» нужна там,
/// где плейсхолдер вреден — добавление медиа в проект (опечатка в пути
/// молча превратилась бы в заглушку) и опрос настоящих разрешений
/// источников для меню рендера.
pub fn open_source_or_placeholder(
    path: &str,
    db_path: Option<&Path>,
    label: &str,
) -> (Box<dyn MediaSource>, Option<String>) {
    match open_source_indexed(path, db_path) {
        Ok(s) => (s, None),
        Err(e) => (Box::new(kt_media::placeholder::PlaceholderSource::new(label)), Some(e)),
    }
}

/// Имя файла для подписи плейсхолдера и уведомлений.
pub fn media_label(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

pub fn resolve_media_path(base: Option<&Path>, path: &str) -> String {
    if path.starts_with("synth:") || Path::new(path).is_absolute() {
        return path.to_string();
    }
    match base {
        Some(b) => b.join(path).to_string_lossy().to_string(),
        None => path.to_string(),
    }
}

/// Фоновое построение индекс-БД + загрузка peak-пирамид для отрисовки.
pub fn spawn_index_worker(
    db_path: Option<PathBuf>,
    media: Vec<(MediaId, String)>,
    out: Arc<Mutex<HashMap<MediaId, Peaks>>>,
) {
    std::thread::Builder::new()
        .name("index-worker".into())
        .spawn(move || {
            let mut db = db_path.and_then(|p| kt_media::index::MediaIndexDb::open(&p).ok());
            for (id, path) in media {
                if out.lock().unwrap().contains_key(&id) {
                    continue;
                }
                let peaks: Option<Peaks> = if path.starts_with("synth:") {
                    open_source(&path).ok().map(|mut s| {
                        kt_media::index::PeakPyramid::build(s.as_mut())
                            .levels
                            .into_iter()
                            .map(|mut chs| {
                                let l = if chs.is_empty() { Vec::new() } else { chs.remove(0) };
                                let r = if chs.is_empty() { l.clone() } else { chs.remove(0) };
                                (l, r)
                            })
                            .collect()
                    })
                } else if let Some(db) = &mut db {
                    match db.ensure_media(Path::new(&path)) {
                        Ok((mid, status)) => {
                            let complete = db.is_complete(mid).unwrap_or(false)
                                && status == kt_media::index::MediaEntryStatus::Reused;
                            if !complete
                                && let Ok(mut src) =
                                    kt_media::ffmpeg::FfmpegSource::open_indexed(&path, Some(db))
                            {
                                // отдельный короткоживущий источник —
                                // не декодеры воспроизведения; заодно
                                // персистится пакетный индекс (тест 34)
                                let _ = db.build_audio_peaks(mid, &mut src);
                                let _ = db.mark_complete(mid);
                            }
                            let mut levels = Vec::new();
                            let mut level = 0u32;
                            while let Ok(Some(l)) = db.audio_peaks(mid, level, 0) {
                                let r = db
                                    .audio_peaks(mid, level, 1)
                                    .ok()
                                    .flatten()
                                    .unwrap_or_else(|| l.clone());
                                levels.push((l, r));
                                level += 1;
                            }
                            Some(levels)
                        }
                        Err(_) => None,
                    }
                } else {
                    // Проект без пути → БД нет, но пики нужны (п.3:
                    // «пропало превью аудио» у безымянного проекта):
                    // строим пирамиду в памяти — при сохранении проекта
                    // воркер перезапустится уже с БД
                    open_source(&path).ok().map(|mut s| {
                        kt_media::index::PeakPyramid::build(s.as_mut())
                            .levels
                            .into_iter()
                            .map(|mut chs| {
                                let l = if chs.is_empty() { Vec::new() } else { chs.remove(0) };
                                let r = if chs.is_empty() { l.clone() } else { chs.remove(0) };
                                (l, r)
                            })
                            .collect()
                    })
                };
                if let Some(p) = peaks {
                    out.lock().unwrap().insert(id, p);
                }
            }
        })
        .expect("spawn index worker");
}

/// Watchdog доступности медиафайлов (порт mediaSourceProbeTimer 900 мс):
/// оффлайн-множество читает UI (тинт клипов); работает в своём потоке.
/// Хэндл watchdog'а: Drop останавливает поток (раньше при переоткрытии
/// проекта старые watchdog'и жили вечно — утечка потоков, п.1).
pub struct WatchdogHandle {
    stop: Arc<std::sync::atomic::AtomicBool>,
}

impl Drop for WatchdogHandle {
    fn drop(&mut self) {
        self.stop.store(true, std::sync::atomic::Ordering::Release);
    }
}

pub fn spawn_media_watchdog(
    project: &kt_model::Project,
    base: Option<&Path>,
) -> (Arc<Mutex<std::collections::HashSet<MediaId>>>, WatchdogHandle) {
    let set: Arc<Mutex<std::collections::HashSet<MediaId>>> =
        Arc::new(Mutex::new(std::collections::HashSet::new()));
    let paths: Vec<(MediaId, String)> = project
        .media
        .iter()
        .filter(|m| !m.path.starts_with("synth:"))
        .map(|m| (m.id, resolve_media_path(base, &m.path)))
        .collect();
    let out = set.clone();
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stop_t = stop.clone();
    std::thread::Builder::new()
        .name("media-watchdog".into())
        .spawn(move || {
            while !stop_t.load(std::sync::atomic::Ordering::Acquire) {
                let mut offline = std::collections::HashSet::new();
                for (id, p) in &paths {
                    if !Path::new(p).exists() {
                        offline.insert(*id);
                    }
                }
                *out.lock().unwrap() = offline;
                std::thread::sleep(std::time::Duration::from_millis(900));
            }
        })
        .expect("spawn media watchdog");
    (set, WatchdogHandle { stop })
}

/// Тайм-код «чч:мм:сс.млс Fкк» (п.19 р.6, тест 63) — верхняя строка.
pub fn format_timecode(t: kt_time::Tick, fps: u32) -> String {
    let ms = t.as_ms();
    let fps = fps.max(1) as i64;
    let frame = t.0.rem_euclid(90_000) / (90_000 / fps);
    format!(
        "{:02}:{:02}:{:02}.{:03} F{:02}",
        ms / 3_600_000,
        ms % 3_600_000 / 60_000,
        ms % 60_000 / 1000,
        ms % 1000,
        frame
    )
}

/// Разбор введённого тайм-кода (п.4 р.10). Толерантен: «90» → 90 с,
/// «1:30» → мм:сс, «1:02:03» → чч:мм:сс, можно с дробной частью
/// («1:02:03.250»). Результат выравнивается по сетке кадров — иначе
/// после перехода номер кадра дёргается. Мусор → None.
pub fn parse_timecode(s: &str, fps: u32) -> Option<kt_time::Tick> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut parts: Vec<&str> = s.split(':').collect();
    if parts.len() > 3 {
        return None;
    }
    // последняя группа может нести дробную часть секунд
    let last = parts.pop()?;
    let (sec_str, frac_str) = match last.split_once(['.', ',']) {
        Some((a, b)) => (a, Some(b)),
        None => (last, None),
    };
    let mut secs: f64 = 0.0;
    for p in &parts {
        let v: f64 = p.trim().parse().ok()?;
        if v < 0.0 {
            return None;
        }
        secs = secs * 60.0 + v;
    }
    let tail: f64 = if sec_str.trim().is_empty() && !parts.is_empty() {
        0.0 // «1:» — минуты без секунд
    } else {
        sec_str.trim().parse().ok()?
    };
    if tail < 0.0 {
        return None;
    }
    secs = secs * 60.0 + tail;
    if let Some(f) = frac_str {
        let f = f.trim();
        if !f.is_empty() {
            let digits: f64 = f.parse().ok()?;
            secs += digits / 10f64.powi(f.len() as i32);
        }
    }
    let grid = kt_time::FpsGrid::new(fps.max(1));
    Some(grid.align_floor(kt_time::Tick::from_secs_f64(secs)))
}


/// Маска ввода таймкода (п.9 р.11): цифры набиваются СПРАВА НАЛЕВО
/// (решение пользователя) в разряды ЧЧММСС, ненабранные старшие разряды —
/// нули. Так «130» это 00:01:30, а не «что-то от текущей позиции»: как в
/// монтажках, где частичный тайм-код всегда дополняется нулями.
pub const TC_MASK_DIGITS: usize = 6;

/// Разряды маски (ровно 6 символов) при текущем вводе.
pub fn mask_digits(input: &str) -> String {
    let d: Vec<char> = input.chars().filter(|c| c.is_ascii_digit()).collect();
    let d: String = d[d.len().saturating_sub(TC_MASK_DIGITS)..].iter().collect();
    format!("{}{}", "0".repeat(TC_MASK_DIGITS - d.len()), d)
}

/// Тик по маске. Минуты/секунды не нормализуются в 0..59 намеренно:
/// «00:99:00» = 99 минут, так набирать быстрее и это не сюрприз.
pub fn mask_to_tick(input: &str, fps: u32) -> kt_time::Tick {
    let d = mask_digits(input);
    let n = |a: usize, b: usize| -> i64 { d[a..b].parse::<i64>().unwrap_or(0) };
    let secs = n(0, 2) * 3600 + n(2, 4) * 60 + n(4, 6);
    let step = 90_000 / fps.max(1) as i64;
    kt_time::Tick(secs * 90_000 / step * step)
}

/// Разряды ЧЧММСС из тика — для вставки готового тайм-кода в маску.
pub fn tick_to_mask(t: kt_time::Tick) -> String {
    let ms = t.as_ms().max(0);
    format!("{:02}{:02}{:02}", (ms / 3_600_000).min(99), ms % 3_600_000 / 60_000, ms % 60_000 / 1000)
}

/// Как маска выглядит на экране: (ненабранная часть, набранная часть).
/// Первая рисуется приглушённо — сразу видно, что уже зафиксировано.
pub fn mask_text(input: &str) -> (String, String) {
    let d = mask_digits(input);
    let typed = input.chars().filter(|c| c.is_ascii_digit()).count().min(TC_MASK_DIGITS);
    let full = format!("{}:{}:{}", &d[0..2], &d[2..4], &d[4..6]);
    // индексы первого набранного символа в «чч:мм:сс» для 0..6 цифр
    let cut = [8usize, 7, 6, 4, 3, 1, 0][typed];
    (full[..cut].to_string(), full[cut..].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
        static GEN: std::sync::OnceLock<()> = std::sync::OnceLock::new();
        GEN.get_or_init(|| {
            let st = std::process::Command::new("bash")
                .arg(dir.join("gen.sh"))
                .status()
                .expect("run fixtures/gen.sh");
            assert!(st.success());
        });
        dir.join("out").join(name).to_string_lossy().to_string()
    }

    /// Тест 63 (п.19 р.6): тайм-код верхней строки.
    #[test]
    fn t63_format_timecode() {
        use kt_time::Tick;
        assert_eq!(format_timecode(Tick(0), 30), "00:00:00.000 F00");
        assert_eq!(format_timecode(Tick(90_000 - 3000), 30), "00:00:00.966 F29");
        assert_eq!(format_timecode(Tick(59 * 90_000), 30), "00:00:59.000 F00");
        assert_eq!(format_timecode(Tick(3600 * 90_000), 30), "01:00:00.000 F00");
        assert_eq!(format_timecode(Tick(90_000 / 24 * 5), 24), "00:00:00.208 F05");
    }

    /// Тест 93 (п.4 р.10): разбор введённого тайм-кода.
    #[test]
    fn t93_parse_timecode() {
        use kt_time::{FpsGrid, Tick};
        let g = FpsGrid::new(30);
        let p = |s: &str| parse_timecode(s, 30);
        // формы ввода
        assert_eq!(p("90"), Some(g.align_floor(Tick(90 * 90_000))));
        assert_eq!(p("1:30"), Some(g.align_floor(Tick(90 * 90_000))));
        assert_eq!(p("1:02:03"), Some(g.align_floor(Tick(3723 * 90_000))));
        assert_eq!(p("0:00:01.5"), Some(g.align_floor(Tick(90_000 + 45_000))));
        assert_eq!(p("  2:00  "), Some(g.align_floor(Tick(120 * 90_000))));
        // результат всегда на границе кадра
        let t = p("0:00:01.999").unwrap();
        assert_eq!(g.align_floor(t), t, "не выровнено по сетке кадров");
        // round-trip с форматтером: строка несёт миллисекунды, поэтому
        // допуск — один кадр (шаг сетки)
        for ticks in [0i64, 90_000, 3_723_000, 12_345_678] {
            let s = format_timecode(Tick(ticks), 30);
            let head = s.split(' ').next().unwrap();
            let back = parse_timecode(head, 30).expect("round-trip");
            assert!(
                (back.0 - g.align_floor(Tick(ticks)).0).abs() <= g.frame_step(),
                "round-trip {s}: {} vs {}",
                back.0,
                ticks
            );
        }
        // мусор
        assert_eq!(p(""), None);
        assert_eq!(p("abc"), None);
        assert_eq!(p("1:2:3:4"), None);
        assert_eq!(p("-5"), None);
    }

    /// Тест 23 (п.3): пики строятся и БЕЗ индекс-БД (безымянный проект) —
    /// раньше db_path=None означал «пиков не будет вообще».
    #[test]
    fn t23_peaks_build_without_db() {
        let out: Arc<Mutex<HashMap<MediaId, Peaks>>> = Arc::new(Mutex::new(HashMap::new()));
        spawn_index_worker(
            None, // безымянный проект
            vec![(MediaId(7), fixture("sine48k_stereo.flac"))],
            out.clone(),
        );
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            if let Some(p) = out.lock().unwrap().get(&MediaId(7)) {
                assert!(!p.is_empty(), "пирамида должна иметь уровни");
                let l0 = &p[0].0;
                assert!(!l0.is_empty(), "уровень 0 не пустой");
                assert!(
                    l0.iter().any(|(lo, hi)| *lo != 0 || *hi != 0),
                    "пики синуса не должны быть нулями"
                );
                break;
            }
            assert!(std::time::Instant::now() < deadline, "пики не построились за 15 с");
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
    }

    /// Тест 23 (п.3): тумбы для медиа, добавленного ПОСЛЕ старта сервиса
    /// (раньше карта медиа замораживалась при старте).
    #[test]
    fn t23_thumbs_for_media_added_after_start() {
        let project = kt_model::Project::new(kt_model::ProjectSettings {
            fps: 30,
            width: 640,
            height: 360,
            sample_rate: 48000,
        });
        let svc = crate::thumbs::ThumbService::start(&project, None);
        svc.add_media(MediaId(3), fixture("testsrc_h264.mp4"));
        let key = crate::thumbs::ThumbKey { media: MediaId(3), t: 0 };
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            svc.request(key);
            if svc.get(&key).is_some() {
                break;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "тумба добавленного медиа не построилась за 15 с"
            );
            std::thread::sleep(std::time::Duration::from_millis(30));
        }
    }
}
