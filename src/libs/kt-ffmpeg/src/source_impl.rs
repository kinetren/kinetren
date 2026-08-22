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
    sws: *mut ffi::SwsContext,
    sws_key: (c_int, c_int, c_int, c_int, c_int),
    eof: bool,
}

pub struct Source {
    api: &'static Api,
    probe: Probe,
    index: PacketIndex,
    audio: Option<AudioPath>,
    video: Option<VideoPath>,
}

// Указатели FFmpeg живут внутри и не покидают структуру; экземпляром
// владеет ровно один MediaWorker-поток (инвариант №5).
unsafe impl Send for Source {}

impl Source {
    pub fn open(api: &'static Api, path: &str) -> Result<Source, FfError> {
        unsafe { (api.av_log_set_level)(16) }; // AV_LOG_ERROR
        let cpath = CString::new(path).map_err(|_| FfError::Av("путь содержит NUL".into()))?;

        // --- скан-проход: probe + пакетный индекс (без декода) ---
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
            return Err(averr(api, "открытие файла", r));
        }
        let r = unsafe { (api.avformat_find_stream_info)(scan, core::ptr::null_mut()) };
        if r < 0 {
            unsafe { (api.avformat_close_input)(&mut scan) };
            return Err(averr(api, "разбор потоков", r));
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
        let video_idx = unsafe {
            (api.av_find_best_stream)(
                scan,
                ffi::AVMEDIA_TYPE_VIDEO,
                -1,
                -1,
                core::ptr::null_mut(),
                0,
            )
        };

        let mut index = PacketIndex { audio_pts: Vec::new(), video_keyframes: Vec::new() };
        let pkt = unsafe { (api.av_packet_alloc)() };
        loop {
            let r = unsafe { (api.av_read_frame)(scan, pkt) };
            if r < 0 {
                break;
            }
            let p = unsafe { &*pkt };
            if p.pts != AV_NOPTS_VALUE {
                if p.stream_index == audio_idx {
                    index.audio_pts.push(p.pts);
                } else if p.stream_index == video_idx
                    && (p.flags & ffi::AV_PKT_FLAG_KEY as c_int) != 0
                {
                    index.video_keyframes.push(p.pts);
                }
            }
            unsafe { (api.av_packet_unref)(pkt) };
        }
        unsafe { (api.av_packet_free)(&mut (pkt as *mut _)) };
        index.audio_pts.sort_unstable();
        index.video_keyframes.sort_unstable();

        let sc = unsafe { &*scan };
        let duration = if sc.duration > 0 {
            Tick(sc.duration * TICK_HZ / ffi::AV_TIME_BASE as i64)
        } else {
            Tick(0)
        };
        let (fps, src_w, src_h) = if video_idx >= 0 {
            let st = unsafe { &**sc.streams.add(video_idx as usize) };
            let par = unsafe { &*st.codecpar };
            let r = st.avg_frame_rate;
            let fps =
                if r.num > 0 && r.den > 0 { Some((r.num as u32, r.den as u32)) } else { Some((30, 1)) };
            (fps, par.width.max(0) as u32, par.height.max(0) as u32)
        } else {
            (None, 0, 0)
        };
        let probe = Probe {
            duration,
            has_video: video_idx >= 0,
            has_audio: audio_idx >= 0,
            fps,
            width: src_w,
            height: src_h,
        };
        unsafe { (api.avformat_close_input)(&mut scan) };

        // --- независимые демуксеры (аудио и видео не дерутся за seek) ---
        let audio = if audio_idx >= 0 {
            Some(AudioPath::open(api, &cpath, audio_idx)?)
        } else {
            None
        };
        let video = if video_idx >= 0 {
            Some(VideoPath::open(api, &cpath, video_idx)?)
        } else {
            None
        };
        Ok(Source { api, probe, index, audio, video })
    }

