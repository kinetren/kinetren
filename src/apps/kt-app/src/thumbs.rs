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
/// Шаг тумбов КВАНТУЕТСЯ к фиксированной лестнице (п.2): непрерывная
/// зависимость от зума давала новый набор ключей на каждый щелчок колеса —
/// филмстрип перегружался целиком. Соседние ступени переиспользуют кэш
/// (ключи крупной ступени — подмножество мелкой).
pub const THUMB_STEP_LADDER: [i64; 5] = [45_000, 90_000, 180_000, 450_000, 1_350_000]; // 0.5/1/2/5/15 c

pub fn thumb_source_step(clip_dur: Tick, inner_width_px: f32, thumb_w_px: f32) -> i64 {
    if clip_dur.0 <= 5 * 90_000 {
        // короткий клип: один тумб, ключ стабилен (кратен 0.5 c)
        return THUMB_STEP_LADDER[0].max(clip_dur.0 / 2).max(45_000);
    }
    let slots = (inner_width_px / thumb_w_px).clamp(1.0, 96.0) as i64;
    let rough = clip_dur.0 / slots.max(1);
    // ближайшая ступень СВЕРХУ (реже — дешевле)
    *THUMB_STEP_LADDER
        .iter()
        .find(|&&s| s >= rough)
        .unwrap_or(&THUMB_STEP_LADDER[THUMB_STEP_LADDER.len() - 1])
}

struct Shared {
    cond: std::sync::Condvar,
    queue: Mutex<(VecDeque<ThumbKey>, HashSet<ThumbKey>)>, // LIFO + dedup
    ready: Mutex<HashMap<ThumbKey, Arc<egui::ColorImage>>>,
    order: Mutex<VecDeque<ThumbKey>>, // для LRU-выселения ready
    /// ЖИВАЯ карта медиа: пополняется при добавлении файлов (п.3 — раньше
    /// карта замораживалась при старте и новые медиа не получали тумбы).
    media: Mutex<HashMap<MediaId, String>>,
}

pub struct ThumbService {
    shared: Arc<Shared>,
}

impl ThumbService {
    pub fn start(project: &Project, project_path: Option<PathBuf>) -> ThumbService {
        let shared = Arc::new(Shared {
            cond: std::sync::Condvar::new(),
            queue: Mutex::new((VecDeque::new(), HashSet::new())),
            ready: Mutex::new(HashMap::new()),
            order: Mutex::new(VecDeque::new()),
            media: Mutex::new(HashMap::new()),
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
        *shared.media.lock().unwrap() = media;
        let db_path = project_path.map(|p| p.with_extension("ktindex.db"));
        let sh = shared.clone();
        std::thread::Builder::new()
            .name("thumbs".into())
            .spawn(move || worker(sh, db_path))
            .expect("spawn thumbs");
        ThumbService { shared }
    }

    /// Поставить в очередь (dedup); отрисовка никогда не блокируется.
    /// Зарегистрировать новое медиа (добавление файла в проект) — тумбы
    /// начинают строиться сразу, без перезапуска сервиса.
    pub fn add_media(&self, id: MediaId, resolved_path: String) {
        if !resolved_path.starts_with("synth:") {
            self.shared.media.lock().unwrap().insert(id, resolved_path);
        }
    }

    pub fn request(&self, key: ThumbKey) {
        let mut q = self.shared.queue.lock().unwrap();
        if q.1.insert(key) {
            q.0.push_back(key); // pop_back = LIFO: свежие запросы первыми
            drop(q);
            self.shared.cond.notify_one();
        }
    }

    pub fn get(&self, key: &ThumbKey) -> Option<Arc<egui::ColorImage>> {
        self.shared.ready.lock().unwrap().get(key).cloned()
    }
}

fn worker(shared: Arc<Shared>, db_path: Option<PathBuf>) {
    let mut db = db_path.and_then(|p| kt_media::index::MediaIndexDb::open(&p).ok());
    // ленивые источники (≤2 открытых одновременно)
    let mut sources: HashMap<MediaId, Box<dyn kt_media::MediaSource>> = HashMap::new();
    let mut db_ids: HashMap<MediaId, i64> = HashMap::new();
    loop {
        // condvar вместо 33 пробуждений/с на пустой очереди (п.1)
        let key = {
            let mut q = shared.queue.lock().unwrap();
            loop {
                if let Some(k) = q.0.pop_back() {
                    q.1.remove(&k);
                    break k;
                }
                let (guard, _timeout) =
                    shared.cond.wait_timeout(q, std::time::Duration::from_secs(2)).unwrap();
                q = guard;
            }
        };
        if shared.ready.lock().unwrap().contains_key(&key) {
            continue;
        }
        let Some(path) = shared.media.lock().unwrap().get(&key.media).cloned() else {
            continue;
        };
        let path = &path;

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
