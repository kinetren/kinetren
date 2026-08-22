//! FfmpegSource — rsmpeg-бэкенд `MediaSource`.
//!
//! Наследует проверенные решения старого движка (docs/porting-notes.md):
//! два НЕЗАВИСИМЫХ демуксера на файл (аудио и видео не дерутся за seek),
//! `threads=1` для VP9/AV1, полное пересоздание декодера при backward seek
//! (покрывает и MT H.264). Исправляет дефект j: пакетный индекс строится
//! одним сканом при открытии (без декода), аудио-seek — sample-accurate:
//! байтовый seek к пакету перед целью + декод вперёд с якорем по pts
//! первого декодированного кадра.
//!
//! Наружу — только 48 kHz stereo f32 (инвариант №1): swr стоит внутри.

use crate::{AudioBurst, MediaError, MediaInfo, MediaSource, Quality, VideoFrame};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, TICK_HZ, Tick};
use rsmpeg::avcodec::{AVCodec, AVCodecContext};
use rsmpeg::avformat::AVFormatContextInput;
use rsmpeg::avutil::{AVChannelLayout, AVDictionary, AVFrame};
use rsmpeg::error::RsmpegError;
use rsmpeg::ffi;
use rsmpeg::swresample::SwrContext;
use rsmpeg::swscale::SwsContext;
use std::collections::VecDeque;
use std::ffi::CString;

/// Дальше этого порога вперёд не декодируем «сквозь», а делаем seek.
const FORWARD_SEEK_THRESHOLD_OUT_SAMPLES: i64 = 3 * AUDIO_SAMPLE_RATE as i64;
const VIDEO_FORWARD_DECODE_MAX_TICKS: i64 = 4 * TICK_HZ; // 4 c сквозного декода

fn cstr(path: &str) -> Result<CString, MediaError> {
    CString::new(path).map_err(|_| MediaError::Decode("path contains NUL".into()))
}

fn averr(context: &str, e: RsmpegError) -> MediaError {
    MediaError::Decode(format!("{context}: {e}"))
}

/// Пакетный индекс, снятый одним сканом при открытии (без декода).
struct PacketIndex {
    /// Аудио-пакеты: (byte_pos, pts). pts в timebase аудио-потока.
    audio_packets: Vec<(i64, i64)>,
    /// pts ключевых видео-кадров в timebase видео-потока (отсортированы).
    video_keyframes: Vec<i64>,
}

struct AudioPath {
    fmt: AVFormatContextInput,
    dec: AVCodecContext,
    swr: SwrContext,
    stream_index: usize,
    time_base: ffi::AVRational,
    in_rate: i32,
    in_fmt: i32,
    in_layout: ffi::AVChannelLayout,
    /// FIFO готового выхода 48k/2ch f32 interleaved.
    fifo: VecDeque<f32>,
    /// Абсолютный номер (в 48k-кадрах) первого кадра в fifo. None до якоря.
    fifo_start: Option<i64>,
    eof: bool,
}

struct VideoPath {
    fmt: AVFormatContextInput,
    dec: AVCodecContext,
    stream_index: usize,
    time_base: ffi::AVRational,
    codec_id: ffi::AVCodecID,
    start_time: i64,
    /// pts (tb потока) последнего отданного декодером кадра.
    last_pts: Option<i64>,
    /// Кэш sws-контекстов по (dst_w, dst_h, src_w, src_h, src_fmt).
    sws: Option<(i32, i32, i32, i32, i32, SwsContext)>,
    eof: bool,
}

pub struct FfmpegSource {
    info: MediaInfo,
    index: PacketIndex,
    audio: Option<AudioPath>,
    video: Option<VideoPath>,
    audio_start_time: i64,
}