    pub fn probe(&self) -> &Probe {
        &self.probe
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
                return Err(averr(api, "аудио seek", r));
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
        let target_ts =
            unsafe { (api.av_rescale_q)(pts.0, tb(1, TICK_HZ as i32), v.time_base) } + v.start_time;

        let forward_ok = match v.last_pts {
            Some(last) if target_ts > last => {
                let ahead = unsafe {
                    (api.av_rescale_q)(target_ts - last, v.time_base, tb(1, TICK_HZ as i32))
                };
                ahead < VIDEO_FORWARD_DECODE_MAX_TICKS
            }
            _ => false,
        };
        if !forward_ok {
            v.seek_and_reopen(api, target_ts)?;
        }
        let frame = v.decode_to(api, target_ts)?;
        let out = v.to_rgba(api, frame, width, height);
        unsafe { (api.av_frame_free)(&mut (frame as *mut _)) };
        out
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
            return Err(averr(api, "аудио: открытие", r));
        }
        unsafe { (api.avformat_find_stream_info)(fmt, core::ptr::null_mut()) };
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
            return Err(FfError::Av("нет аудио-декодера".into()));
        }
        let dec = unsafe { (api.avcodec_alloc_context3)(codec) };
        let r = unsafe { (api.avcodec_parameters_to_context)(dec, st.codecpar) };
        if r < 0 {
            return Err(averr(api, "аудио codecpar", r));
        }
        unsafe { (*dec).pkt_timebase = st.time_base };
        let r = unsafe { (api.avcodec_open2)(dec, codec, core::ptr::null_mut()) };
        if r < 0 {
            return Err(averr(api, "аудио декодер", r));
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
                    let si = unsafe { (*pkt).stream_index };
                    if si != self.stream_index {
                        unsafe { (api.av_packet_unref)(pkt) };
                        continue;
                    }
                    let r = unsafe { (api.avcodec_send_packet)(self.dec, pkt) };
                    unsafe { (api.av_packet_unref)(pkt) };
                    if r < 0 && r != AVERROR_EAGAIN {
                        return Err(averr(api, "аудио send", r));
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
                        return Err(averr(api, "аудио receive", r));
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
    fn open(api: &Api, path: &CString, stream_index: c_int) -> Result<VideoPath, FfError> {
        let mut fmt: *mut ffi::AVFormatContext = core::ptr::null_mut();
        let r = unsafe {
            (api.avformat_open_input)(&mut fmt, path.as_ptr(), core::ptr::null(), core::ptr::null_mut())
        };
        if r < 0 {
            return Err(averr(api, "видео: открытие", r));
        }
        unsafe { (api.avformat_find_stream_info)(fmt, core::ptr::null_mut()) };
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
            sws: core::ptr::null_mut(),
            sws_key: (0, 0, 0, 0, 0),
            eof: false,
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
            return Err(FfError::Av("нет видео-декодера".into()));
        }
        let dec = unsafe { (api.avcodec_alloc_context3)(codec) };
        let r = unsafe { (api.avcodec_parameters_to_context)(dec, st.codecpar) };
        if r < 0 {
            return Err(averr(api, "видео codecpar", r));
        }
        unsafe { (*dec).pkt_timebase = st.time_base };
        // VP9/AV1: threads=1 — многопоточный декод этих кодеков нестабилен
        // при частых seek (проверено старым движком)
        let mut opts: *mut ffi::AVDictionary = core::ptr::null_mut();
        if codec_id == ffi::AV_CODEC_ID_VP9 || codec_id == ffi::AV_CODEC_ID_AV1 {
            let k = CString::new("threads").unwrap();
            let v = CString::new("1").unwrap();
            unsafe { (api.av_dict_set)(&mut opts, k.as_ptr(), v.as_ptr(), 0) };
        }
        let r = unsafe { (api.avcodec_open2)(dec, codec, &mut opts) };
        unsafe { (api.av_dict_free)(&mut opts) };
        if r < 0 {
            return Err(averr(api, "видео декодер", r));
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
            return Err(averr(api, "видео seek", r));
        }
        unsafe { (api.avcodec_free_context)(&mut self.dec) };
        self.dec = Self::open_decoder(api, self.fmt, self.stream_index, self.codec_id)?;
        self.last_pts = None;
        self.eof = false;
        Ok(())
    }

    /// Кадр с наибольшим pts <= target_ts (или первый после, если точного нет).
    fn decode_to(&mut self, api: &Api, target_ts: i64) -> Result<*mut ffi::AVFrame, FfError> {
        let pkt = unsafe { (api.av_packet_alloc)() };
        let mut best: *mut ffi::AVFrame = core::ptr::null_mut();
        let mut scratch = unsafe { (api.av_frame_alloc)() };
        let res = (|| -> Result<*mut ffi::AVFrame, FfError> {
            loop {
                if self.eof && best.is_null() {
                    return Err(FfError::OutOfRange);
                }
                if self.eof {
                    return Ok(best);
                }
                let r = unsafe { (api.av_read_frame)(self.fmt, pkt) };
                if r < 0 {
                    unsafe { (api.avcodec_send_packet)(self.dec, core::ptr::null()) };
                    self.eof = true;
                } else {
                    let si = unsafe { (*pkt).stream_index };
                    if si != self.stream_index {
                        unsafe { (api.av_packet_unref)(pkt) };
                        continue;
                    }
                    let r = unsafe { (api.avcodec_send_packet)(self.dec, pkt) };
                    unsafe { (api.av_packet_unref)(pkt) };
                    if r < 0 && r != AVERROR_EAGAIN {
                        return Err(averr(api, "видео send", r));
                    }
                }
                loop {
                    let r = unsafe { (api.avcodec_receive_frame)(self.dec, scratch) };
                    if r == AVERROR_EAGAIN {
                        break;
                    }
                    if r == AVERROR_EOF_ {
                        self.eof = true;
                        break;
                    }
                    if r < 0 {
                        return Err(averr(api, "видео receive", r));
                    }
                    let f = unsafe { &*scratch };
                    let pts = if f.best_effort_timestamp != AV_NOPTS_VALUE {
                        f.best_effort_timestamp
                    } else {
                        f.pts
                    };
                    self.last_pts = Some(pts);
                    let done = pts >= target_ts;
                    if pts <= target_ts || best.is_null() {
                        // забираем кадр себе (перенос владения буферами)
                        if !best.is_null() {
                            unsafe { (api.av_frame_free)(&mut best) };
                        }
                        best = scratch;
                        scratch = unsafe { (api.av_frame_alloc)() };
                    } else {
                        unsafe { (api.av_frame_unref)(scratch) };
                    }
                    if done && !best.is_null() {
                        return Ok(best);
                    }
                }
            }
        })();
        unsafe {
            (api.av_frame_free)(&mut scratch);
            (api.av_packet_free)(&mut (pkt as *mut _));
        }
        match res {
            Ok(f) if !f.is_null() => Ok(f),
            Ok(_) => Err(FfError::OutOfRange),
            Err(e) => {
                if !best.is_null() {
                    unsafe { (api.av_frame_free)(&mut best) };
                }
                Err(e)
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
