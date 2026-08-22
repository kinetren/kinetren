// Внутрипроцессный муксер MP4 (h264 + aac). ВКЛЮЧАЕТСЯ в каждый версионный
// модуль вместе с source_impl.rs — типы соответствуют загруженной версии.
//
// Заменяет прежний шелл-аут в бинарник ffmpeg: кодеки берутся из СИСТЕМНОГО
// FFmpeg, поэтому доступны его аппаратные энкодеры (nvenc/vaapi/qsv).

use crate::MuxConfig;

/// Константы-#define, которые bindgen не выносит.
const AVFMT_GLOBALHEADER: c_int = 0x0040;
const AVIO_FLAG_WRITE: c_int = 2;
const AV_CODEC_FLAG_GLOBAL_HEADER: c_int = 1 << 22;

/// Кандидаты видео-энкодера по убыванию предпочтения: сперва аппаратные.
const VIDEO_ENCODERS: &[(&str, bool)] = &[
    ("h264_nvenc", true),
    ("h264_vaapi", true),
    ("h264_qsv", true),
    ("libx264", false),
    ("mpeg4", false),
];

/// Доступные в системе энкодеры (для UI).
pub fn available_encoders(api: &Api) -> Vec<String> {
    VIDEO_ENCODERS
        .iter()
        .filter(|(name, _)| {
            let c = CString::new(*name).unwrap();
            !unsafe { (api.avcodec_find_encoder_by_name)(c.as_ptr()) }.is_null()
        })
        .map(|(name, _)| name.to_string())
        .collect()
}

pub struct Muxer {
    api: &'static Api,
    fmt: *mut ffi::AVFormatContext,
    venc: *mut ffi::AVCodecContext,
    aenc: *mut ffi::AVCodecContext,
    vstream_idx: c_int,
    astream_idx: c_int,
    sws: *mut ffi::SwsContext,
    vframe: *mut ffi::AVFrame,
    aframe: *mut ffi::AVFrame,
    pkt: *mut ffi::AVPacket,
    width: c_int,
    height: c_int,
    fps: c_int,
    video_pts: i64,
    audio_pts: i64,
    /// Накопитель аудио до размера кадра энкодера (aac = 1024).
    audio_fifo: Vec<f32>,
    frame_size: usize,
    encoder_name: String,
}

unsafe impl Send for Muxer {}

