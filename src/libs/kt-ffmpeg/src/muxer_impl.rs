// In-process MP4 muxer (h264 + aac). INCLUDED into each version
// module together with source_impl.rs — the types match the loaded version.
//
// It replaces the former shell-out to the ffmpeg binary: codecs are taken from the
// installed FFmpeg, so its hardware encoders are available. The cascade per OS and each
// candidate's pixel format and options: `crate::encoders` (windows-build).

use crate::MuxConfig;

/// #define constants that bindgen does not emit.
const AVFMT_GLOBALHEADER: c_int = 0x0040;
const AVIO_FLAG_WRITE: c_int = 2;
const AV_CODEC_FLAG_GLOBAL_HEADER: c_int = 1 << 22;

/// The cascade of this OS.
fn video_encoders() -> Vec<crate::encoders::EncoderSpec> {
    crate::encoders::video_encoder_candidates(crate::search::Os::NATIVE)
}

fn av_pix(p: crate::encoders::PixFmt) -> ffi::AVPixelFormat {
    match p {
        crate::encoders::PixFmt::Yuv420p => ffi::AV_PIX_FMT_YUV420P,
        crate::encoders::PixFmt::Nv12 => ffi::AV_PIX_FMT_NV12,
    }
}

/// The hardware table of this major, or why the device candidates are skipped (D5); the test
/// seam can report it absent.
fn hw_api() -> Result<&'static HwApi, String> {
    if let Some(m) = crate::hw_seam::get().missing {
        return Err(m);
    }
    match HW.get() {
        Some(Ok(h)) => Ok(h),
        Some(Err(m)) => Err(m.clone()),
        None => Err("the hardware table is not loaded".into()),
    }
}

/// Open the first VAAPI device of the plan (D3): the device and its path, or why none opened.
/// The FFmpeg log is silenced by the caller: a node that is not a VAAPI device is normal.
fn open_vaapi_device(hw: &HwApi) -> Result<(*mut ffi::AVBufferRef, String), String> {
    let mut why = Vec::new();
    for path in crate::vaapi_device_paths() {
        let c = CString::new(path.as_str()).map_err(|_| "device path contains NUL".to_string())?;
        let mut dev: *mut ffi::AVBufferRef = core::ptr::null_mut();
        let r = unsafe {
            (hw.av_hwdevice_ctx_create)(&mut dev, ffi::AV_HWDEVICE_TYPE_VAAPI, c.as_ptr(), core::ptr::null_mut(), 0)
        };
        if r >= 0 && !dev.is_null() {
            return Ok((dev, path));
        }
        why.push(format!("{path}: code {r}"));
    }
    Err(if why.is_empty() {
        format!("no VAAPI device (no render node in {})", crate::vaapi_device_dir_hint())
    } else {
        format!("no VAAPI device ({})", why.join(", "))
    })
}

/// The driver's vendor string of an opened VAAPI device (the report): libva's
/// `vaQueryVendorString` on the device's display — the first field of `AVVAAPIDeviceContext`.
#[cfg(unix)]
fn vaapi_vendor(dev: *mut ffi::AVBufferRef) -> String {
    type Query = unsafe extern "C" fn(*mut c_void) -> *const c_char;
    let Ok(lib) = (unsafe { libloading::Library::new("libva.so.2") }) else {
        return "unknown (libva.so.2 not loaded)".into();
    };
    let Ok(query) = (unsafe { lib.get::<Query>(b"vaQueryVendorString\0") }) else {
        return "unknown".into();
    };
    unsafe {
        let ctx = (*dev).data as *const ffi::AVHWDeviceContext;
        let display = *((*ctx).hwctx as *const *mut c_void);
        let v = query(display);
        if v.is_null() { "unknown".into() } else { core::ffi::CStr::from_ptr(v).to_string_lossy().into_owned() }
    }
}

#[cfg(not(unix))]
fn vaapi_vendor(_dev: *mut ffi::AVBufferRef) -> String {
    "unknown".into()
}

