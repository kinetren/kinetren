// Реализация декодера. ВКЛЮЧАЕТСЯ (include!) в каждый версионный модуль,
// поэтому `ffi` и `Api` здесь — типы конкретной мажорной версии FFmpeg.
//
// Логика — порт прежнего rsmpeg-варианта (docs/porting-notes.md):
// два независимых демуксера на файл, пакетный индекс при открытии,
// sample-accurate аудио-seek, threads=1 для VP9/AV1, пересоздание
// видео-декодера при backward seek, swscale в RGBA.

use crate::{FfError, Probe, RgbaFrame};
use kt_time::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, TICK_HZ, Tick};
use std::collections::VecDeque;
use std::ffi::CString;

const FORWARD_SEEK_THRESHOLD_OUT_SAMPLES: i64 = 3 * AUDIO_SAMPLE_RATE as i64;
const VIDEO_FORWARD_DECODE_MAX_TICKS: i64 = 4 * TICK_HZ;
const AVERROR_EAGAIN: c_int = -11; // EAGAIN
const AVERROR_EOF_: c_int = -0x20464F45; // FFERRTAG('E','O','F',' ')
/// bindgen не берёт эти #define (сложные выражения / cast);
/// значения стабильны во всех мажорах FFmpeg.
const AV_NOPTS_VALUE: i64 = i64::MIN;
const AVSEEK_FLAG_BACKWARD: c_int = 1;

fn tb(num: i32, den: i32) -> ffi::AVRational {
    ffi::AVRational { num, den }
}

/// Проверяет, что `wanted` (индекс из скан-контекста) существует и имеет
/// нужный тип в ЭТОМ контексте; иначе ищет заново. У ленивых демуксеров
/// (ogg) число потоков в новом контексте может отличаться — чужой индекс
/// без проверки приводил к av_assert0 внутри libavformat (abort процесса).
fn resolve_stream_index(
    api: &Api,
    fmt: *mut ffi::AVFormatContext,
    wanted: c_int,
    media_type: ffi::AVMediaType,
    what: &str,
) -> Result<c_int, FfError> {
    let nb = unsafe { (*fmt).nb_streams } as c_int;
    if wanted >= 0 && wanted < nb {
        let st = unsafe { &**(*fmt).streams.add(wanted as usize) };
        let par = unsafe { &*st.codecpar };
        if par.codec_type == media_type
            && (st.disposition as u32 & ffi::AV_DISPOSITION_ATTACHED_PIC) == 0
        {
            return Ok(wanted);
        }
    }
    let found = unsafe {
        (api.av_find_best_stream)(fmt, media_type, -1, -1, core::ptr::null_mut(), 0)
    };
    if found >= 0 && found < nb {
        return Ok(found);
    }
    Err(FfError::Av(format!("{what}: stream not found in context (nb_streams={nb})")))
}

fn averr(api: &Api, ctx: &str, code: c_int) -> FfError {
    let mut buf = [0i8; 256];
    unsafe { (api.av_strerror)(code, buf.as_mut_ptr() as *mut c_char, buf.len()) };
    let msg = unsafe { core::ffi::CStr::from_ptr(buf.as_ptr() as *const c_char) }
        .to_string_lossy()
        .to_string();
    FfError::Av(format!("{ctx}: {msg} ({code})"))
}

/// Пакетный индекс, снятый одним сканом при открытии (без декода).
struct PacketIndex {
    /// (pkt_pts) аудио-пакетов в timebase аудио-потока.
    audio_pts: Vec<i64>,
    /// pts ключевых кадров видео (timebase видео-потока), отсортированы.
    video_keyframes: Vec<i64>,
}

struct AudioPath {
    fmt: *mut ffi::AVFormatContext,
    dec: *mut ffi::AVCodecContext,
    swr: *mut ffi::SwrContext,
    stream_index: c_int,
    time_base: ffi::AVRational,
    start_time: i64,
    fifo: VecDeque<f32>,
    /// Абсолютный номер первого кадра в fifo (48k). None до якоря.
    fifo_start: Option<i64>,
    eof: bool,
}

struct VideoPath {
    fmt: *mut ffi::AVFormatContext,
    dec: *mut ffi::AVCodecContext,
    stream_index: c_int,
    time_base: ffi::AVRational,
    start_time: i64,
    codec_id: ffi::AVCodecID,
    last_pts: Option<i64>,
    /// Кадр-ОТВЕТ на последний запрос: наибольший pts <= цели. Владение
    /// у потока — вызывающий его только конвертирует, не освобождает.
    held: *mut ffi::AVFrame,
    held_pts: i64,
    /// Первый декодированный кадр ПОСЛЕ цели: он уже стоил декода, и
    /// выбрасывать его нельзя — иначе следующий запрос уедет в seek
    /// (п.14 р.14: отсюда и брался seek-шторм при fps проекта > fps
    /// источника).
    pending: *mut ffi::AVFrame,
    pending_pts: i64,
    /// ОДИНОЧНАЯ КАРТИНКА (Р.67): в файле один кадр, и seek по нему
    /// демуксеру картинок не нужен — а попытка его сделать оставляла
    /// поток без единого кадра («out of range» на любом тике, то самое
    /// вечное «Loading…»).
    still: bool,
    sws: *mut ffi::SwsContext,
    sws_key: (c_int, c_int, c_int, c_int, c_int),
    eof: bool,
    /// Диагностика: пересоздания декодера (backward) и декодированные кадры.
    backward_reopens: u64,
    frames_decoded: u64,
}

