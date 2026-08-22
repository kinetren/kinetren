//! ThumbService: филмстрип-тумбнейлы для видео-клипов.
//!
//! НЕ трогает декодеры воспроизведения (поправка к инварианту №8: paint
//! может только ЕНКЬЮИТЬ запросы этому выделенному воркеру). Свой
//! короткоживущий FfmpegSource на медиа; jpeg персистится в sidecar
//! индекс-БД (таблица thumbnails) — повторное открытие проекта рисует
//! полосу без декода.

use crate::util::{open_source, resolve_media_path};
use eframe::egui;
use kt_media::Quality;
use kt_model::{MediaId, Project};
use kt_time::Tick;
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub const THUMB_W: u32 = 320;
pub const THUMB_H: u32 = 180;
/// LRU готовых текстур (порт kMaxPreviewCacheEntries=384).
pub const MAX_READY: usize = 384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThumbKey {
    pub media: MediaId,
    /// Момент в тиках, выровненный вызывающим по сетке шага.
    pub t: i64,
}

/// Шаг источника для тумбнейлов (порт videoClipThumbnailSourceStepMs):
/// один тумб при клипе ≤5 c, иначе dur/slots, зажатый в 1..15 c.
pub fn thumb_source_step(clip_dur: Tick, inner_width_px: f32, thumb_w_px: f32) -> i64 {
    if clip_dur.0 <= 5 * 90_000 {
        return clip_dur.0.max(90_000);
    }
    let slots = (inner_width_px / thumb_w_px).clamp(1.0, 96.0) as i64;
    (clip_dur.0 / slots.max(1)).clamp(90_000, 15 * 90_000)
}

struct Shared {
    queue: Mutex<(VecDeque<ThumbKey>, HashSet<ThumbKey>)>, // LIFO + dedup
    ready: Mutex<HashMap<ThumbKey, Arc<egui::ColorImage>>>,
    order: Mutex<VecDeque<ThumbKey>>, // для LRU-выселения ready
}

pub struct ThumbService {
    shared: Arc<Shared>,
}

impl ThumbService {
    pub fn start(project: &Project, project_path: Option<PathBuf>) -> ThumbService {
        let shared = Arc::new(Shared {
            queue: Mutex::new((VecDeque::new(), HashSet::new())),
            ready: Mutex::new(HashMap::new()),
            order: Mutex::new(VecDeque::new()),
        });
        let media: HashMap<MediaId, String> = {
            let base = project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
            project
                .media
                .iter()
                .filter(|m| !m.path.starts_with("synth:"))
                .map(|m| (m.id, resolve_media_path(base.as_deref(), &m.path)))
                .collect()
        };
        let db_path = project_path.map(|p| p.with_extension("ktindex.db"));
        let sh = shared.clone();
        std::thread::Builder::new()
            .name("thumbs".into())
            .spawn(move || worker(sh, media, db_path))
            .expect("spawn thumbs");
        ThumbService { shared }
    }

    /// Поставить в очередь (dedup); отрисовка никогда не блокируется.
    pub fn request(&self, key: ThumbKey) {
        let mut q = self.shared.queue.lock().unwrap();
        if q.1.insert(key) {
            q.0.push_back(key); // pop_back = LIFO: свежие запросы первыми
        }
    }

    pub fn get(&self, key: &ThumbKey) -> Option<Arc<egui::ColorImage>> {
        self.shared.ready.lock().unwrap().get(key).cloned()
    }
}

fn worker(shared: Arc<Shared>, media: HashMap<MediaId, String>, db_path: Option<PathBuf>) {
    let mut db = db_path.and_then(|p| kt_media::index::MediaIndexDb::open(&p).ok());
    // ленивые источники (≤2 открытых одновременно)
    let mut sources: HashMap<MediaId, Box<dyn kt_media::MediaSource>> = HashMap::new();
    let mut db_ids: HashMap<MediaId, i64> = HashMap::new();
    loop {
        let key = {
            let mut q = shared.queue.lock().unwrap();
            match q.0.pop_back() {
                Some(k) => {
                    q.1.remove(&k);
                    Some(k)
                }
                None => None,
            }
        };
        let Some(key) = key else {
            std::thread::sleep(std::time::Duration::from_millis(30));
            continue;
        };
        if shared.ready.lock().unwrap().contains_key(&key) {
            continue;
        }
        let Some(path) = media.get(&key.media) else { continue };

        // 1) пробуем БД
        let mut jpeg: Option<Vec<u8>> = None;
        if let Some(db) = &mut db {
            let db_id = *db_ids.entry(key.media).or_insert_with(|| {
                db.ensure_media(std::path::Path::new(path)).map(|(id, _)| id).unwrap_or(-1)
            });
            if db_id >= 0 {
                jpeg = db.thumbnail(db_id, key.t).ok().flatten();
            }
        }
        // 2) декод при промахе (+ запись в БД)
        if jpeg.is_none() {
            if sources.len() > 2 && !sources.contains_key(&key.media) {
                sources.clear();
            }
            let src = sources.entry(key.media).or_insert_with(|| {
                open_source(path).unwrap_or_else(|_| {
                    Box::new(kt_media::synthetic::SyntheticSource::new(
                        Tick(0),
                        kt_media::synthetic::AudioPattern::Silence,
                    ))
                })
            });
            if let Ok(frame) =
                src.read_video_frame(Tick(key.t), Some(Quality { width: THUMB_W, height: THUMB_H }))
            {
                let mut buf = Vec::new();
                let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 80);
                let img = image::RgbaImage::from_raw(frame.width, frame.height, frame.rgba.into())
                    .unwrap();
                if image::DynamicImage::ImageRgba8(img).write_with_encoder(enc).is_ok() {
                    if let (Some(db), Some(&db_id)) = (&db, db_ids.get(&key.media))
                        && db_id >= 0
                    {
                        let _ = db.put_thumbnail(db_id, key.t, &buf);
                    }
                    jpeg = Some(buf);
                }
            }
        }
        // 3) jpeg -> ColorImage в ready (+LRU)
        if let Some(j) = jpeg
            && let Ok(img) = image::load_from_memory_with_format(&j, image::ImageFormat::Jpeg)
        {
            let rgba = img.to_rgba8();
            let ci = egui::ColorImage::from_rgba_unmultiplied(
                [rgba.width() as usize, rgba.height() as usize],
                rgba.as_raw(),
            );
            let mut ready = shared.ready.lock().unwrap();
            let mut order = shared.order.lock().unwrap();
            ready.insert(key, Arc::new(ci));
            order.push_back(key);
            while order.len() > MAX_READY {
                if let Some(old) = order.pop_front() {
                    ready.remove(&old);
                }
            }
        }
    }
}