impl Muxer {
    pub fn open(api: &'static Api, cfg: &MuxConfig) -> Result<Muxer, FfError> {
        let path = CString::new(cfg.path.as_str())
            .map_err(|_| FfError::Av("путь содержит NUL".into()))?;
        let mut fmt: *mut ffi::AVFormatContext = core::ptr::null_mut();
        let mp4 = CString::new("mp4").unwrap();
        let r = unsafe {
            (api.avformat_alloc_output_context2)(
                &mut fmt,
                core::ptr::null(),
                mp4.as_ptr(),
                path.as_ptr(),
            )
        };
        if r < 0 || fmt.is_null() {
            return Err(averr(api, "создание mp4-контекста", r));
        }

        // --- видео-энкодер: автовыбор с РЕАЛЬНОЙ проверкой открытия ---
        // Наличие энкодера в сборке (find_encoder_by_name) не значит, что он
        // заработает: h264_nvenc есть почти везде, но без libcuda/железа
        // avcodec_open2 падает. Поэтому пробуем открывать по очереди.
        let width = (cfg.width.max(2) as c_int) & !1;
        let height = (cfg.height.max(2) as c_int) & !1;
        let set = |d: &mut *mut ffi::AVDictionary, k: &str, v: &str| {
            let (ck, cv) = (CString::new(k).unwrap(), CString::new(v).unwrap());
            unsafe { (api.av_dict_set)(d, ck.as_ptr(), cv.as_ptr(), 0) };
        };
        let global_header = unsafe {
            !(*fmt).oformat.is_null()
                && ((*(*fmt).oformat).flags & AVFMT_GLOBALHEADER) != 0
        };
        // на время проб глушим FFmpeg-лог (неудачные кандидаты — норма)
        unsafe { (api.av_log_set_level)(8) }; // AV_LOG_FATAL
        let mut opened: Option<(String, *mut ffi::AVCodecContext)> = None;
        let mut last_err = String::new();
        for (name, _hw) in VIDEO_ENCODERS {
            if let Some(pref) = &cfg.force_encoder
                && pref != name
            {
                continue;
            }
            let c = CString::new(*name).unwrap();
            let codec = unsafe { (api.avcodec_find_encoder_by_name)(c.as_ptr()) };
            if codec.is_null() {
                continue;
            }
            let ctx = unsafe { (api.avcodec_alloc_context3)(codec) };
            unsafe {
                (*ctx).width = width;
                (*ctx).height = height;
                (*ctx).time_base = tb(1, cfg.fps as c_int);
                (*ctx).framerate = tb(cfg.fps as c_int, 1);
                (*ctx).pix_fmt = ffi::AV_PIX_FMT_YUV420P;
                (*ctx).gop_size = cfg.fps as c_int * 2;
                (*ctx).max_b_frames = 0;
                if global_header {
                    (*ctx).flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
                }
            }
            let mut vopts: *mut ffi::AVDictionary = core::ptr::null_mut();
            match *name {
                "libx264" => {
                    set(&mut vopts, "preset", "veryfast");
                    set(&mut vopts, "crf", &cfg.quality.to_string());
                }
                "h264_nvenc" => {
                    set(&mut vopts, "preset", "p4");
                    set(&mut vopts, "rc", "vbr");
                    set(&mut vopts, "cq", &cfg.quality.to_string());
                }
                "h264_qsv" => set(&mut vopts, "global_quality", &cfg.quality.to_string()),
                "h264_vaapi" => set(&mut vopts, "qp", &cfg.quality.to_string()),
                _ => {}
            }
            let r = unsafe { (api.avcodec_open2)(ctx, codec, &mut vopts) };
            unsafe { (api.av_dict_free)(&mut vopts) };
            if r >= 0 {
                opened = Some((name.to_string(), ctx));
                break;
            }
            last_err = format!("{name}: код {r}");
            let mut ctx = ctx;
            unsafe { (api.avcodec_free_context)(&mut ctx) };
        }
        unsafe { (api.av_log_set_level)(16) }; // обратно AV_LOG_ERROR
        let (enc_name, venc) = opened.ok_or_else(|| {
            FfError::Av(format!(
                "ни один видео-энкодер не открылся (последняя ошибка — {last_err})"
            ))
        })?;
        let enc_name = enc_name.as_str();

        // --- аудио-энкодер aac ---
        let aac = CString::new("aac").unwrap();
        let acodec = unsafe { (api.avcodec_find_encoder_by_name)(aac.as_ptr()) };
        if acodec.is_null() {
            return Err(FfError::Av("в системном FFmpeg нет AAC-энкодера".into()));
        }
        let aenc = unsafe { (api.avcodec_alloc_context3)(acodec) };
        unsafe {
            (*aenc).sample_rate = AUDIO_SAMPLE_RATE as c_int;
            (*aenc).sample_fmt = ffi::AV_SAMPLE_FMT_FLTP;
            (*aenc).bit_rate = cfg.audio_bitrate as i64;
            (*aenc).time_base = tb(1, AUDIO_SAMPLE_RATE as c_int);
            (api.av_channel_layout_default)(&mut (*aenc).ch_layout, AUDIO_CHANNELS as c_int);
            if ((*(*fmt).oformat).flags & AVFMT_GLOBALHEADER) != 0 {
                (*aenc).flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
            }
        }
        let r = unsafe { (api.avcodec_open2)(aenc, acodec, core::ptr::null_mut()) };
        if r < 0 {
            return Err(averr(api, "открытие AAC", r));
        }
        let frame_size = {
            let fs = unsafe { (*aenc).frame_size };
            if fs > 0 { fs as usize } else { 1024 }
        };

        // --- потоки ---
        let vst = unsafe { (api.avformat_new_stream)(fmt, core::ptr::null()) };
        let ast = unsafe { (api.avformat_new_stream)(fmt, core::ptr::null()) };
        if vst.is_null() || ast.is_null() {
            return Err(FfError::Av("не удалось создать потоки".into()));
        }
        unsafe {
            (*vst).time_base = tb(1, cfg.fps as c_int);
            (*ast).time_base = tb(1, AUDIO_SAMPLE_RATE as c_int);
            (api.avcodec_parameters_from_context)((*vst).codecpar, venc);
            (api.avcodec_parameters_from_context)((*ast).codecpar, aenc);
        }
        let (vstream_idx, astream_idx) = unsafe { ((*vst).index, (*ast).index) };

        // --- открытие файла и заголовок (+faststart) ---
        let r = unsafe { (api.avio_open)(&mut (*fmt).pb, path.as_ptr(), AVIO_FLAG_WRITE) };
        if r < 0 {
            return Err(averr(api, "создание файла", r));
        }
        let mut mopts: *mut ffi::AVDictionary = core::ptr::null_mut();
        set(&mut mopts, "movflags", "+faststart");
        let r = unsafe { (api.avformat_write_header)(fmt, &mut mopts) };
        unsafe { (api.av_dict_free)(&mut mopts) };
        if r < 0 {
            return Err(averr(api, "запись заголовка", r));
        }

        // --- рабочие буферы ---
        let vframe = unsafe { (api.av_frame_alloc)() };
        unsafe {
            (*vframe).format = ffi::AV_PIX_FMT_YUV420P;
            (*vframe).width = width;
            (*vframe).height = height;
            (api.av_frame_get_buffer)(vframe, 0);
        }
        let aframe = unsafe { (api.av_frame_alloc)() };
        unsafe {
            (*aframe).format = ffi::AV_SAMPLE_FMT_FLTP;
            (*aframe).nb_samples = frame_size as c_int;
            (api.av_channel_layout_default)(&mut (*aframe).ch_layout, AUDIO_CHANNELS as c_int);
            (*aframe).sample_rate = AUDIO_SAMPLE_RATE as c_int;
            (api.av_frame_get_buffer)(aframe, 0);
        }
        let sws = unsafe {
            (api.sws_getContext)(
                width,
                height,
                ffi::AV_PIX_FMT_RGBA,
                width,
                height,
                ffi::AV_PIX_FMT_YUV420P,
                ffi::SWS_BILINEAR as c_int,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                core::ptr::null(),
            )
        };
        Ok(Muxer {
            api,
            fmt,
            venc,
            aenc,
            vstream_idx,
            astream_idx,
            sws,
            vframe,
            aframe,
            pkt: unsafe { (api.av_packet_alloc)() },
            width,
            height,
            fps: cfg.fps as c_int,
            video_pts: 0,
            audio_pts: 0,
            audio_fifo: Vec::new(),
            frame_size,
            encoder_name: enc_name.to_string(),
        })
    }