pub struct Source {
    api: &'static Api,
    probe: Probe,
    index: PacketIndex,
    /// Индекс пришёл готовым из sidecar-БД — скан-проход не выполнялся.
    index_prebuilt: bool,
    audio: Option<AudioPath>,
    video: Option<VideoPath>,
}

// Указатели FFmpeg живут внутри и не покидают структуру; экземпляром
// владеет ровно один MediaWorker-поток (инвариант №5).
unsafe impl Send for Source {}

impl Source {
    pub fn open(api: &'static Api, path: &str) -> Result<Source, FfError> {
        Self::open_with_index(api, path, None)
    }

    pub fn open_with_index(
        api: &'static Api,
        path: &str,
        prebuilt: Option<crate::PrebuiltIndex>,
    ) -> Result<Source, FfError> {
        unsafe { (api.av_log_set_level)(16) }; // AV_LOG_ERROR
        let cpath = CString::new(path).map_err(|_| FfError::Av("path contains NUL".into()))?;

        // --- probe (заголовок) + пакетный индекс (полный скан без декода;
        //     при готовом индексе из sidecar-БД скан пропускается) ---
        let mut scan: *mut ffi::AVFormatContext = core::ptr::null_mut();
        let r = unsafe {
            (api.avformat_open_input)(
                &mut scan,
                cpath.as_ptr(),
                core::ptr::null(),
                core::ptr::null_mut(),
            )
        };
        if r < 0 {
            return Err(averr(api, "opening file", r));
        }
        crate::iostats::OPENS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        let r = unsafe { (api.avformat_find_stream_info)(scan, core::ptr::null_mut()) };
        if r < 0 {
            unsafe { (api.avformat_close_input)(&mut scan) };
            return Err(averr(api, "parsing streams", r));
        }
        let audio_idx = unsafe {
            (api.av_find_best_stream)(
                scan,
                ffi::AVMEDIA_TYPE_AUDIO,
                -1,
                -1,
                core::ptr::null_mut(),
                0,
            )
        };
        let mut video_idx = unsafe {
            (api.av_find_best_stream)(
                scan,
                ffi::AVMEDIA_TYPE_VIDEO,
                -1,
                -1,
                core::ptr::null_mut(),
                0,
            )
        };
        // Обложка (attached picture, напр. mjpeg в ogg/mp3) — не видео:
        // это один пакет без таймлайна, а seek по такому потоку в ogg
        // бьёт av_assert0 (stream_index < ogg->nstreams) и валит процесс.
        if video_idx >= 0 {
            let st = unsafe { &**(*scan).streams.add(video_idx as usize) };
            if (st.disposition as u32 & ffi::AV_DISPOSITION_ATTACHED_PIC) != 0 {
                video_idx = -1;
            }
        }

        let index_prebuilt = prebuilt.is_some();
        let index = match prebuilt {
            Some(p) => {
                // Индекс из БД уже отсортирован при персисте; сортировка
                // дешёвая и защищает от битого блоба.
                let mut idx = PacketIndex { audio_pts: p.audio_pts, video_keyframes: p.video_keyframes };
                idx.audio_pts.sort_unstable();
                idx.video_keyframes.sort_unstable();
                idx
            }
            None => {
                let mut idx =
                    PacketIndex { audio_pts: Vec::new(), video_keyframes: Vec::new() };
                let pkt = unsafe { (api.av_packet_alloc)() };
                loop {
                    let r = unsafe { (api.av_read_frame)(scan, pkt) };
                    if r < 0 {
                        break;
                    }
                    crate::iostats::PACKET_READS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                    let p = unsafe { &*pkt };
                    if p.pts != AV_NOPTS_VALUE {
                        if p.stream_index == audio_idx {
                            idx.audio_pts.push(p.pts);
                        } else if p.stream_index == video_idx
                            && (p.flags & ffi::AV_PKT_FLAG_KEY as c_int) != 0
                        {
                            idx.video_keyframes.push(p.pts);
                        }
                    }
                    unsafe { (api.av_packet_unref)(pkt) };
                }
                unsafe { (api.av_packet_free)(&mut (pkt as *mut _)) };
                idx.audio_pts.sort_unstable();
                idx.video_keyframes.sort_unstable();
                idx
            }
        };

        let sc = unsafe { &*scan };
        // ОДИНОЧНАЯ КАРТИНКА (Р.67): демуксеры картинок называются
        // `image2`, `png_pipe`, `jpeg_pipe`, `webp_pipe`… — общий признак
        // `_pipe` плюс `image2`. Признак берётся у ФОРМАТА, а не у длины:
        // `image2` честно сообщает 0.04 с (один кадр при 25 fps), и по
        // длительности картинка неотличима от очень короткого видео.
        let still = {
            let name = unsafe { std::ffi::CStr::from_ptr((*sc.iformat).name) }
                .to_string_lossy()
                .to_string();
            let by_format = name == "image2" || name == "image2pipe" || name.ends_with("_pipe");
            by_format && audio_idx < 0 && video_idx >= 0
        };
        let duration = if still {
            // «практически бесконечная», как у заглушки: у картинки нет
            // конца материала, и событие любой длины показывает её целиком
            Tick(TICK_HZ * 3600 * 1000)
        } else if sc.duration > 0 {
            Tick(sc.duration * TICK_HZ / ffi::AV_TIME_BASE as i64)
        } else {
            Tick(0)
        };
        // у картинки своей сетки кадров нет: `fps: None` — это и есть
        // «спрашивай любой тик», а 25 fps демуксера — выдумка контейнера
        // АЛЬФА исходного потока (Р.67): спрашиваем у дескриптора формата
        let has_alpha = if video_idx >= 0 {
            let st = unsafe { &**sc.streams.add(video_idx as usize) };
            let par = unsafe { &*st.codecpar };
            let d = unsafe { (api.av_pix_fmt_desc_get)(par.format) };
            !d.is_null() && unsafe { (*d).flags } & ffi::AV_PIX_FMT_FLAG_ALPHA as u64 != 0
        } else {
            false
        };
        let (fps, src_w, src_h, sar) = if video_idx >= 0 {
            let st = unsafe { &**sc.streams.add(video_idx as usize) };
            let par = unsafe { &*st.codecpar };
            let r = st.avg_frame_rate;
            let fps =
                if r.num > 0 && r.den > 0 { Some((r.num as u32, r.den as u32)) } else { Some((30, 1)) };
            // SAR (Р.62): у потока приоритет над кодеком (контейнер знает
            // о ремапе), ноль/отрицательное — не задано → 1:1
            let pick = |a: ffi::AVRational| (a.num > 0 && a.den > 0).then_some((a.num as u32, a.den as u32));
            let sar = pick(st.sample_aspect_ratio).or_else(|| pick(par.sample_aspect_ratio)).unwrap_or((1, 1));
            (fps, par.width.max(0) as u32, par.height.max(0) as u32, sar)
        } else {
            (None, 0, 0, (1, 1))
        };
        let probe = Probe {
            duration,
            has_video: video_idx >= 0,
            has_audio: audio_idx >= 0,
            fps: if still { None } else { fps },
            width: src_w,
            height: src_h,
            sar,
            still,
            has_alpha,
        };
        unsafe { (api.avformat_close_input)(&mut scan) };

        // --- независимые демуксеры (аудио и видео не дерутся за seek) ---
        // Индексы из скан-контекста НЕ переносимы автоматически: у ленивых
        // демуксеров (ogg) новый контекст может видеть меньше потоков.
        // AudioPath/VideoPath перепроверяют индекс на своём контексте.
        let audio = if audio_idx >= 0 {
            Some(AudioPath::open(api, &cpath, audio_idx)?)
        } else {
            None
        };
        let video = if video_idx >= 0 {
            Some(VideoPath::open(api, &cpath, video_idx, still)?)
        } else {
            None
        };
        Ok(Source { api, probe, index, index_prebuilt, audio, video })
    }

