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
    if let Some(p) = synth_pattern(path) {
        return Ok(Box::new(SyntheticSource::new(Tick(90_000 * 3600), p)));
    }
    kt_media::ffmpeg::FfmpegSource::open(path).map(|s| Box::new(s) as _).map_err(|e| e.to_string())
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
                            if !complete && let Ok(mut src) = open_source(&path) {
                                // отдельный короткоживущий источник —
                                // не декодеры воспроизведения
                                let _ = db.build_audio_peaks(mid, src.as_mut());
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
                    None
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
pub fn spawn_media_watchdog(
    project: &kt_model::Project,
    base: Option<&Path>,
) -> Arc<Mutex<std::collections::HashSet<MediaId>>> {
    let set: Arc<Mutex<std::collections::HashSet<MediaId>>> =
        Arc::new(Mutex::new(std::collections::HashSet::new()));
    let paths: Vec<(MediaId, String)> = project
        .media
        .iter()
        .filter(|m| !m.path.starts_with("synth:"))
        .map(|m| (m.id, resolve_media_path(base, &m.path)))
        .collect();
    let out = set.clone();
    std::thread::Builder::new()
        .name("media-watchdog".into())
        .spawn(move || {
            loop {
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
    set
}