    pub fn encoder_name(&self) -> &str {
        &self.encoder_name
    }

    pub fn write_video(&mut self, rgba: &[u8]) -> Result<(), FfError> {
        let api = self.api;
        let expected = self.width as usize * self.height as usize * 4;
        if rgba.len() != expected {
            return Err(FfError::Av(format!(
                "кадр {} байт, ожидалось {expected}",
                rgba.len()
            )));
        }
        unsafe {
            let src: [*const u8; 4] = [rgba.as_ptr(), core::ptr::null(), core::ptr::null(), core::ptr::null()];
            let src_ls: [c_int; 4] = [self.width * 4, 0, 0, 0];
            (api.sws_scale)(
                self.sws,
                src.as_ptr(),
                src_ls.as_ptr(),
                0,
                self.height,
                (*self.vframe).data.as_ptr(),
                (*self.vframe).linesize.as_ptr(),
            );
            (*self.vframe).pts = self.video_pts;
        }
        self.video_pts += 1;
        self.encode(true)
    }

    pub fn write_audio(&mut self, samples: &[f32]) -> Result<(), FfError> {
        self.audio_fifo.extend_from_slice(samples);
        while self.audio_fifo.len() >= self.frame_size * AUDIO_CHANNELS {
            unsafe {
                // interleaved f32 → planar FLTP
                for ch in 0..AUDIO_CHANNELS {
                    let dst = (*self.aframe).data[ch] as *mut f32;
                    for i in 0..self.frame_size {
                        *dst.add(i) = self.audio_fifo[i * AUDIO_CHANNELS + ch];
                    }
                }
                (*self.aframe).pts = self.audio_pts;
            }
            self.audio_pts += self.frame_size as i64;
            self.audio_fifo.drain(..self.frame_size * AUDIO_CHANNELS);
            self.encode(false)?;
        }
        Ok(())
    }

