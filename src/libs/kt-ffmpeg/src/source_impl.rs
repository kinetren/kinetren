// Decoder implementation. INCLUDED (include!) into each version module,
// therefore `ffi` and `Api` here are the types of the specific major version of FFmpeg.
//
// The logic is a port of the former rsmpeg variant (docs/porting-notes.md):
// two independent demuxers per file, a packet index on open,
// sample-accurate audio seek, threads=1 for VP9/AV1, recreation of the
// video decoder on backward seek, swscale into RGBA.

use crate::{FfError, Probe, RgbaFrame};
use kt_time::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, TICK_HZ, Tick};
use std::collections::VecDeque;
use std::ffi::CString;

const FORWARD_SEEK_THRESHOLD_OUT_SAMPLES: i64 = 3 * AUDIO_SAMPLE_RATE as i64;
const VIDEO_FORWARD_DECODE_MAX_TICKS: i64 = 4 * TICK_HZ;
const AVERROR_EAGAIN: c_int = -11; // EAGAIN
const AVERROR_EOF_: c_int = -0x20464F45; // FFERRTAG('E','O','F',' ')
/// bindgen does not pick up these #defines (complex expressions / casts);
/// the values are stable across all FFmpeg majors.
const AV_NOPTS_VALUE: i64 = i64::MIN;
const AVSEEK_FLAG_BACKWARD: c_int = 1;

fn tb(num: i32, den: i32) -> ffi::AVRational {
    ffi::AVRational { num, den }
}

/// Checks that `wanted` (the index from the scan context) exists and has the
/// required type in THIS context; otherwise it searches again. For lazy demuxers
/// (ogg) the number of streams in a new context may differ — a foreign index
/// without a check led to av_assert0 inside libavformat (process abort).
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

/// Packet index taken by one scan on open (without decode).
struct PacketIndex {
    /// (pkt_pts) of audio packets in the audio stream’s timebase.
    audio_pts: Vec<i64>,
    /// video keyframe pts (video stream timebase), sorted.
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
    /// Absolute number of the first frame in the fifo (48k). None before the anchor.
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
    /// The response frame to the last request: the greatest pts <= the target. Ownership
    /// belongs to the stream — the caller only converts it, does not free it.
    held: *mut ffi::AVFrame,
    held_pts: i64,
    /// The first decoded frame AFTER the target: it has already cost a decode, and
    /// it cannot be discarded — otherwise the next request would go into a seek
    /// (item 14, round 14: that is where the seek storm came from when the project fps was higher than the
    /// source fps).
    pending: *mut ffi::AVFrame,
    pending_pts: i64,
    /// SINGLE IMAGE (R.67): the file holds one frame, and a seek over it
    /// is not needed by the image demuxer — but attempting it left the
    /// stream without a single frame (“out of range” at any tick, that very
    /// eternal “Loading…”).
    still: bool,
    sws: *mut ffi::SwsContext,
    sws_key: (c_int, c_int, c_int, c_int, c_int),
    eof: bool,
    /// Diagnostics: decoder recreations (backward) and decoded frames.
    backward_reopens: u64,
    frames_decoded: u64,
}

pub struct Source {
    api: &'static Api,
    probe: Probe,
    index: PacketIndex,
    /// Index came ready from the sidecar DB — the scan pass was not performed.
    index_prebuilt: bool,
    audio: Option<AudioPath>,
    video: Option<VideoPath>,
}