/// The hardware section of the report (media-index): the table, and each render node.
pub fn hw_report(api: &'static Api) -> crate::HwReport {
    let functions = hw_api().map(|_| ());
    let linux = crate::search::Os::NATIVE == crate::search::Os::Linux;
    let mut nodes = Vec::new();
    if linux {
        let dir = crate::vaapi_device_dir_hint();
        let all: Vec<String> = (128..=135)
            .map(|n| format!("{dir}/renderD{n}"))
            .filter(|p| std::path::Path::new(p).exists())
            .collect();
        unsafe { (api.av_log_set_level)(8) };
        for node in all {
            let v = match hw_api() {
                Err(m) => Err(format!("hardware context functions missing: {m}")),
                Ok(hw) => {
                    let c = CString::new(node.as_str()).unwrap();
                    let mut dev: *mut ffi::AVBufferRef = core::ptr::null_mut();
                    let r = unsafe {
                        (hw.av_hwdevice_ctx_create)(&mut dev, ffi::AV_HWDEVICE_TYPE_VAAPI, c.as_ptr(), core::ptr::null_mut(), 0)
                    };
                    if r >= 0 && !dev.is_null() {
                        let vendor = vaapi_vendor(dev);
                        unsafe { (hw.av_buffer_unref)(&mut dev) };
                        Ok(vendor)
                    } else {
                        Err(format!("code {r}"))
                    }
                }
            };
            nodes.push((node, v));
        }
        unsafe { (api.av_log_set_level)(16) };
    }
    crate::HwReport { functions, nodes, linux }
}

/// Encoders available in the system (for the UI): present in this FFmpeg; a device candidate
/// only with a device that opens, named (D6) — the encoder itself is not opened here.
pub fn available_encoders(api: &'static Api) -> Vec<crate::AvailableEncoder> {
    let mut out = Vec::new();
    for e in video_encoders() {
        let c = CString::new(e.name).unwrap();
        if unsafe { (api.avcodec_find_encoder_by_name)(c.as_ptr()) }.is_null() {
            continue;
        }
        let device = match e.ctx {
            crate::encoders::HwKind::None => None,
            crate::encoders::HwKind::Vaapi => {
                let Ok(hw) = hw_api() else { continue };
                unsafe { (api.av_log_set_level)(8) };
                let dev = open_vaapi_device(hw);
                unsafe { (api.av_log_set_level)(16) };
                match dev {
                    Ok((mut d, path)) => {
                        unsafe { (hw.av_buffer_unref)(&mut d) };
                        Some(path)
                    }
                    Err(_) => continue,
                }
            }
        };
        out.push(crate::AvailableEncoder { name: e.name.to_string(), device });
    }
    out
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
    /// Audio accumulator up to the encoder frame size (aac = 1024).
    audio_fifo: Vec<f32>,
    frame_size: usize,
    encoder_name: String,
    report: crate::OpenReport,
    /// Encoded packet bytes written so far: [video, audio].
    bytes: [u64; 2],
    /// A device encoder (D3/D4): the table, the device and frames contexts, and the surface each
    /// NV12 frame is uploaded to. All null for a system-memory encoder.
    hw: Option<&'static HwApi>,
    hw_device: *mut ffi::AVBufferRef,
    hw_frames: *mut ffi::AVBufferRef,
    hwframe: *mut ffi::AVFrame,
}

unsafe impl Send for Muxer {}

impl Muxer {
    pub fn open(api: &'static Api, cfg: &MuxConfig) -> Result<Muxer, FfError> {
        let path = CString::new(cfg.path.as_str())
            .map_err(|_| FfError::Av("path contains NUL".into()))?;
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
            return Err(averr(api, "creating mp4 context", r));
        }

