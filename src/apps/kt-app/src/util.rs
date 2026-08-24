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