    pub fn probe(&self) -> &Probe {
        &self.probe
    }

    pub fn packet_index(&self) -> (&[i64], &[i64]) {
        (&self.index.audio_pts, &self.index.video_keyframes)
    }

    pub fn index_was_prebuilt(&self) -> bool {
        self.index_prebuilt
    }

    pub fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        let v = self.video.as_ref()?;
        let ts = unsafe {
            (self.api.av_rescale_q)(pts.0, tb(1, TICK_HZ as i32), v.time_base)
        } + v.start_time;
        let i = self.index.video_keyframes.partition_point(|&k| k <= ts);
        let k = *self.index.video_keyframes.get(i.checked_sub(1)?)?;
        let rel = k - v.start_time;
        Some(Tick(unsafe {
            (self.api.av_rescale_q)(rel, v.time_base, tb(1, TICK_HZ as i32))
        }))
    }

    pub fn read_audio_burst(&mut self, idx: i64, out: &mut [f32]) -> Result<(), FfError> {
        out.fill(0.0);
        if idx < 0 {
            return Err(FfError::OutOfRange);
        }
        let api = self.api;
        let frames = (out.len() / AUDIO_CHANNELS) as i64;
        let audio_pts = &self.index.audio_pts;
        let a = self.audio.as_mut().ok_or(FfError::OutOfRange)?;
        let t0 = idx * frames;
        let t1 = t0 + frames;

        let need_seek = match a.fifo_start {
            Some(fs) => {
                let fe = fs + (a.fifo.len() / AUDIO_CHANNELS) as i64;
                t0 < fs || t0 > fe + FORWARD_SEEK_THRESHOLD_OUT_SAMPLES
            }
            None => true,
        };
        if need_seek {
            // ближайший индексированный пакет с запасом на разгон декодера
            let target_pts = unsafe {
                (api.av_rescale_q)(t0, tb(1, AUDIO_SAMPLE_RATE as i32), a.time_base)
            } + a.start_time;
            let pos = audio_pts.partition_point(|&pts| pts <= target_pts).saturating_sub(3);
            let seek_pts = audio_pts.get(pos).copied().unwrap_or(a.start_time);
            let r = unsafe {
                (api.av_seek_frame)(a.fmt, a.stream_index, seek_pts, AVSEEK_FLAG_BACKWARD)
            };
            if r < 0 {
                return Err(averr(api, "audio seek", r));
            }
            a.reopen_decoder(api)?;
        }
        a.decode_until(api, t1)?;

        let fs = a.fifo_start.ok_or(FfError::OutOfRange)?;
        if t0 > fs {
            let drop = ((t0 - fs) as usize * AUDIO_CHANNELS).min(a.fifo.len());
            a.fifo.drain(..drop);
            a.fifo_start = Some(fs + (drop / AUDIO_CHANNELS) as i64);
        }
        let fs = a.fifo_start.unwrap();
        if fs > t0 {
            // сигнал начинается позже цели — недостающее остаётся тишиной
            let offset = ((fs - t0) as usize).min(frames as usize);
            let avail = (a.fifo.len() / AUDIO_CHANNELS).min(frames as usize - offset);
            for f in 0..avail {
                for ch in 0..AUDIO_CHANNELS {
                    out[(offset + f) * AUDIO_CHANNELS + ch] = a.fifo[f * AUDIO_CHANNELS + ch];
                }
            }
            return Ok(());
        }
        let have = a.fifo.len() / AUDIO_CHANNELS;
        if have == 0 && a.eof {
            return Err(FfError::OutOfRange);
        }
        for i in 0..(have.min(frames as usize) * AUDIO_CHANNELS) {
            out[i] = a.fifo[i];
        }
        Ok(())
    }

    pub fn read_video_frame(
        &mut self,
        pts: Tick,
        width: u32,
        height: u32,
    ) -> Result<RgbaFrame, FfError> {
        let api = self.api;
        let v = self.video.as_mut().ok_or(FfError::OutOfRange)?;
        let target_ts = v.target_ts(api, pts);
        v.prepare_for(api, target_ts)?;
        let frame = v.frame_for(api, target_ts)?;
        v.to_rgba(api, frame, width, height)
    }

    /// Спан-декод: кадр для КАЖДОГО ключа из `keys` (строго возрастают) за
    /// ОДИН forward-проход декодера. Семантика ровно та же, что у
    /// `read_video_frame` — обе идут через `frame_for` (п.14 р.14: раньше
    /// это были две независимые реализации с РАЗНОЙ семантикой, из-за чего
    /// экспорт и превью показывали разные кадры и ломался инвариант №7).
    pub fn read_video_span(
        &mut self,
        keys: &[Tick],
        width: u32,
        height: u32,
        emit: &mut dyn FnMut(Tick, RgbaFrame),
    ) -> Result<(), FfError> {
        let api = self.api;
        let v = self.video.as_mut().ok_or(FfError::OutOfRange)?;
        if keys.is_empty() {
            return Ok(());
        }
        debug_assert!(keys.windows(2).all(|w| w[0] < w[1]), "span keys must be increasing");
        let first = v.target_ts(api, keys[0]);
        v.prepare_for(api, first)?;
        for k in keys {
            let target = v.target_ts(api, *k);
            let frame = v.frame_for(api, target)?;
            let rgba = v.to_rgba(api, frame, width, height)?;
            emit(*k, rgba);
        }
        Ok(())
    }

    /// Диагностика для тестов производительности (п.14/15): сколько раз
    /// пересоздавался декодер (backward seek) и сколько кадров декодировано.
    pub fn video_debug_stats(&self) -> (u64, u64) {
        self.video.as_ref().map(|v| (v.backward_reopens, v.frames_decoded)).unwrap_or((0, 0))
    }
}

