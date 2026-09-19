//! Превью ВЛОЖЕННОЙ ЛЕНТЫ: кадры и огибающая (Р.56, п.452).
//!
//! **Претензия владельца дословно:** «у композиций нет превью почему-то» —
//! и, уточняя, «на таймлайне нет превью как у обычных событий - кадров и
//! аудио-пиков».
//!
//! До Р.56 у события-вложения на таймлайне была мини-карта вместо
//! филмстрипа, а аудио-половина не рисовала НИЧЕГО: пики ищутся по
//! `MediaId` ФАЙЛА, а `MediaRef`-а у ленты нет намеренно (п.378) — заведи
//! ей путь, и watchdog покрасил бы её офлайном, а служба индекса полезла бы
//! строить пики ffmpeg'ом. Это записывалось как «филмстрипа у ленты нет и
//! быть не может», но невозможным было только ОДНО решение — тумба по
//! файлу. У ленты есть другое: её содержимое можно ОТРЕНДЕРИТЬ тем же
//! кодом композиции, что и всё остальное (инвариант 7).
//!
//! **Форма — СЕРВИС по образцу `analysis`**, а не задача: очередь, дедуп,
//! негативный кэш, одна работа за раз.
//!
//! **Ключ — (лента, `content_hash`)**, а не путь. Хэш редакции ленты уже
//! считает движок (`kt_engine::content_hash`), и он же инвалидирует:
//! правка внутри ленты меняет хэш, и старая полоса просто перестаёт
//! спрашиваться. В sidecar-БД это НЕ кладётся: её схема ключуется
//! идентичностью ФАЙЛА (`media(path, size, mtime, xxhash)`), а у ленты
//! файла нет — притворяться нечем, поэтому кэш живёт в памяти под LRU.
//!
//! **Инвариант 5 соблюдается формой офлайн-пути:** свои короткоживущие
//! источники, никому не отданные, задача конечна и отменяема, реального
//! времени никто не ждёт. **Инвариант 8 — тем, что отрисовка только СТАВИТ
//! ЗАЯВКУ по ключу**, а ключ не зависит ни от зума, ни от прокрутки.
//!
//! **Пауза обязательна, а не желательна:** аудио ленты рендерится офлайн и
//! на длинной ленте занимает секунды процессора; работай он во время
//! воспроизведения — отнимал бы его у реального времени (инвариант 6).

use eframe::egui;
use kt_media::Quality;
use kt_model::{MediaId, Project};
use kt_time::Tick;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

/// Размер кадра полосы — тот же, что у обычной тумбы: рисуются они рядом и
/// одним кодом.
pub const REEL_THUMB_W: u32 = crate::thumbs::THUMB_W;
pub const REEL_THUMB_H: u32 = crate::thumbs::THUMB_H;

/// Потолок кадров на ленту. Это не «бюджет памяти», а обещание цены: у
/// часовой ленты шаг просто станет крупнее, и заказ останется конечным.
pub const MAX_FRAMES_PER_REEL: usize = 48;

/// Сколько лент держим готовыми (LRU). Кадр 320×180 RGBA — 230 КиБ.
pub const MAX_REELS_READY: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReelKey {
    pub seq: MediaId,
    /// Редакция содержимого ленты (`kt_engine::content_hash`).
    pub hash: u64,
}

/// Кадры ленты по её СОБСТВЕННОМУ времени: `frames[i]` — момент `i * step`.
#[derive(Debug, Clone)]
pub struct ReelStrip {
    pub step: i64,
    pub frames: Vec<Option<Arc<egui::ColorImage>>>,
    pub end: Tick,
}

impl ReelStrip {
    /// Кадр в момент ленты `t` — ближайший СЛЕВА, как у филмстрипа файла.
    pub fn at(&self, t: i64) -> Option<&Arc<egui::ColorImage>> {
        if self.step <= 0 {
            return None;
        }
        let i = (t.max(0) / self.step) as usize;
        self.frames.get(i)?.as_ref()
    }

    /// Индекс кадра — им же ключуется текстура.
    pub fn index(&self, t: i64) -> usize {
        if self.step <= 0 { 0 } else { (t.max(0) / self.step) as usize }
    }
}

/// Готовое превью ленты. Пики приходят ПОЗЖЕ кадров: кадры дёшевы и видны
/// сразу, аудио ленты считается офлайн-рендером и на длинной ленте идёт
/// секунды. До тех пор волноформа — честно пустая, а не выдуманная.
#[derive(Debug, Clone, Default)]
pub struct ReelPreview {
    pub strip: Option<ReelStrip>,
    pub peaks: Option<crate::util::Peaks>,
}

/// Что задача присылает по ходу: сначала кадры, потом итог.
pub type ReelPartial = ReelStrip;

type Job = crate::bg::BgJob<ReelPartial, Result<crate::util::Peaks, String>>;

/// Служба превью лент.
#[derive(Default)]
pub struct ReelPreviews {
    running: Option<(ReelKey, Job)>,
    queue: VecDeque<(ReelKey, Box<ReelBody>)>,
    ready: HashMap<ReelKey, Arc<ReelPreview>>,
    order: VecDeque<ReelKey>,
    failed: HashMap<ReelKey, String>,
    paused: bool,
}