// FFmpeg pointers live inside and never leave the struct; an instance is
// owned by exactly one MediaWorker thread (invariant 5).
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

        // --- probe (header) + packet index (full scan without decode;
        //     when the index is ready from the sidecar DB the scan is skipped) ---
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
        // Cover art (attached picture, e.g. mjpeg in ogg/mp3) — not video:
        // it is a single packet without a timeline, and a seek over such a stream
        // in ogg hits av_assert0 (stream_index < ogg->nstreams) and crashes the process.
        if video_idx >= 0 {
            let st = unsafe { &**(*scan).streams.add(video_idx as usize) };
            if (st.disposition as u32 & ffi::AV_DISPOSITION_ATTACHED_PIC) != 0 {
                video_idx = -1;
            }
        }

        let index_prebuilt = prebuilt.is_some();
        let index = match prebuilt {
            Some(p) => {
                // The index from the DB is already sorted on persist; sorting is
                // cheap and protects against a corrupted blob.
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
        // SINGLE IMAGE (R.67): image demuxers are named
        // `image2`, `png_pipe`, `jpeg_pipe`, `webp_pipe`… — the common sign
        // `_pipe` plus `image2`. The sign is taken from the FORMAT, not from the length:
        // `image2` honestly reports 0.04 s (one frame at 25 fps), and by
        // duration an image is indistinguishable from a very short video.
        let still = {
            let name = unsafe { std::ffi::CStr::from_ptr((*sc.iformat).name) }
                .to_string_lossy()
                .to_string();
            let by_format = name == "image2" || name == "image2pipe" || name.ends_with("_pipe");
            by_format && audio_idx < 0 && video_idx >= 0
        };
        let duration = if still {
            // “practically infinite”, like the stub: an image has no
            // end of material, and an event of any length shows it in full
            Tick(TICK_HZ * 3600 * 1000)
        } else if sc.duration > 0 {
            Tick(sc.duration * TICK_HZ / ffi::AV_TIME_BASE as i64)
        } else {
            Tick(0)
        };
        // an image has no frame grid of its own: `fps: None` — that is
        // “ask for any tick”, and the demuxer’s 25 fps is a container fiction
        // ALPHA of the source stream (R.67): we ask the pixel-format descriptor
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
            // SAR (R.62): the stream takes priority over the codec (the container knows
            // about the remap); zero/negative — not set → 1:1
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

        // --- independent demuxers (audio and video do not fight over seek) ---
        // Indices from the scan context are NOT transferable automatically: for lazy
        // demuxers (ogg) the new context may see fewer streams.
        // AudioPath/VideoPath re-check the index against their own context.
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
            // nearest indexed packet with a margin for the decoder to spin up
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
            // the signal starts later than the target — the missing part stays silence
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

    /// Span decode: a frame for EACH key from `keys` (strictly increasing) in
    /// ONE forward pass of the decoder. The semantics are exactly the same as
    /// `read_video_frame` — both go through `frame_for` (item 14, round 14: previously
    /// these were two independent implementations with DIFFERENT semantics, so the
    /// export and preview showed different frames and invariant 7 was broken).
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

    /// Diagnostics for performance tests (item 14/15): how many times the decoder
    /// was recreated (backward seek) and how many frames were decoded.
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
// Audio
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
        // swr → 48k stereo f32 (invariant 1: only this format goes out)
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
                        self.push_frame(api, core::ptr::null_mut())?; // swr drain
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
// Video
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
        // threads=auto for ALL codecs: the libavcodec default is ONE thread,
        // and 4K was decoded on a single thread (item 14: “VLC is easy, we take a minute”).
        // The old MT ban for VP9/AV1 concerned the SHARED decoder with shared
        // seek (avcpp); ours always recreates the decoder on backward seek,
        // and the bit-exactness of MT decoding is guaranteed by the codec spec
        // (checked by t10_vp9 and t10_h264_backward_seek_is_deterministic).
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

    /// Backward seek: container seek + FULL recreation of the decoder
    /// (safe for MT H.264).
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

    /// Next decoded frame (ownership belongs to the caller:
    /// av_frame_free). None = end of stream. Updates last_pts.
    fn decode_next_frame(&mut self, api: &Api) -> Result<Option<*mut ffi::AVFrame>, FfError> {
        let pkt = unsafe { (api.av_packet_alloc)() };
        let frame = unsafe { (api.av_frame_alloc)() };
        let res = (|| -> Result<Option<*mut ffi::AVFrame>, FfError> {
            loop {
                // first try to take a ready frame
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
                // the decoder is starved — feed it packets
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

    /// Answer frame for the target: **the greatest pts <= target_ts** — ALWAYS, not
    /// “whatever luck the decoder state brings” (item 14, round 14).
    ///
    /// Previously this worked by two different rules: after a seek a `best` accumulated
    /// and became a floor, while on the forward path the previous frame had already been
    /// handed out and freed, so the branch “best is empty → return what was decoded” fired
    /// — that is, a frame to the RIGHT of the target. When the project fps ≠ the source
    /// fps the rules alternated, and the frames in the export went
    /// `f0, f1, f0, f2, f1, f3…` — the user saw “backward jumps”.
    ///
    /// Now the stream holds two frames: `held` (the answer, floor) and `pending`
    /// (the first decoded frame AFTER the target — it has already cost a decode). Ownership
    /// stays with the stream: the caller only converts the frame to RGBA.
    /// A borrowed pointer is returned; it must not be freed.
    /// Project tick → source ts. Rounding DOWN: `av_rescale_q`
    /// rounds to the nearest, and the target could jump over the boundary of the frame of the
    /// source (the second source of judder, item 14, round 14).
    fn target_ts(&self, api: &Api, pts: Tick) -> i64 {
        let _ = api;
        // floor by hand: ts = pts/TICK_HZ seconds / (num/den) units
        let num = self.time_base.num as i128;
        let den = self.time_base.den as i128;
        let ts = if num > 0 {
            ((pts.0 as i128) * den).div_euclid(TICK_HZ as i128 * num) as i64
        } else {
            unsafe { (api.av_rescale_q)(pts.0, tb(1, TICK_HZ as i32), self.time_base) }
        };
        ts + self.start_time
    }

    /// Is a seek needed before the target request: backward or too far forward.
    fn prepare_for(&mut self, api: &Api, target_ts: i64) -> Result<(), FfError> {
        // the image has one frame: no seek, no reopening (R.67)
        if self.still {
            return Ok(());
        }
        // the answer is already held — no seek or decode required
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
        // the deferred frame has matured into the answer
        if !self.pending.is_null() && self.pending_pts <= target_ts {
            if !self.held.is_null() {
                unsafe { (api.av_frame_free)(&mut self.held) };
            }
            self.held = self.pending;
            self.held_pts = self.pending_pts;
            self.pending = core::ptr::null_mut();
            self.pending_pts = i64::MIN;
        }
        // the answer is already there: the target is between held and pending — return held
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
                    // the first frame AFTER the target closes the request
                    if !self.pending.is_null() {
                        unsafe { (api.av_frame_free)(&mut self.pending) };
                    }
                    self.pending = frame;
                    self.pending_pts = pts;
                    if self.held.is_null() {
                        // there are no frames before the target at all (a request before the first
                        // frame of the file) — honestly return the first existing one
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