impl Drop for Source {
    fn drop(&mut self) {
        let api = self.api;
        if let Some(a) = &mut self.audio {
            unsafe {
                (api.swr_free)(&mut a.swr);
                (api.avcodec_free_context)(&mut a.dec);
                (api.avformat_close_input)(&mut a.fmt);
            }
        }
        if let Some(v) = &mut self.video {
            unsafe {
                if !v.sws.is_null() {
                    (api.sws_freeContext)(v.sws);
                }
                (api.avcodec_free_context)(&mut v.dec);
                (api.avformat_close_input)(&mut v.fmt);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Аудио
// ---------------------------------------------------------------------------

impl AudioPath {
    fn open(api: &Api, path: &CString, stream_index: c_int) -> Result<AudioPath, FfError> {
        let mut fmt: *mut ffi::AVFormatContext = core::ptr::null_mut();
        let r = unsafe {
            (api.avformat_open_input)(&mut fmt, path.as_ptr(), core::ptr::null(), core::ptr::null_mut())
        };
        if r < 0 {
            return Err(averr(api, "audio: open", r));
        }
        crate::iostats::OPENS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        unsafe { (api.avformat_find_stream_info)(fmt, core::ptr::null_mut()) };
        let stream_index =
            resolve_stream_index(api, fmt, stream_index, ffi::AVMEDIA_TYPE_AUDIO, "audio")?;
        let st = unsafe { &**(*fmt).streams.add(stream_index as usize) };
        let time_base = st.time_base;
        let start_time = if st.start_time == AV_NOPTS_VALUE { 0 } else { st.start_time };
        let (dec, swr) = Self::open_decoder(api, fmt, stream_index)?;
        Ok(AudioPath {
            fmt,
            dec,
            swr,
            stream_index,
            time_base,
            start_time,
            fifo: VecDeque::new(),
            fifo_start: None,
            eof: false,
        })
    }

    fn open_decoder(
        api: &Api,
        fmt: *mut ffi::AVFormatContext,
        stream_index: c_int,
    ) -> Result<(*mut ffi::AVCodecContext, *mut ffi::SwrContext), FfError> {
        let st = unsafe { &**(*fmt).streams.add(stream_index as usize) };
        let par = unsafe { &*st.codecpar };
        let codec = unsafe { (api.avcodec_find_decoder)(par.codec_id) };
        if codec.is_null() {
            return Err(FfError::Av("no audio decoder".into()));
        }
        crate::iostats::OPENS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        let dec = unsafe { (api.avcodec_alloc_context3)(codec) };
        let r = unsafe { (api.avcodec_parameters_to_context)(dec, st.codecpar) };
        if r < 0 {
            return Err(averr(api, "audio codecpar", r));
        }
        unsafe { (*dec).pkt_timebase = st.time_base };
        let r = unsafe { (api.avcodec_open2)(dec, codec, core::ptr::null_mut()) };
        if r < 0 {
            return Err(averr(api, "audio decoder", r));
        }
        // swr → 48k stereo f32 (инвариант №1: наружу только этот формат)
        let mut out_layout: ffi::AVChannelLayout = unsafe { core::mem::zeroed() };
        unsafe { (api.av_channel_layout_default)(&mut out_layout, AUDIO_CHANNELS as c_int) };
        let mut swr: *mut ffi::SwrContext = core::ptr::null_mut();
        let r = unsafe {
            (api.swr_alloc_set_opts2)(
                &mut swr,
                &out_layout,
                ffi::AV_SAMPLE_FMT_FLT,
                AUDIO_SAMPLE_RATE as c_int,
                &(*dec).ch_layout,
                (*dec).sample_fmt,
                (*dec).sample_rate,
                0,
                core::ptr::null_mut(),
            )
        };
        unsafe { (api.av_channel_layout_uninit)(&mut out_layout) };
        if r < 0 {
            return Err(averr(api, "swr", r));
        }
        let r = unsafe { (api.swr_init)(swr) };
        if r < 0 {
            return Err(averr(api, "swr init", r));
        }
        Ok((dec, swr))
    }

    fn reopen_decoder(&mut self, api: &Api) -> Result<(), FfError> {
        unsafe {
            (api.swr_free)(&mut self.swr);
            (api.avcodec_free_context)(&mut self.dec);
        }
        let (dec, swr) = Self::open_decoder(api, self.fmt, self.stream_index)?;
        self.dec = dec;
        self.swr = swr;
        self.fifo.clear();
        self.fifo_start = None;
        self.eof = false;
        Ok(())
    }

    fn push_frame(&mut self, api: &Api, frame: *mut ffi::AVFrame) -> Result<(), FfError> {
        if self.fifo_start.is_none() && !frame.is_null() {
            let f = unsafe { &*frame };
            let pts = if f.best_effort_timestamp != AV_NOPTS_VALUE {
                f.best_effort_timestamp
            } else if f.pts != AV_NOPTS_VALUE {
                f.pts
            } else {
                0
            };
            let rel = pts - self.start_time;
            let out_sample = unsafe {
                (api.av_rescale_q)(rel, self.time_base, tb(1, AUDIO_SAMPLE_RATE as i32))
            };
            self.fifo_start = Some(out_sample);
        }
        let in_samples = if frame.is_null() { 0 } else { unsafe { (*frame).nb_samples } };
        let out_cap = unsafe { (api.swr_get_out_samples)(self.swr, in_samples) }.max(0) as usize;
        if out_cap == 0 {
            return Ok(());
        }
        let mut buf = vec![0f32; out_cap * AUDIO_CHANNELS];
        let mut out_ptr = buf.as_mut_ptr() as *mut u8;
        let (in_ptr, in_n) = if frame.is_null() {
            (core::ptr::null_mut(), 0)
        } else {
            (unsafe { (*frame).extended_data as *mut *const u8 }, in_samples)
        };
        let produced =
            unsafe { (api.swr_convert)(self.swr, &mut out_ptr, out_cap as c_int, in_ptr, in_n) };
        if produced < 0 {
            return Err(averr(api, "swr convert", produced));
        }
        self.fifo.extend(&buf[..produced as usize * AUDIO_CHANNELS]);
        Ok(())
    }

    fn decode_until(&mut self, api: &Api, target_end: i64) -> Result<(), FfError> {
        let pkt = unsafe { (api.av_packet_alloc)() };
        let mut frame = unsafe { (api.av_frame_alloc)() };
        let res = (|| -> Result<(), FfError> {
            loop {
                if let Some(fs) = self.fifo_start
                    && fs + (self.fifo.len() / AUDIO_CHANNELS) as i64 >= target_end
                {
                    return Ok(());
                }
                if self.eof {
                    return Ok(());
                }
                let r = unsafe { (api.av_read_frame)(self.fmt, pkt) };
                if r < 0 {
                    unsafe { (api.avcodec_send_packet)(self.dec, core::ptr::null()) };
                    self.eof = true;
                } else {
                    crate::iostats::PACKET_READS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                    let si = unsafe { (*pkt).stream_index };
                    if si != self.stream_index {
                        unsafe { (api.av_packet_unref)(pkt) };
                        continue;
                    }
                    let r = unsafe { (api.avcodec_send_packet)(self.dec, pkt) };
                    unsafe { (api.av_packet_unref)(pkt) };
                    if r < 0 && r != AVERROR_EAGAIN {
                        return Err(averr(api, "audio send", r));
                    }
                }
                loop {
                    let r = unsafe { (api.avcodec_receive_frame)(self.dec, frame) };
                    if r == AVERROR_EAGAIN {
                        break;
                    }
                    if r == AVERROR_EOF_ {
                        self.push_frame(api, core::ptr::null_mut())?; // дренаж swr
                        break;
                    }
                    if r < 0 {
                        return Err(averr(api, "audio receive", r));
                    }
                    self.push_frame(api, frame)?;
                    unsafe { (api.av_frame_unref)(frame) };
                }
            }
        })();
        unsafe {
            (api.av_frame_free)(&mut frame);
            (api.av_packet_free)(&mut (pkt as *mut _));
        }
        res
    }
}

// ---------------------------------------------------------------------------
// Видео
// ---------------------------------------------------------------------------

impl VideoPath {
    fn open(
        api: &Api,
        path: &CString,
        stream_index: c_int,
        still: bool,
    ) -> Result<VideoPath, FfError> {
        let mut fmt: *mut ffi::AVFormatContext = core::ptr::null_mut();
        let r = unsafe {
            (api.avformat_open_input)(&mut fmt, path.as_ptr(), core::ptr::null(), core::ptr::null_mut())
        };
        if r < 0 {
            return Err(averr(api, "video: open", r));
        }
        crate::iostats::OPENS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        unsafe { (api.avformat_find_stream_info)(fmt, core::ptr::null_mut()) };
        let stream_index =
            resolve_stream_index(api, fmt, stream_index, ffi::AVMEDIA_TYPE_VIDEO, "video")?;
        let st = unsafe { &**(*fmt).streams.add(stream_index as usize) };
        let time_base = st.time_base;
        let start_time = if st.start_time == AV_NOPTS_VALUE { 0 } else { st.start_time };
        let codec_id = unsafe { (*st.codecpar).codec_id };
        let dec = Self::open_decoder(api, fmt, stream_index, codec_id)?;
        Ok(VideoPath {
            fmt,
            dec,
            stream_index,
            time_base,
            start_time,
            codec_id,
            last_pts: None,
            held: core::ptr::null_mut(),
            held_pts: i64::MIN,
            pending: core::ptr::null_mut(),
            pending_pts: i64::MIN,
            still,
            sws: core::ptr::null_mut(),
            sws_key: (0, 0, 0, 0, 0),
            eof: false,
            backward_reopens: 0,
            frames_decoded: 0,
        })
    }

    fn open_decoder(
        api: &Api,
        fmt: *mut ffi::AVFormatContext,
        stream_index: c_int,
        codec_id: ffi::AVCodecID,
    ) -> Result<*mut ffi::AVCodecContext, FfError> {
        let st = unsafe { &**(*fmt).streams.add(stream_index as usize) };
        let codec = unsafe { (api.avcodec_find_decoder)(codec_id) };
        if codec.is_null() {
            return Err(FfError::Av("no video decoder".into()));
        }
        crate::iostats::OPENS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
        let dec = unsafe { (api.avcodec_alloc_context3)(codec) };
        let r = unsafe { (api.avcodec_parameters_to_context)(dec, st.codecpar) };
        if r < 0 {
            return Err(averr(api, "video codecpar", r));
        }
        unsafe { (*dec).pkt_timebase = st.time_base };
        // threads=auto для ВСЕХ кодеков: дефолт libavcodec — ОДИН поток,
        // и 4К декодился в одну нитку (п.14: «VLC легко, у нас минута»).
        // Старый запрет MT для VP9/AV1 касался ОБЩЕГО декодера с shared
        // seek (avcpp); у нас backward seek всегда пересоздаёт декодер,
        // а битовая точность MT-декода гарантирована спецификацией кодека
        // (проверяется t10_vp9 и t10_h264_backward_seek_is_deterministic).
        let mut opts: *mut ffi::AVDictionary = core::ptr::null_mut();
        let k = CString::new("threads").unwrap();
        let v = CString::new("auto").unwrap();
        unsafe { (api.av_dict_set)(&mut opts, k.as_ptr(), v.as_ptr(), 0) };
        let r = unsafe { (api.avcodec_open2)(dec, codec, &mut opts) };
        unsafe { (api.av_dict_free)(&mut opts) };
        if r < 0 {
            return Err(averr(api, "video decoder", r));
        }
        Ok(dec)
    }

    /// Backward seek: контейнерный seek + ПОЛНОЕ пересоздание декодера
    /// (безопасно для MT H.264).
    fn seek_and_reopen(&mut self, api: &Api, target_ts: i64) -> Result<(), FfError> {
        let r = unsafe {
            (api.av_seek_frame)(
                self.fmt,
                self.stream_index,
                target_ts,
                AVSEEK_FLAG_BACKWARD,
            )
        };
        if r < 0 {
            return Err(averr(api, "video seek", r));
        }
        unsafe { (api.avcodec_free_context)(&mut self.dec) };
        self.dec = Self::open_decoder(api, self.fmt, self.stream_index, self.codec_id)?;
        self.last_pts = None;
        self.drop_cached_frames(api);
        self.eof = false;
        self.backward_reopens += 1;
        Ok(())
    }

    fn drop_cached_frames(&mut self, api: &Api) {
        if !self.held.is_null() {
            unsafe { (api.av_frame_free)(&mut self.held) };
        }
        if !self.pending.is_null() {
            unsafe { (api.av_frame_free)(&mut self.pending) };
        }
        self.held_pts = i64::MIN;
        self.pending_pts = i64::MIN;
    }

    /// Следующий декодированный кадр (владение — у вызывающего:
    /// av_frame_free). None = EOF потока. Обновляет last_pts.
    fn decode_next_frame(&mut self, api: &Api) -> Result<Option<*mut ffi::AVFrame>, FfError> {
        let pkt = unsafe { (api.av_packet_alloc)() };
        let frame = unsafe { (api.av_frame_alloc)() };
        let res = (|| -> Result<Option<*mut ffi::AVFrame>, FfError> {
            loop {
                // сперва пробуем забрать готовый кадр
                let r = unsafe { (api.avcodec_receive_frame)(self.dec, frame) };
                if r >= 0 {
                    let f = unsafe { &*frame };
                    let pts = if f.best_effort_timestamp != AV_NOPTS_VALUE {
                        f.best_effort_timestamp
                    } else {
                        f.pts
                    };
                    self.last_pts = Some(pts);
                    self.frames_decoded += 1;
                    return Ok(Some(frame));
                }
                if r == AVERROR_EOF_ {
                    self.eof = true;
                    return Ok(None);
                }
                if r != AVERROR_EAGAIN {
                    return Err(averr(api, "video receive", r));
                }
                if self.eof {
                    return Ok(None);
                }
                // декодер голоден — кормим пакетами
                let r = unsafe { (api.av_read_frame)(self.fmt, pkt) };
                if r < 0 {
                    unsafe { (api.avcodec_send_packet)(self.dec, core::ptr::null()) };
                    self.eof = true;
                    continue;
                }
                crate::iostats::PACKET_READS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
                let si = unsafe { (*pkt).stream_index };
                if si != self.stream_index {
                    unsafe { (api.av_packet_unref)(pkt) };
                    continue;
                }
                let r = unsafe { (api.avcodec_send_packet)(self.dec, pkt) };
                unsafe { (api.av_packet_unref)(pkt) };
                if r < 0 && r != AVERROR_EAGAIN {
                    return Err(averr(api, "video send", r));
                }
            }
        })();
        unsafe { (api.av_packet_free)(&mut (pkt as *mut _)) };
        if !matches!(res, Ok(Some(_))) {
            let mut f = frame;
            unsafe { (api.av_frame_free)(&mut f) };
        }
        res
    }

    /// Кадр-ответ для цели: **наибольший pts <= target_ts** — ВСЕГДА, а не
    /// «как повезёт с состоянием декодера» (п.14 р.14).
    ///
    /// Раньше это работало по двум разным правилам: после seek копился
    /// `best` и получался floor, а на forward-пути предыдущий кадр был уже
    /// отдан и освобождён, срабатывала ветка «best пуст → верни то, что
    /// декодировалось» — то есть кадр СПРАВА от цели. При fps проекта ≠ fps
    /// источника правила чередовались, и кадры в экспорте шли
    /// `f0, f1, f0, f2, f1, f3…` — пользователь видел «откаты назад».
    ///
    /// Теперь поток держит два кадра: `held` (ответ, floor) и `pending`
    /// (первый декодированный ПОСЛЕ цели — он уже стоил декода). Владение
    /// остаётся у потока: вызывающий только конвертирует кадр в RGBA.
    /// Возвращается заимствованный указатель, освобождать его нельзя.
    /// Тик проекта → ts источника. Округление ВНИЗ: `av_rescale_q`
    /// округляет к ближайшему, и цель могла перескочить границу кадра
    /// источника (второй источник джаддера, п.14 р.14).
    fn target_ts(&self, api: &Api, pts: Tick) -> i64 {
        let _ = api;
        // floor вручную: ts = pts/TICK_HZ секунд / (num/den) единиц
        let num = self.time_base.num as i128;
        let den = self.time_base.den as i128;
        let ts = if num > 0 {
            ((pts.0 as i128) * den).div_euclid(TICK_HZ as i128 * num) as i64
        } else {
            unsafe { (api.av_rescale_q)(pts.0, tb(1, TICK_HZ as i32), self.time_base) }
        };
        ts + self.start_time
    }

    /// Нужен ли seek перед запросом цели: назад или слишком далеко вперёд.
    fn prepare_for(&mut self, api: &Api, target_ts: i64) -> Result<(), FfError> {
        // у картинки один кадр: ни seek, ни переоткрытия (Р.67)
        if self.still {
            return Ok(());
        }
        // ответ уже удержан — ни seek, ни декода не требуется
        if !self.held.is_null() && self.held_pts <= target_ts && !self.pending.is_null() {
            return Ok(());
        }
        let forward_ok = match self.last_pts {
            Some(last) if target_ts >= last => {
                let ahead = unsafe {
                    (api.av_rescale_q)(target_ts - last, self.time_base, tb(1, TICK_HZ as i32))
                };
                ahead < VIDEO_FORWARD_DECODE_MAX_TICKS
            }
            _ => self.held_pts != i64::MIN && target_ts >= self.held_pts,
        };
        if !forward_ok {
            self.seek_and_reopen(api, target_ts)?;
        }
        Ok(())
    }

    fn frame_for(&mut self, api: &Api, target_ts: i64) -> Result<*mut ffi::AVFrame, FfError> {
        // отложенный кадр дозрел до ответа
        if !self.pending.is_null() && self.pending_pts <= target_ts {
            if !self.held.is_null() {
                unsafe { (api.av_frame_free)(&mut self.held) };
            }
            self.held = self.pending;
            self.held_pts = self.pending_pts;
            self.pending = core::ptr::null_mut();
            self.pending_pts = i64::MIN;
        }
        // ответ уже есть: цель между held и pending — отдаём held
        if !self.held.is_null() && self.held_pts <= target_ts && !self.pending.is_null() {
            return Ok(self.held);
        }
        loop {
            match self.decode_next_frame(api) {
                Ok(Some(frame)) => {
                    let f = unsafe { &*frame };
                    let pts = if f.best_effort_timestamp != AV_NOPTS_VALUE {
                        f.best_effort_timestamp
                    } else {
                        f.pts
                    };
                    if pts <= target_ts {
                        if !self.held.is_null() {
                            unsafe { (api.av_frame_free)(&mut self.held) };
                        }
                        self.held = frame;
                        self.held_pts = pts;
                        continue;
                    }
                    // первый кадр ПОСЛЕ цели закрывает запрос
                    if !self.pending.is_null() {
                        unsafe { (api.av_frame_free)(&mut self.pending) };
                    }
                    self.pending = frame;
                    self.pending_pts = pts;
                    if self.held.is_null() {
                        // раньше цели кадров нет вовсе (запрос до первого
                        // кадра файла) — честно отдаём первый существующий
                        self.held = self.pending;
                        self.held_pts = self.pending_pts;
                        self.pending = core::ptr::null_mut();
                        self.pending_pts = i64::MIN;
                    }
                    return Ok(self.held);
                }
                Ok(None) => {
                    return if self.held.is_null() {
                        Err(FfError::OutOfRange)
                    } else {
                        Ok(self.held)
                    };
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn to_rgba(
        &mut self,
        api: &Api,
        frame: *mut ffi::AVFrame,
        width: u32,
        height: u32,
    ) -> Result<RgbaFrame, FfError> {
        let f = unsafe { &*frame };
        let (dst_w, dst_h) = (
            (width.max(2) as c_int) & !1,
            (height.max(2) as c_int) & !1,
        );
        let key = (dst_w, dst_h, f.width, f.height, f.format);
        if self.sws.is_null() || self.sws_key != key {
            if !self.sws.is_null() {
                unsafe { (api.sws_freeContext)(self.sws) };
            }
            self.sws = unsafe {
                (api.sws_getContext)(
                    f.width,
                    f.height,
                    f.format as ffi::AVPixelFormat,
                    dst_w,
                    dst_h,
                    ffi::AV_PIX_FMT_RGBA,
                    ffi::SWS_FAST_BILINEAR as c_int,
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                    core::ptr::null(),
                )
            };
            if self.sws.is_null() {
                return Err(FfError::Av("sws_getContext failed".into()));
            }
            self.sws_key = key;
        }
        let row = dst_w as usize * 4;
        let mut rgba = vec![0u8; row * dst_h as usize];
        let dst_data: [*mut u8; 4] = [rgba.as_mut_ptr(), core::ptr::null_mut(), core::ptr::null_mut(), core::ptr::null_mut()];
        let dst_linesize: [c_int; 4] = [row as c_int, 0, 0, 0];
        let r = unsafe {
            (api.sws_scale)(
                self.sws,
                f.data.as_ptr() as *const *const u8,
                f.linesize.as_ptr(),
                0,
                f.height,
                dst_data.as_ptr(),
                dst_linesize.as_ptr(),
            )
        };
        if r < 0 {
            return Err(averr(api, "sws_scale", r));
        }
        let pts = if f.best_effort_timestamp != AV_NOPTS_VALUE {
            f.best_effort_timestamp
        } else {
            f.pts
        };
        let rel = pts - self.start_time;
        let pts_tick =
            Tick(unsafe { (api.av_rescale_q)(rel, self.time_base, tb(1, TICK_HZ as i32)) });
        Ok(RgbaFrame { pts: pts_tick, width: dst_w as u32, height: dst_h as u32, rgba })
    }
}