type ReelBody =
    dyn FnOnce(crate::bg::BgCtl, crate::bg::BgSend<ReelPartial>) -> Result<crate::util::Peaks, String>
        + Send;

impl ReelPreviews {
    /// Шаг полосы: наименьшая ступень лестницы тумб, при которой кадров не
    /// больше потолка. Лестница та же, что у филмстрипа файла, — и ключи
    /// тогда стоят на тех же местах.
    pub fn strip_step(len: Tick) -> i64 {
        let n = MAX_FRAMES_PER_REEL as i64;
        *crate::thumbs::THUMB_STEP_LADDER
            .iter()
            .find(|&&s| len.0 <= s * n)
            .unwrap_or(crate::thumbs::THUMB_STEP_LADDER.last().unwrap())
    }

    pub fn get(&self, key: &ReelKey) -> Option<Arc<ReelPreview>> {
        self.ready.get(key).cloned()
    }

    pub fn peaks(&self, key: &ReelKey) -> Option<&crate::util::Peaks> {
        self.ready.get(key)?.peaks.as_ref()
    }

    pub fn failure(&self, key: &ReelKey) -> Option<&str> {
        self.failed.get(key).map(|s| s.as_str())
    }

    pub fn is_running(&self) -> bool {
        self.running.as_ref().is_some_and(|(_, j)| j.is_running())
    }

    pub fn pending(&self) -> usize {
        self.queue.len() + usize::from(self.running.is_some())
    }

    /// Заявка на ленту. Возвращает `false`, если делать нечего: уже
    /// готово, уже считается, уже стоит в очереди или уже не вышло.
    ///
    /// Зовётся ИЗ ОТРИСОВКИ, поэтому обязана быть дешёвой и идемпотентной:
    /// ключ не зависит ни от зума, ни от прокрутки (инвариант 8).
    pub fn request(&mut self, key: ReelKey, project: Project, index_db: Option<std::path::PathBuf>,
                   base: Option<std::path::PathBuf>) -> bool {
        if self.ready.contains_key(&key)
            || self.failed.contains_key(&key)
            || self.running.as_ref().is_some_and(|(k, _)| *k == key)
            || self.queue.iter().any(|(k, _)| *k == key)
        {
            return false;
        }
        let body = move |ctl: crate::bg::BgCtl, send: crate::bg::BgSend<ReelPartial>| {
            render_reel_preview(ctl, send, &project, base.as_deref(), index_db.as_deref())
        };
        self.queue.push_back((key, Box::new(body)));
        true
    }

    /// Правка внутри ленты: старые редакции больше не нужны, и незаконченная
    /// работа по ним — тем более.
    pub fn invalidate_seq(&mut self, seq: MediaId, keep_hash: u64) {
        let stale = |k: &ReelKey| k.seq == seq && k.hash != keep_hash;
        self.ready.retain(|k, _| !stale(k));
        self.order.retain(|k| !stale(k));
        self.failed.retain(|k, _| !stale(k));
        self.queue.retain(|(k, _)| !stale(k));
        if let Some((k, j)) = &self.running
            && stale(k)
        {
            j.cancel();
            self.running = None;
        }
    }

    pub fn cancel_all(&mut self) {
        if let Some((_, j)) = &self.running {
            j.cancel();
        }
        self.queue.clear();
        self.running = None;
    }

    /// Пауза на время воспроизведения и рендера: реальное время важнее
    /// картинки на полосе (инвариант 6).
    pub fn set_paused(&mut self, on: bool) {
        self.paused = on;
        if let Some((_, j)) = &self.running {
            j.set_paused(on);
        }
    }

    /// Двинуть службу: забрать готовое, запустить следующую. `true` — что-то
    /// изменилось (повод перерисовать).
    pub fn poll(&mut self) -> bool {
        let mut any = false;
        // забираем из джоба ВСЁ и отпускаем заём: класть в кэш — уже
        // работа со `self`
        let taken = self.running.as_mut().map(|(key, job)| {
            let changed = job.poll();
            (*key, changed, job.progress.take(), job.done.take())
        });
        if let Some((key, changed, strip, done)) = taken {
            any |= changed;
            if let Some(strip) = strip {
                let mut e = (*self.ready.get(&key).cloned().unwrap_or_default()).clone();
                e.strip = Some(strip);
                self.put(key, e);
            }
            if let Some(done) = done {
                match done {
                    Ok(peaks) => {
                        let mut e = (*self.ready.get(&key).cloned().unwrap_or_default()).clone();
                        e.peaks = Some(peaks);
                        self.put(key, e);
                    }
                    Err(e) => {
                        self.failed.insert(key, e);
                    }
                }
                self.running = None;
                any = true;
            }
        }
        // одна работа за раз: офлайн-рендер ленты декодирует файлы, и три
        // сразу отняли бы диск у воспроизведения
        if self.running.is_none()
            && let Some((key, body)) = self.queue.pop_front()
        {
            let job = crate::bg::BgJob::spawn("reel-preview", body);
            job.set_paused(self.paused);
            self.running = Some((key, job));
            any = true;
        }
        any
    }