impl FfmpegSource {
    pub fn open(path: &str) -> Result<FfmpegSource, MediaError> {
        let cpath = cstr(path)?;
        // Скан-проход: probe + пакетный индекс (позиции аудио-пакетов,
        // ключевые кадры видео). Декода нет — только чтение пакетов.
        let mut scan = AVFormatContextInput::open(&cpath).map_err(|e| averr("open", e))?;
        let audio_best =
            scan.find_best_stream(ffi::AVMEDIA_TYPE_AUDIO).map_err(|e| averr("find audio", e))?;
        let video_best =
            scan.find_best_stream(ffi::AVMEDIA_TYPE_VIDEO).map_err(|e| averr("find video", e))?;
        let audio_idx = audio_best.map(|(i, _)| i);
        let video_idx = video_best.map(|(i, _)| i);

        let mut index = PacketIndex { audio_packets: Vec::new(), video_keyframes: Vec::new() };
        while let Some(pkt) = scan.read_packet().map_err(|e| averr("scan", e))? {
            let si = pkt.stream_index as usize;
            if Some(si) == audio_idx && pkt.pts != ffi::AV_NOPTS_VALUE {
                index.audio_packets.push((pkt.pos, pkt.pts));
            } else if Some(si) == video_idx
                && (pkt.flags & ffi::AV_PKT_FLAG_KEY as i32) != 0
                && pkt.pts != ffi::AV_NOPTS_VALUE
            {
                index.video_keyframes.push(pkt.pts);
            }
        }
        index.video_keyframes.sort_unstable();

        let duration = if scan.duration > 0 {
            Tick(scan.duration * TICK_HZ / ffi::AV_TIME_BASE as i64)
        } else {
            Tick(0)
        };
        let fps = video_idx.map(|i| {
            let st = &scan.streams()[i];
            let r = st.avg_frame_rate;
            if r.num > 0 && r.den > 0 { (r.num as u32, r.den as u32) } else { (30, 1) }
        });
        let info = MediaInfo {
            duration,
            has_video: video_idx.is_some(),
            has_audio: audio_idx.is_some(),
            fps,
        };
        drop(scan);

        // Отдельные демуксеры на аудио и видео (как в старом движке —
        // это избавляло от драки за один FormatContext).
        let audio = match audio_idx {
            Some(i) => Some(AudioPath::open(path, i)?),
            None => None,
        };
        let audio_start_time = audio
            .as_ref()
            .map(|a| {
                let st = a.fmt.streams()[a.stream_index].start_time;
                if st == ffi::AV_NOPTS_VALUE { 0 } else { st }
            })
            .unwrap_or(0);
        let video = match video_idx {
            Some(i) => Some(VideoPath::open(path, i)?),
            None => None,
        };

        Ok(FfmpegSource { info, index, audio, video, audio_start_time })
    }
}

// ---------------------------------------------------------------------------
// Аудио
// ---------------------------------------------------------------------------

impl AudioPath {
    fn open(path: &str, stream_index: usize) -> Result<AudioPath, MediaError> {
        let fmt = AVFormatContextInput::open(&cstr(path)?).map_err(|e| averr("audio open", e))?;
        let (dec, time_base, in_rate, in_fmt, in_layout, _codec_id) =
            Self::open_decoder(&fmt, stream_index)?;
        let swr = Self::make_swr(in_rate, in_fmt, &in_layout)?;
        Ok(AudioPath {
            fmt,
            dec,
            swr,
            stream_index,
            time_base,
            in_rate,
            in_fmt,
            in_layout,
            fifo: VecDeque::new(),
            fifo_start: None,
            eof: false,
        })
    }

    fn open_decoder(
        fmt: &AVFormatContextInput,
        stream_index: usize,
    ) -> Result<
        (AVCodecContext, ffi::AVRational, i32, i32, ffi::AVChannelLayout, ffi::AVCodecID),
        MediaError,
    > {
        let st = &fmt.streams()[stream_index];
        let par = st.codecpar();
        let codec_id = par.codec_id;
        let codec = AVCodec::find_decoder(codec_id)
            .ok_or_else(|| MediaError::Decode("no audio decoder".into()))?;
        let mut dec = AVCodecContext::new(&codec);
        dec.apply_codecpar(&par).map_err(|e| averr("audio codecpar", e))?;
        dec.set_pkt_timebase(st.time_base);
        dec.open(None).map_err(|e| averr("audio decoder open", e))?;
        let in_layout = {
            let mut l = AVChannelLayout::from_nb_channels(dec.ch_layout().nb_channels);
            l.copy(&dec.ch_layout());
            l.into_inner()
        };
        Ok((dec, st.time_base, par.sample_rate, par.format, in_layout, codec_id))
    }