    fn encode(&mut self, video: bool) -> Result<(), FfError> {
        let api = self.api;
        let (enc, frame, stream_idx, tb_src) = if video {
            (self.venc, self.vframe, self.vstream_idx, tb(1, self.fps))
        } else {
            (self.aenc, self.aframe, self.astream_idx, tb(1, AUDIO_SAMPLE_RATE as c_int))
        };
        let r = unsafe { (api.avcodec_send_frame)(enc, frame) };
        if r < 0 && r != AVERROR_EAGAIN {
            return Err(averr(api, "энкодер send", r));
        }
        self.drain(enc, stream_idx, tb_src)
    }

    fn drain(
        &mut self,
        enc: *mut ffi::AVCodecContext,
        stream_idx: c_int,
        tb_src: ffi::AVRational,
    ) -> Result<(), FfError> {
        let api = self.api;
        loop {
            let r = unsafe { (api.avcodec_receive_packet)(enc, self.pkt) };
            if r == AVERROR_EAGAIN || r == AVERROR_EOF_ {
                return Ok(());
            }
            if r < 0 {
                return Err(averr(api, "энкодер receive", r));
            }
            unsafe {
                let st = &**(*self.fmt).streams.add(stream_idx as usize);
                (api.av_packet_rescale_ts)(self.pkt, tb_src, st.time_base);
                (*self.pkt).stream_index = stream_idx;
                let r = (api.av_interleaved_write_frame)(self.fmt, self.pkt);
                (api.av_packet_unref)(self.pkt);
                if r < 0 {
                    return Err(averr(api, "запись пакета", r));
                }
            }
        }
    }

    /// Дозаписать хвосты, закрыть файл.
    pub fn finish(mut self) -> Result<(), FfError> {
        let api = self.api;
        // добить неполный аудио-кадр тишиной
        if !self.audio_fifo.is_empty() {
            let need = self.frame_size * AUDIO_CHANNELS - self.audio_fifo.len();
            self.audio_fifo.extend(std::iter::repeat_n(0.0, need));
            let fifo = std::mem::take(&mut self.audio_fifo);
            self.write_audio(&fifo)?;
        }
        // флаш энкодеров
        unsafe { (api.avcodec_send_frame)(self.venc, core::ptr::null()) };
        self.drain(self.venc, self.vstream_idx, tb(1, self.fps))?;
        unsafe { (api.avcodec_send_frame)(self.aenc, core::ptr::null()) };
        self.drain(self.aenc, self.astream_idx, tb(1, AUDIO_SAMPLE_RATE as c_int))?;
        let r = unsafe { (api.av_write_trailer)(self.fmt) };
        if r < 0 {
            return Err(averr(api, "финализация файла", r));
        }
        Ok(())
    }
}

impl Drop for Muxer {
    fn drop(&mut self) {
        let api = self.api;
        unsafe {
            if !self.sws.is_null() {
                (api.sws_freeContext)(self.sws);
            }
            (api.av_frame_free)(&mut self.vframe);
            (api.av_frame_free)(&mut self.aframe);
            (api.av_packet_free)(&mut self.pkt);
            (api.avcodec_free_context)(&mut self.venc);
            (api.avcodec_free_context)(&mut self.aenc);
            if !self.fmt.is_null() {
                (api.avio_closep)(&mut (*self.fmt).pb);
                (api.avformat_free_context)(self.fmt);
                self.fmt = core::ptr::null_mut();
            }
        }
    }
}