    fn put(&mut self, key: ReelKey, e: ReelPreview) {
        if !self.ready.contains_key(&key) {
            self.order.push_back(key);
        }
        self.ready.insert(key, Arc::new(e));
        while self.order.len() > MAX_REELS_READY {
            if let Some(old) = self.order.pop_front() {
                self.ready.remove(&old);
            }
        }
    }
}

/// Медиа, которые лента читает — включая вложенные в неё ленты.
fn used_media(p: &Project) -> Vec<MediaId> {
    let mut out: Vec<MediaId> = p
        .clips
        .iter()
        .map(|c| c.media)
        .chain(p.sequences.iter().flat_map(|s| s.clips.iter().map(|c| c.media)))
        .collect();
    out.sort_unstable();
    out.dedup();
    out.retain(|m| p.media.iter().any(|r| r.id == *m));
    out
}

/// Тело задачи: сначала КАДРЫ (дёшево и сразу видно), потом огибающая.
///
/// Свои короткоживущие источники — ровно те три условия инварианта 5: ни с
/// кем не делятся, темпа не обещают, задача конечна и отменяема.
pub fn render_reel_preview(
    ctl: crate::bg::BgCtl,
    send: crate::bg::BgSend<ReelPartial>,
    p: &Project,
    base: Option<&std::path::Path>,
    index_db: Option<&std::path::Path>,
) -> Result<crate::util::Peaks, String> {
    // источники открываются ДВАЖДЫ — по разу на проход: видео и аудио
    // читают их по-разному, а делить один источник между проходами значит
    // делить и позицию чтения (инвариант 5: владелец ровно один)
    let open = || -> Vec<(MediaId, Box<dyn kt_media::MediaSource>)> {
        used_media(p)
            .into_iter()
            .filter_map(|m| {
                let r = p.media.iter().find(|r| r.id == m)?;
                let path = crate::util::resolve_media_path(base, &r.path);
                crate::util::open_source_indexed(&path, index_db).ok().map(|s| (m, s))
            })
            .collect()
    };

    let end = kt_engine::export::project_range(p);
    let step = ReelPreviews::strip_step(end);
    let q = Quality { width: REEL_THUMB_W, height: REEL_THUMB_H };

    // --- кадры -------------------------------------------------------
    let mut facts = kt_engine::video::SourceFacts::default();
    let mut src: HashMap<MediaId, Box<dyn kt_media::MediaSource>> = HashMap::new();
    for (m, s) in open() {
        facts.observe(m, s.info());
        src.insert(m, s);
    }
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig {
        max_audio_bytes: 1,
        max_video_bytes: 64 << 20,
    });
    let mut video = kt_engine::VideoRenderer::new_with_facts(p, &facts);
    let n = ((end.0 + step - 1) / step).max(1) as usize;
    let mut frames: Vec<Option<Arc<egui::ColorImage>>> = vec![None; n.min(MAX_FRAMES_PER_REEL)];
    let mut buf = Vec::new();
    for i in 0..frames.len() {
        if ctl.should_stop() {
            return Err("cancelled".into());
        }
        let t = video.grid.align_floor(Tick(i as i64 * step));
        let mut per_src: HashMap<(MediaId, Quality), Vec<Tick>> = HashMap::new();
        for (media, pts, dq) in
            video.wanted_frames(kt_time::TickRange::new(t, Tick(t.0 + video.grid.frame_step())), q)
        {
            per_src.entry((media, dq)).or_default().push(pts);
        }
        for ((media, dq), keys) in per_src {
            let Some(s) = src.get_mut(&media) else { continue };
            let sid = kt_engine::source_id(media);
            let missing: Vec<Tick> = keys
                .into_iter()
                .filter(|k| cache.get_video(sid, Some(dq), kt_engine::video_cache_idx(*k)).is_none())
                .collect();
            if missing.is_empty() {
                continue;
            }
            let _ = s.read_video_span(&missing, Some(dq), &mut |k, frame| {
                cache.insert_video(sid, Some(dq), kt_engine::video_cache_idx(k), frame.into());
            });
        }
        video.compose_from_cache_into(&cache, t, q, &mut buf);
        if buf.len() == (q.width * q.height * 4) as usize {
            frames[i] = Some(Arc::new(egui::ColorImage::from_rgba_unmultiplied(
                [q.width as usize, q.height as usize],
                &buf,
            )));
        }
    }
    // кадры уходят СРАЗУ: аудио считается дольше, и ждать его незачем
    send.send(ReelStrip { step, frames, end });
    drop(src);

    // --- огибающая ---------------------------------------------------
    if ctl.should_stop() {
        return Err("cancelled".into());
    }
    let mut acc = kt_media::index::PeakAcc::default();
    let cancel = ctl.cancel_flag().clone();
    kt_engine::export::render_raw_audio_range(
        p,
        open(),
        None,
        &cancel,
        |_| {},
        |chunk| {
            acc.push(chunk);
            Ok(())
        },
    )
    .map_err(|e| e.to_string())?;
    Ok(crate::util::pyramid_to_peaks(acc.finish()))
}