    fn make_swr(
        in_rate: i32,
        in_fmt: i32,
        in_layout: &ffi::AVChannelLayout,
    ) -> Result<SwrContext, MediaError> {
        let out_layout = AVChannelLayout::from_nb_channels(AUDIO_CHANNELS as i32).into_inner();
        let mut swr = SwrContext::new(
            &out_layout,
            ffi::AV_SAMPLE_FMT_FLT,
            AUDIO_SAMPLE_RATE as i32,
            in_layout,
            in_fmt,
            in_rate,
        )
        .map_err(|e| averr("swr", e))?;
        swr.init().map_err(|e| averr("swr init", e))?;
        Ok(swr)
    }

    /// Полный сброс декодера+ресемплера (после seek).
    fn reopen_decoder(&mut self) -> Result<(), MediaError> {
        let (dec, ..) = Self::open_decoder(&self.fmt, self.stream_index)?;
        self.dec = dec;
        self.swr = Self::make_swr(self.in_rate, self.in_fmt, &self.in_layout)?;
        self.fifo.clear();
        self.fifo_start = None;
        self.eof = false;
        Ok(())
    }

    /// Пропустить декодированный кадр через swr и дописать в fifo.
    fn push_frame(&mut self, frame: Option<&AVFrame>, start_time: i64) -> Result<(), MediaError> {
        if let (Some(f), None) = (frame, self.fifo_start) {
            // якорь: pts первого кадра после seek → абсолютный 48k-кадр
            let pts = if f.best_effort_timestamp != ffi::AV_NOPTS_VALUE {
                f.best_effort_timestamp
            } else if f.pts != ffi::AV_NOPTS_VALUE {
                f.pts
            } else {
                0
            };
            let rel = pts - start_time;
            let out_sample = unsafe {
                ffi::av_rescale_q(
                    rel,
                    self.time_base,
                    ffi::AVRational { num: 1, den: AUDIO_SAMPLE_RATE as i32 },
                )
            };
            self.fifo_start = Some(out_sample);
        }
        let mut out = AVFrame::new();
        out.set_nb_samples(match frame {
            Some(f) => self.swr.get_out_samples(f.nb_samples),
            None => self.swr.get_out_samples(0).max(1),
        });
        out.set_ch_layout(AVChannelLayout::from_nb_channels(AUDIO_CHANNELS as i32).into_inner());
        out.set_format(ffi::AV_SAMPLE_FMT_FLT);
        out.set_sample_rate(AUDIO_SAMPLE_RATE as i32);
        out.alloc_buffer().map_err(|e| averr("swr out alloc", e))?;
        self.swr.convert_frame(frame, &mut out).map_err(|e| averr("swr convert", e))?;
        let produced = out.nb_samples as usize;
        if produced > 0 {
            let data = unsafe {
                std::slice::from_raw_parts(out.data[0] as *const f32, produced * AUDIO_CHANNELS)
            };
            self.fifo.extend(data.iter().copied());
        }
        Ok(())
    }