        // --- video encoder: automatic choice with a REAL open check ---
        // The presence of an encoder in the build (find_encoder_by_name) does not mean it
        // will work: h264_nvenc is almost everywhere, but without libcuda/hardware
        // avcodec_open2 fails. So we try opening them one by one.
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
        // during the probes, the FFmpeg log is silenced (failed candidates are normal)
        unsafe { (api.av_log_set_level)(8) }; // AV_LOG_FATAL
        let mut opened: Option<(String, *mut ffi::AVCodecContext, ffi::AVPixelFormat)> = None;
        // the device and frames contexts of an opened device candidate (D3)
        let mut hw_opened: Option<(&'static HwApi, *mut ffi::AVBufferRef, *mut ffi::AVBufferRef, String)> = None;
        let mut last_err = String::new();
        // every candidate that exists here and failed, with its code, and the opened one's facts
        let mut report = crate::OpenReport::default();
        for spec in video_encoders() {
            let name = &spec.name;
            if let Some(pref) = &cfg.force_encoder
                && pref != name
            {
                continue;
            }
            let c = CString::new(*name).unwrap();
            let codec = unsafe { (api.avcodec_find_encoder_by_name)(c.as_ptr()) };
            if codec.is_null() {
                // a forced name this FFmpeg lacks is named, not an empty "last error"
                last_err = format!("{name}: not in this FFmpeg build");
                continue;
            }
            // A DEVICE CANDIDATE IS ONE STEP: device → frames → open (D3). What fails before
            // the open skips it WITHOUT `avcodec_open2` — the encoder's failed-init path stays
            // out of the cascade (ktprompts v26.10 T16).
            let mut device: Option<(&'static HwApi, *mut ffi::AVBufferRef, *mut ffi::AVBufferRef, String)> = None;
            if spec.ctx == crate::encoders::HwKind::Vaapi {
                let skip = |report: &mut crate::OpenReport, last_err: &mut String, why: String| {
                    *last_err = format!("{name}: {why}");
                    report.skipped.push((name.to_string(), why));
                };
                let hw = match hw_api() {
                    Ok(h) => h,
                    Err(m) => {
                        skip(&mut report, &mut last_err, format!("hardware context functions missing: {m}"));
                        continue;
                    }
                };
                let (mut dev, path) = match open_vaapi_device(hw) {
                    Ok(d) => d,
                    Err(why) => {
                        skip(&mut report, &mut last_err, why);
                        continue;
                    }
                };
                let mut frames = unsafe { (hw.av_hwframe_ctx_alloc)(dev) };
                if frames.is_null() {
                    unsafe { (hw.av_buffer_unref)(&mut dev) };
                    skip(&mut report, &mut last_err, "frames context: not allocated".into());
                    continue;
                }
                let (fw, fh) = spec.frames_size(width as u32, height as u32).unwrap_or((width as u32, height as u32));
                let r = unsafe {
                    let fc = (*frames).data as *mut ffi::AVHWFramesContext;
                    (*fc).format = ffi::AV_PIX_FMT_VAAPI;
                    (*fc).sw_format = av_pix(spec.pix);
                    (*fc).width = fw as c_int;
                    (*fc).height = fh as c_int;
                    (*fc).initial_pool_size = 20;
                    (hw.av_hwframe_ctx_init)(frames)
                };
                if r < 0 {
                    unsafe {
                        (hw.av_buffer_unref)(&mut frames);
                        (hw.av_buffer_unref)(&mut dev);
                    }
                    skip(&mut report, &mut last_err, format!("frames context: code {r}"));
                    continue;
                }
                device = Some((hw, dev, frames, path));
            }
            let ctx = unsafe { (api.avcodec_alloc_context3)(codec) };
            unsafe {
                (*ctx).width = width;
                (*ctx).height = height;
                (*ctx).time_base = tb(1, cfg.fps as c_int);
                (*ctx).framerate = tb(cfg.fps as c_int, 1);
                (*ctx).pix_fmt = av_pix(spec.pix);
                if let Some((hw, _, frames, _)) = &device {
                    (*ctx).pix_fmt = ffi::AV_PIX_FMT_VAAPI;
                    (*ctx).hw_frames_ctx = (hw.av_buffer_ref)(*frames);
                }
                (*ctx).gop_size = cfg.fps as c_int * 2;
                (*ctx).max_b_frames = 0;
                if global_header {
                    (*ctx).flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
                }
            }
            let mut vopts: *mut ffi::AVDictionary = core::ptr::null_mut();
            let opts = match cfg.video_bitrate {
                Some(bps) => {
                    // target bitrate mode (item 9)
                    unsafe { (*ctx).bit_rate = bps as i64 };
                    spec.bitrate_options(bps)
                }
                None => spec.quality_options(cfg.quality),
            };
            for (k, v) in &opts {
                set(&mut vopts, k, v);
            }
            let r = unsafe { (api.avcodec_open2)(ctx, codec, &mut vopts) };
            unsafe { (api.av_dict_free)(&mut vopts) };
            if r >= 0 {
                report.encoder = name.to_string();
                report.hardware = spec.hardware;
                report.pix_fmt = match spec.pix {
                    crate::encoders::PixFmt::Yuv420p => "yuv420p",
                    crate::encoders::PixFmt::Nv12 => "nv12",
                }
                .to_string();
                report.options = opts.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
                report.device = device.as_ref().map(|d| d.3.clone());
                opened = Some((name.to_string(), ctx, av_pix(spec.pix)));
                hw_opened = device;
                break;
            }
            report.tried.push((name.to_string(), r));
            last_err = if device.is_some() { format!("{name}: encoder: code {r}") } else { format!("{name}: code {r}") };
            let mut ctx = ctx;
            unsafe { (api.avcodec_free_context)(&mut ctx) };
            if let Some((hw, mut dev, mut frames, _)) = device {
                unsafe {
                    (hw.av_buffer_unref)(&mut frames);
                    (hw.av_buffer_unref)(&mut dev);
                }
            }
        }
        unsafe { (api.av_log_set_level)(16) }; // back to AV_LOG_ERROR
        let (enc_name, venc, vpix) = opened.ok_or_else(|| {
            FfError::Av(format!(
                "no video encoder could be opened (last error: {last_err})"
            ))
        })?;
        let enc_name = enc_name.as_str();

        // --- aac audio encoder ---
        let aac = CString::new("aac").unwrap();
        let acodec = unsafe { (api.avcodec_find_encoder_by_name)(aac.as_ptr()) };
        if acodec.is_null() {
            return Err(FfError::Av("system FFmpeg has no AAC encoder".into()));
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
            return Err(averr(api, "opening AAC", r));
        }
        let frame_size = {
            let fs = unsafe { (*aenc).frame_size };
            if fs > 0 { fs as usize } else { 1024 }
        };

        // --- streams ---
        let vst = unsafe { (api.avformat_new_stream)(fmt, core::ptr::null()) };
        let ast = unsafe { (api.avformat_new_stream)(fmt, core::ptr::null()) };
        if vst.is_null() || ast.is_null() {
            return Err(FfError::Av("failed to create streams".into()));
        }
        unsafe {
            (*vst).time_base = tb(1, cfg.fps as c_int);
            (*ast).time_base = tb(1, AUDIO_SAMPLE_RATE as c_int);
            (api.avcodec_parameters_from_context)((*vst).codecpar, venc);
            (api.avcodec_parameters_from_context)((*ast).codecpar, aenc);
        }
        let (vstream_idx, astream_idx) = unsafe { ((*vst).index, (*ast).index) };

        // --- opening the file and header (+faststart) ---
        let r = unsafe { (api.avio_open)(&mut (*fmt).pb, path.as_ptr(), AVIO_FLAG_WRITE) };
        if r < 0 {
            return Err(averr(api, "creating file", r));
        }
        let mut mopts: *mut ffi::AVDictionary = core::ptr::null_mut();
        set(&mut mopts, "movflags", "+faststart");
        let r = unsafe { (api.avformat_write_header)(fmt, &mut mopts) };
        unsafe { (api.av_dict_free)(&mut mopts) };
        if r < 0 {
            return Err(averr(api, "writing header", r));
        }

        // --- working buffers ---
        let vframe = unsafe { (api.av_frame_alloc)() };
        unsafe {
            (*vframe).format = vpix;
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
        // a device encoder gets each NV12 frame uploaded to a surface (D4)
        let hwframe = if hw_opened.is_some() { unsafe { (api.av_frame_alloc)() } } else { core::ptr::null_mut() };
        let (hw, hw_device, hw_frames) = match &hw_opened {
            Some((hw, dev, frames, _)) => (Some(*hw), *dev, *frames),
            None => (None, core::ptr::null_mut(), core::ptr::null_mut()),
        };
        let sws = unsafe {
            (api.sws_getContext)(
                width,
                height,
                ffi::AV_PIX_FMT_RGBA,
                width,
                height,
                vpix,
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
            report,
            bytes: [0, 0],
            hw,
            hw_device,
            hw_frames,
            hwframe,
        })
    }

    pub fn encoder_name(&self) -> &str {
        &self.encoder_name
    }

    pub fn open_report(&self) -> &crate::OpenReport {
        &self.report
    }

    pub fn bytes(&self) -> (u64, u64) {
        (self.bytes[0], self.bytes[1])
    }

    pub fn write_video(&mut self, rgba: &[u8]) -> Result<(), FfError> {
        let api = self.api;
        let expected = self.width as usize * self.height as usize * 4;
        if rgba.len() != expected {
            return Err(FfError::Av(format!(
                "frame is {} bytes, expected {expected}",
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
            // a device encoder: the NV12 frame goes to a surface of the frames context (D4)
            if let Some(hw) = self.hw {
                let r = (hw.av_hwframe_get_buffer)(self.hw_frames, self.hwframe, 0);
                if r < 0 {
                    return Err(averr(api, "getting a VAAPI surface", r));
                }
                let r = (hw.av_hwframe_transfer_data)(self.hwframe, self.vframe, 0);
                if r < 0 {
                    (api.av_frame_unref)(self.hwframe);
                    return Err(averr(api, "uploading a frame to the surface", r));
                }
                (*self.hwframe).pts = self.video_pts;
            }
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
            let f = if self.hw.is_some() { self.hwframe } else { self.vframe };
            (self.venc, f, self.vstream_idx, tb(1, self.fps))
        } else {
            (self.aenc, self.aframe, self.astream_idx, tb(1, AUDIO_SAMPLE_RATE as c_int))
        };
        // ffmpeg-9: the frame says how long it lasts — one tick of its encoder time base for
        // video, `frame_size` samples for audio. Without it the LAST video sample was written
        // with zero duration on every major (the track ended at that frame's start); FFmpeg 6–8
        // kept the frame only because the edit list was rounded up to milliseconds, FFmpeg 9
        // writes an exact one and the frame was cut off (t33: 59 of 60).
        unsafe { (*frame).duration = if video { 1 } else { self.frame_size as i64 } };
        // EAGAIN = “encoder is full, frame NOT accepted” — a retry is required
        // after draining the queue. Previously the frame was silently LOST while the PTS grid
        // continued: a video of the same length with “jumps” (item 9 — frame drops
        // in nvenc with a deep queue; for audio — a hole of 1024
        // samples with an A/V shift).
        let mut attempts = 0u32;
        loop {
            let r = unsafe { (api.avcodec_send_frame)(enc, frame) };
            if r >= 0 {
                // the encoder holds its own reference to the surface
                if video && self.hw.is_some() {
                    unsafe { (api.av_frame_unref)(frame) };
                }
                break;
            }
            if r != AVERROR_EAGAIN {
                return Err(averr(api, "encoder send", r));
            }
            attempts += 1;
            if attempts > 1000 {
                return Err(FfError::Av(
                    "encoder does not accept frames (1000 attempts on EAGAIN)".into(),
                ));
            }
            // free space in the encoder queue and repeat the send
            self.drain(enc, stream_idx, tb_src)?;
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
                return Err(averr(api, "encoder receive", r));
            }
            unsafe {
                // an encoder that does not carry the frame's duration over (delayed ones may
                // not) still writes a packet that lasts one frame — see `encode`
                if (*self.pkt).duration == 0 {
                    let video = stream_idx == self.vstream_idx;
                    (*self.pkt).duration = if video { 1 } else { self.frame_size as i64 };
                }
                let st = &**(*self.fmt).streams.add(stream_idx as usize);
                (api.av_packet_rescale_ts)(self.pkt, tb_src, st.time_base);
                (*self.pkt).stream_index = stream_idx;
                // the packet's bytes, per stream, before the muxer takes them (D2)
                let which = if stream_idx == self.vstream_idx { 0 } else { 1 };
                self.bytes[which] += (*self.pkt).size.max(0) as u64;
                let r = (api.av_interleaved_write_frame)(self.fmt, self.pkt);
                (api.av_packet_unref)(self.pkt);
                if r < 0 {
                    return Err(averr(api, "writing packet", r));
                }
            }
        }
    }

    /// Flush the tails, close the file.
    pub fn finish(mut self) -> Result<(), FfError> {
        let api = self.api;
        // pad the incomplete audio frame with silence
        if !self.audio_fifo.is_empty() {
            let need = self.frame_size * AUDIO_CHANNELS - self.audio_fifo.len();
            self.audio_fifo.extend(std::iter::repeat_n(0.0, need));
            let fifo = std::mem::take(&mut self.audio_fifo);
            self.write_audio(&fifo)?;
        }
        // flush the encoders
        unsafe { (api.avcodec_send_frame)(self.venc, core::ptr::null()) };
        self.drain(self.venc, self.vstream_idx, tb(1, self.fps))?;
        unsafe { (api.avcodec_send_frame)(self.aenc, core::ptr::null()) };
        self.drain(self.aenc, self.astream_idx, tb(1, AUDIO_SAMPLE_RATE as c_int))?;
        let r = unsafe { (api.av_write_trailer)(self.fmt) };
        if r < 0 {
            return Err(averr(api, "finalizing file", r));
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
            if !self.hwframe.is_null() {
                (api.av_frame_free)(&mut self.hwframe);
            }
            (api.av_frame_free)(&mut self.aframe);
            (api.av_packet_free)(&mut self.pkt);
            (api.avcodec_free_context)(&mut self.venc);
            (api.avcodec_free_context)(&mut self.aenc);
            // D4: frames → encoder context → frames context → device
            if let Some(hw) = self.hw {
                (hw.av_buffer_unref)(&mut self.hw_frames);
                (hw.av_buffer_unref)(&mut self.hw_device);
            }
            if !self.fmt.is_null() {
                (api.avio_closep)(&mut (*self.fmt).pb);
                (api.avformat_free_context)(self.fmt);
                self.fmt = core::ptr::null_mut();
            }
        }
    }
}