    /// Декодировать вперёд, пока fifo не покроет [target_end) или EOF.
    fn decode_until(&mut self, target_end: i64, start_time: i64) -> Result<(), MediaError> {
        loop {
            if let Some(fs) = self.fifo_start
                && fs + (self.fifo.len() / AUDIO_CHANNELS) as i64 >= target_end
            {
                return Ok(());
            }
            if self.eof {
                return Ok(());
            }
            match self.fmt.read_packet().map_err(|e| averr("audio read", e))? {
                Some(pkt) => {
                    if pkt.stream_index as usize != self.stream_index {
                        continue;
                    }
                    self.dec.send_packet(Some(&pkt)).map_err(|e| averr("audio send", e))?;
                }
                None => {
                    self.dec.send_packet(None).map_err(|e| averr("audio flush", e))?;
                    self.eof = true;
                }
            }
            loop {
                match self.dec.receive_frame() {
                    Ok(frame) => self.push_frame(Some(&frame), start_time)?,
                    Err(RsmpegError::DecoderDrainError) => break,
                    Err(RsmpegError::DecoderFlushedError) => {
                        // дренаж swr
                        self.push_frame(None, start_time)?;
                        break;
                    }
                    Err(e) => return Err(averr("audio receive", e)),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Видео
// ---------------------------------------------------------------------------

impl VideoPath {
    fn open(path: &str, stream_index: usize) -> Result<VideoPath, MediaError> {
        let fmt = AVFormatContextInput::open(&cstr(path)?).map_err(|e| averr("video open", e))?;
        let st = &fmt.streams()[stream_index];
        let time_base = st.time_base;
        let start_time = if st.start_time == ffi::AV_NOPTS_VALUE { 0 } else { st.start_time };
        let codec_id = st.codecpar().codec_id;
        let dec = Self::open_decoder(&fmt, stream_index, codec_id)?;
        Ok(VideoPath {
            fmt,
            dec,
            stream_index,
            time_base,
            codec_id,
            start_time,
            last_pts: None,
            sws: None,
            eof: false,
        })
    }

    fn open_decoder(
        fmt: &AVFormatContextInput,
        stream_index: usize,
        codec_id: ffi::AVCodecID,
    ) -> Result<AVCodecContext, MediaError> {
        let st = &fmt.streams()[stream_index];
        let codec = AVCodec::find_decoder(codec_id)
            .ok_or_else(|| MediaError::Decode("no video decoder".into()))?;
        let mut dec = AVCodecContext::new(&codec);
        dec.apply_codecpar(&st.codecpar()).map_err(|e| averr("video codecpar", e))?;
        dec.set_pkt_timebase(st.time_base);
        // VP9/AV1: threads=1 — многопоточный декод этих кодеков нестабилен
        // при частых seek (проверено старым движком).
        let single_threaded = codec_id == ffi::AV_CODEC_ID_VP9 || codec_id == ffi::AV_CODEC_ID_AV1;
        let dict = single_threaded.then(|| AVDictionary::new(c"threads", c"1", 0));
        dec.open(dict).map_err(|e| averr("video decoder open", e))?;
        Ok(dec)
    }

    /// Backward seek: контейнерный seek + ПОЛНОЕ пересоздание декодера
    /// (безопасно для MT H.264, дешевле, чем ловить его артефакты).
    fn seek_and_reopen(&mut self, target_ts: i64) -> Result<(), MediaError> {
        self.fmt
            .seek(self.stream_index as i32, target_ts, ffi::AVSEEK_FLAG_BACKWARD as i32)
            .map_err(|e| averr("video seek", e))?;
        self.dec = Self::open_decoder(&self.fmt, self.stream_index, self.codec_id)?;
        self.last_pts = None;
        self.eof = false;
        Ok(())
    }

    /// Декодировать вперёд до кадра, содержащего target_ts (tb потока).
    /// Возвращает кадр с наибольшим pts <= target_ts (или первый после,
    /// если точного нет).
    fn decode_to(&mut self, target_ts: i64) -> Result<AVFrame, MediaError> {
        let mut best: Option<AVFrame> = None;
        loop {
            if self.eof {
                break;
            }
            match self.fmt.read_packet().map_err(|e| averr("video read", e))? {
                Some(pkt) => {
                    if pkt.stream_index as usize != self.stream_index {
                        continue;
                    }
                    self.dec.send_packet(Some(&pkt)).map_err(|e| averr("video send", e))?;
                }
                None => {
                    self.dec.send_packet(None).map_err(|e| averr("video drain", e))?;
                    self.eof = true;
                }
            }
            loop {
                match self.dec.receive_frame() {
                    Ok(frame) => {
                        let pts = if frame.best_effort_timestamp != ffi::AV_NOPTS_VALUE {
                            frame.best_effort_timestamp
                        } else {
                            frame.pts
                        };
                        self.last_pts = Some(pts);
                        let done = pts >= target_ts;
                        if pts <= target_ts || best.is_none() {
                            best = Some(frame);
                        }
                        if done && let Some(b) = best.take() {
                            return Ok(b);
                        }
                    }
                    Err(RsmpegError::DecoderDrainError) => break,
                    Err(RsmpegError::DecoderFlushedError) => {
                        self.eof = true;
                        break;
                    }
                    Err(e) => return Err(averr("video receive", e)),
                }
            }
        }
        best.ok_or(MediaError::OutOfRange)
    }

    fn frame_to_rgba(
        &mut self,
        frame: &AVFrame,
        quality: Option<Quality>,
    ) -> Result<VideoFrame, MediaError> {
        let (dst_w, dst_h) = match quality {
            Some(q) => (q.width.max(2) as i32 & !1, q.height.max(2) as i32 & !1),
            None => (frame.width, frame.height),
        };
        let key = (dst_w, dst_h, frame.width, frame.height, frame.format);
        if self.sws.as_ref().map(|s| (s.0, s.1, s.2, s.3, s.4)) != Some(key) {
            let ctx = SwsContext::get_context(
                frame.width,
                frame.height,
                frame.format,
                dst_w,
                dst_h,
                ffi::AV_PIX_FMT_RGBA,
                ffi::SWS_FAST_BILINEAR,
                None,
                None,
                None,
            )
            .ok_or_else(|| MediaError::Decode("sws_getContext failed".into()))?;
            self.sws = Some((dst_w, dst_h, frame.width, frame.height, frame.format, ctx));
        }
        let mut dst = AVFrame::new();
        dst.set_width(dst_w);
        dst.set_height(dst_h);
        dst.set_format(ffi::AV_PIX_FMT_RGBA);
        dst.alloc_buffer().map_err(|e| averr("rgba alloc", e))?;
        let sws = &mut self.sws.as_mut().unwrap().5;
        sws.scale_frame(frame, 0, frame.height, &mut dst).map_err(|e| averr("sws", e))?;

        // копия с учётом выравнивания linesize → плотный буфер
        let stride = dst.linesize[0] as usize;
        let row = (dst_w as usize) * 4;
        let mut rgba = vec![0u8; row * dst_h as usize];
        let src = unsafe { std::slice::from_raw_parts(dst.data[0], stride * dst_h as usize) };
        for y in 0..dst_h as usize {
            rgba[y * row..(y + 1) * row].copy_from_slice(&src[y * stride..y * stride + row]);
        }
        let pts_tick = {
            let pts = if frame.best_effort_timestamp != ffi::AV_NOPTS_VALUE {
                frame.best_effort_timestamp
            } else {
                frame.pts
            };
            let rel = pts - self.start_time;
            Tick(unsafe {
                ffi::av_rescale_q(
                    rel,
                    self.time_base,
                    ffi::AVRational { num: 1, den: TICK_HZ as i32 },
                )
            })
        };
        Ok(VideoFrame {
            pts: pts_tick,
            width: dst_w as u32,
            height: dst_h as u32,
            rgba: rgba.into_boxed_slice(),
            has_alpha: false,
        })
    }
}

// ---------------------------------------------------------------------------
// MediaSource
// ---------------------------------------------------------------------------

impl MediaSource for FfmpegSource {
    fn info(&self) -> &MediaInfo {
        &self.info
    }

    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        if idx < 0 {
            return Err(MediaError::OutOfRange);
        }
        let start_time = self.audio_start_time;
        let audio = self.audio.as_mut().ok_or(MediaError::OutOfRange)?;
        let t0 = idx * AUDIO_BURST_FRAMES as i64;
        let t1 = t0 + AUDIO_BURST_FRAMES as i64;

        // seek нужен, если цель вне [fifo_start, fifo_end + порог сквозного декода)
        let need_seek = match audio.fifo_start {
            Some(fs) => {
                let fe = fs + (audio.fifo.len() / AUDIO_CHANNELS) as i64;
                t0 < fs || t0 > fe + FORWARD_SEEK_THRESHOLD_OUT_SAMPLES
            }
            None => true,
        };

        if need_seek {
            // ближайший индексированный аудио-пакет с pts-временем <= цели,
            // с запасом в пару пакетов на разгон декодера
            let target_pts = unsafe {
                ffi::av_rescale_q(
                    t0,
                    ffi::AVRational { num: 1, den: AUDIO_SAMPLE_RATE as i32 },
                    audio.time_base,
                )
            } + start_time;
            let pos = self
                .index
                .audio_packets
                .partition_point(|&(_, pts)| pts <= target_pts)
                .saturating_sub(3);
            let (_byte_pos, pkt_pts) =
                self.index.audio_packets.get(pos).copied().unwrap_or((0, start_time));
            audio
                .fmt
                .seek(audio.stream_index as i32, pkt_pts, ffi::AVSEEK_FLAG_BACKWARD as i32)
                .map_err(|e| averr("audio seek", e))?;
            audio.reopen_decoder()?;
        }

        audio.decode_until(t1, start_time)?;

        let fs = audio.fifo_start.ok_or(MediaError::OutOfRange)?;
        // выбрасываем из fifo всё до t0
        if t0 > fs {
            let drop = ((t0 - fs) as usize * AUDIO_CHANNELS).min(audio.fifo.len());
            audio.fifo.drain(..drop);
            audio.fifo_start = Some(fs + (drop / AUDIO_CHANNELS) as i64);
        }
        let fs = audio.fifo_start.unwrap();
        if fs > t0 {
            // якорь оказался позже цели (сигнал начинается не с нуля) —
            // недостающее заполняем тишиной
            let mut burst = AudioBurst::silence(idx);
            let offset = ((fs - t0) as usize).min(AUDIO_BURST_FRAMES);
            let avail = (audio.fifo.len() / AUDIO_CHANNELS).min(AUDIO_BURST_FRAMES - offset);
            for f in 0..avail {
                for ch in 0..AUDIO_CHANNELS {
                    burst.data[(offset + f) * AUDIO_CHANNELS + ch] =
                        audio.fifo[f * AUDIO_CHANNELS + ch];
                }
            }
            return Ok(burst);
        }
        let have = audio.fifo.len() / AUDIO_CHANNELS;
        if have == 0 && audio.eof {
            return Err(MediaError::OutOfRange);
        }
        let mut burst = AudioBurst::silence(idx);
        let n = have.min(AUDIO_BURST_FRAMES);
        for i in 0..n * AUDIO_CHANNELS {
            burst.data[i] = audio.fifo[i];
        }
        Ok(burst)
    }

    fn read_video_frame(
        &mut self,
        pts: Tick,
        quality: Option<Quality>,
    ) -> Result<VideoFrame, MediaError> {
        let video = self.video.as_mut().ok_or(MediaError::OutOfRange)?;
        let target_ts = unsafe {
            ffi::av_rescale_q(
                pts.0,
                ffi::AVRational { num: 1, den: TICK_HZ as i32 },
                video.time_base,
            )
        } + video.start_time;

        let forward_ok = match video.last_pts {
            Some(last) => {
                target_ts > last && {
                    let ahead_ticks = unsafe {
                        ffi::av_rescale_q(
                            target_ts - last,
                            video.time_base,
                            ffi::AVRational { num: 1, den: TICK_HZ as i32 },
                        )
                    };
                    ahead_ticks < VIDEO_FORWARD_DECODE_MAX_TICKS
                }
            }
            None => false,
        };
        if !forward_ok {
            video.seek_and_reopen(target_ts)?;
        }
        let frame = video.decode_to(target_ts)?;
        video.frame_to_rgba(&frame, quality)
    }

    fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        let video = self.video.as_ref()?;
        let ts = unsafe {
            ffi::av_rescale_q(
                pts.0,
                ffi::AVRational { num: 1, den: TICK_HZ as i32 },
                video.time_base,
            )
        } + video.start_time;
        let i = self.index.video_keyframes.partition_point(|&k| k <= ts);
        let k = *self.index.video_keyframes.get(i.checked_sub(1)?)?;
        let rel = k - video.start_time;
        Some(Tick(unsafe {
            ffi::av_rescale_q(rel, video.time_base, ffi::AVRational { num: 1, den: TICK_HZ as i32 })
        }))
    }
}

// SAFETY: все raw-указатели ffmpeg живут внутри и не покидают структуру;
// доступ — только через &mut self (экземпляром владеет один MediaWorker).
unsafe impl Send for FfmpegSource {}
