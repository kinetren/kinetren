// In-process muxer (MP4/MOV/MKV/WebM; the codecs of `crate::format`, change render-settings). INCLUDED into each version
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

/// The cascade of `codec` on this OS.
fn video_encoders(codec: crate::format::VideoCodec) -> Vec<crate::encoders::EncoderSpec> {
    crate::encoders::codec_candidates(crate::search::Os::NATIVE, codec)
}

/// The video's time base (found-defects 823): the caller's, else `den/num` of the frame rate — one tick a
/// frame, 1001/24000 for 23.976.
fn video_time_base(cfg: &crate::MuxConfig) -> ffi::AVRational {
    match cfg.time_base {
        Some((n, d)) => tb(n, d),
        None => tb(cfg.frame_rate.den.max(1) as c_int, cfg.frame_rate.num.max(1) as c_int),
    }
}

/// One frame's length in the video's time base, at least one tick.
fn frame_ticks(cfg: &crate::MuxConfig) -> i64 {
    let b = video_time_base(cfg);
    let (rn, rd) = (cfg.frame_rate.num.max(1) as i64, cfg.frame_rate.den.max(1) as i64);
    // a second of the base is b.den/b.num ticks; a frame lasts rd/rn of a second
    let (n, d) = (b.den as i64 * rd, b.num as i64 * rn);
    ((n + d / 2) / d.max(1)).max(1)
}

fn av_pix(p: crate::encoders::PixFmt) -> ffi::AVPixelFormat {
    match p {
        crate::encoders::PixFmt::Yuv420p => ffi::AV_PIX_FMT_YUV420P,
        crate::encoders::PixFmt::Nv12 => ffi::AV_PIX_FMT_NV12,
        crate::encoders::PixFmt::Yuv420p10 => ffi::AV_PIX_FMT_YUV420P10LE,
        crate::encoders::PixFmt::P010 => ffi::AV_PIX_FMT_P010LE,
        crate::encoders::PixFmt::Yuv422p10 => ffi::AV_PIX_FMT_YUV422P10LE,
    }
}

/// The sample format an audio encoder is fed (render-settings D7).
fn audio_sample_fmt(a: crate::format::AudioCodec) -> ffi::AVSampleFormat {
    use crate::format::AudioCodec;
    match a {
        AudioCodec::Aac | AudioCodec::None => ffi::AV_SAMPLE_FMT_FLTP,
        AudioCodec::Opus => ffi::AV_SAMPLE_FMT_FLT,
        AudioCodec::Pcm16 => ffi::AV_SAMPLE_FMT_S16,
        AudioCodec::Pcm24 | AudioCodec::Flac => ffi::AV_SAMPLE_FMT_S32,
    }
}

/// `x` scaled to an integer of `full` steps, rounded half away from zero without libm
/// (invariant 11: a frozen path never calls the platform's libm).
fn quantize(x: f32, full: f32) -> i32 {
    let y = x.clamp(-1.0, 1.0) * full;
    (if y >= 0.0 { y + 0.5 } else { y - 0.5 }) as i32
}

/// What this FFmpeg can write, for the window (D2): a container's muxer, a codec's encoders (a
/// device candidate only with its device, as `available_encoders`), an audio encoder.
pub fn availability(api: &'static Api) -> crate::format::Availability {
    use crate::format::{AudioCodec, Container, VideoCodec};
    let has = |name: &str| {
        let c = CString::new(name).unwrap();
        !unsafe { (api.avcodec_find_encoder_by_name)(c.as_ptr()) }.is_null()
    };
    let containers = Container::ALL
        .into_iter()
        .filter(|c| {
            let name = CString::new(c.av_name()).unwrap();
            let mut fmt: *mut ffi::AVFormatContext = core::ptr::null_mut();
            let r = unsafe {
                (api.avformat_alloc_output_context2)(&mut fmt, core::ptr::null(), name.as_ptr(), core::ptr::null())
            };
            if !fmt.is_null() {
                unsafe { (api.avformat_free_context)(fmt) };
            }
            r >= 0 && !fmt.is_null()
        })
        .collect();
    crate::format::Availability {
        containers,
        video: VideoCodec::ALL.into_iter().map(|v| (v, codec_encoders(api, v))).collect(),
        audio: AudioCodec::ALL.into_iter().filter(|a| a.encoder().is_some_and(has)).collect(),
    }
}

/// Read back what a written file holds (t2802).
pub fn probe_output(api: &'static Api, path: &str) -> Result<crate::format::OutputProbe, FfError> {
    let cpath = CString::new(path).map_err(|_| FfError::Av("path contains NUL".into()))?;
    let mut fmt: *mut ffi::AVFormatContext = core::ptr::null_mut();
    let r = unsafe { (api.avformat_open_input)(&mut fmt, cpath.as_ptr(), core::ptr::null(), core::ptr::null_mut()) };
    if r < 0 {
        return Err(averr(api, "opening the output", r));
    }
    let mut out = crate::format::OutputProbe::default();
    let codec_name = |id: ffi::AVCodecID| -> String {
        match id {
            ffi::AV_CODEC_ID_H264 => "h264",
            ffi::AV_CODEC_ID_HEVC => "hevc",
            ffi::AV_CODEC_ID_AV1 => "av1",
            ffi::AV_CODEC_ID_VP9 => "vp9",
            ffi::AV_CODEC_ID_PRORES => "prores",
            ffi::AV_CODEC_ID_MPEG4 => "mpeg4",
            ffi::AV_CODEC_ID_AAC => "aac",
            ffi::AV_CODEC_ID_OPUS => "opus",
            ffi::AV_CODEC_ID_FLAC => "flac",
            ffi::AV_CODEC_ID_PCM_S16LE => "pcm_s16le",
            ffi::AV_CODEC_ID_PCM_S24LE => "pcm_s24le",
            _ => "other",
        }
        .to_string()
    };
    let dict = |m: *mut ffi::AVDictionary, key: &str| -> Option<String> {
        let k = CString::new(key).unwrap();
        let e = unsafe { (api.av_dict_get)(m, k.as_ptr(), core::ptr::null(), 0) };
        (!e.is_null()).then(|| unsafe { core::ffi::CStr::from_ptr((*e).value) }.to_string_lossy().into_owned())
    };
    let result = (|| unsafe {
        let r = (api.avformat_find_stream_info)(fmt, core::ptr::null_mut());
        if r < 0 {
            return Err(averr(api, "reading the output's streams", r));
        }
        out.format = core::ffi::CStr::from_ptr((*(*fmt).iformat).name).to_string_lossy().into_owned();
        out.title = dict((*fmt).metadata, "title");
        out.comment = dict((*fmt).metadata, "comment");
        let mut vidx = -1;
        let mut vtb = tb(1, 1);
        for i in 0..(*fmt).nb_streams as usize {
            let st = *(*fmt).streams.add(i);
            let par = (*st).codecpar;
            if (*par).codec_type == ffi::AVMEDIA_TYPE_VIDEO && vidx < 0 {
                vidx = i as c_int;
                vtb = (*st).time_base;
                out.video_codec = codec_name((*par).codec_id);
                out.profile = (*par).profile;
                let desc = (api.av_pix_fmt_desc_get)((*par).format);
                if !desc.is_null() {
                    out.pix_fmt = core::ffi::CStr::from_ptr((*desc).name).to_string_lossy().into_owned();
                }
                out.width = (*par).width as u32;
                out.height = (*par).height as u32;
                let fr = (*st).avg_frame_rate;
                let fr = if fr.num > 0 { fr } else { (*st).r_frame_rate };
                out.fps = (fr.num, fr.den);
                out.color = ((*par).color_space as u32, (*par).color_range as u32, (*par).color_primaries as u32, (*par).color_trc as u32);
                out.chroma_location = (*par).chroma_location as u32;
                out.video_tag = (*par).codec_tag;
            } else if (*par).codec_type == ffi::AVMEDIA_TYPE_AUDIO && out.audio_codec.is_none() {
                out.audio_codec = Some(codec_name((*par).codec_id));
                out.audio_sample_fmt = (*par).format;
                out.audio_bits = (*par).bits_per_raw_sample.max((*par).bits_per_coded_sample);
            }
        }
        let pkt = (api.av_packet_alloc)();
        let mut dur: i64 = 0;
        while (api.av_read_frame)(fmt, pkt) >= 0 {
            if (*pkt).stream_index == vidx {
                out.packets.push(((*pkt).pts, (*pkt).dts, ((*pkt).flags & 1) != 0, (*pkt).size.max(0) as u32));
                dur += (*pkt).duration;
            }
            (api.av_packet_unref)(pkt);
        }
        let mut pkt = pkt;
        (api.av_packet_free)(&mut pkt);
        out.video_secs = if dur > 0 {
            dur as f64 * vtb.num as f64 / vtb.den.max(1) as f64
        } else if out.fps.0 > 0 {
            out.packets.len() as f64 * out.fps.1 as f64 / out.fps.0 as f64
        } else {
            0.0
        };
        Ok(())
    })();
    unsafe { (api.avformat_close_input)(&mut fmt) };
    result.map(|_| out)
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
            crate::hw_device_opened();
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

/// Open `node` as a VAAPI device here: the driver's vendor, or why not.
pub fn hw_open_device(api: &'static Api, node: &str) -> Result<String, String> {
    let hw = hw_api().map_err(|m| format!("hardware context functions missing: {m}"))?;
    let c = CString::new(node).map_err(|_| "device path contains NUL".to_string())?;
    let mut dev: *mut ffi::AVBufferRef = core::ptr::null_mut();
    unsafe { (api.av_log_set_level)(8) };
    let r = unsafe {
        (hw.av_hwdevice_ctx_create)(&mut dev, ffi::AV_HWDEVICE_TYPE_VAAPI, c.as_ptr(), core::ptr::null_mut(), 0)
    };
    unsafe { (api.av_log_set_level)(16) };
    if r >= 0 && !dev.is_null() {
        crate::hw_device_opened();
        let vendor = vaapi_vendor(dev);
        unsafe { (hw.av_buffer_unref)(&mut dev) };
        Ok(vendor)
    } else {
        Err(format!("code {r}"))
    }
}

/// The hardware section of the report (media-index): the table, and each render node — asked in
/// a throwaway process where there is one, so this report outlives the driver it is about.
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
        for node in all {
            let v = match crate::hw_trial::ask_device(&node) {
                crate::hw_trial::Verdict::Passed(vendor) => Ok(vendor),
                crate::hw_trial::Verdict::Refused(why) => Err(why),
                crate::hw_trial::Verdict::NotHosted => hw_open_device(api, &node),
            };
            nodes.push((node, v));
        }
    }
    crate::HwReport { functions, nodes, linux }
}

/// Encoders available in the system (for the UI): present in this FFmpeg; a device candidate
/// only with a device that opens, named (D6) — the encoder itself is not opened here.
pub fn available_encoders(api: &'static Api) -> Vec<crate::AvailableEncoder> {
    codec_encoders(api, crate::format::VideoCodec::H264)
}

/// The encoders of `codec` the settings may offer (render-settings D2): the same rule per codec.
pub fn codec_encoders(api: &'static Api, codec: crate::format::VideoCodec) -> Vec<crate::AvailableEncoder> {
    let mut out = Vec::new();
    for e in video_encoders(codec) {
        let c = CString::new(e.name).unwrap();
        if unsafe { (api.avcodec_find_encoder_by_name)(c.as_ptr()) }.is_null() {
            continue;
        }
        let device = match e.ctx {
            crate::encoders::HwKind::None => None,
            crate::encoders::HwKind::Vaapi => {
                let Ok(hw) = hw_api() else { continue };
                let mut device = None;
                let mut in_process = false;
                for path in crate::vaapi_device_paths() {
                    match crate::hw_trial::ask_device(&path) {
                        crate::hw_trial::Verdict::Passed(_) => {
                            device = Some(path);
                            break;
                        }
                        crate::hw_trial::Verdict::Refused(_) => {}
                        crate::hw_trial::Verdict::NotHosted => in_process = true,
                    }
                }
                if in_process {
                    unsafe { (api.av_log_set_level)(8) };
                    let dev = open_vaapi_device(hw);
                    unsafe { (api.av_log_set_level)(16) };
                    if let Ok((mut d, path)) = dev {
                        unsafe { (hw.av_buffer_unref)(&mut d) };
                        device = Some(path);
                    }
                }
                if device.is_none() {
                    continue;
                }
                device
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
    /// The muxer's own RGBA → encoder-format conversion (`write_video`); a pipelined export
    /// converts on its own `VideoConverter`s and hands the frames to `write_video_frame`.
    conv: VideoConverter,
    vframe: VideoFrame,
    aframe: *mut ffi::AVFrame,
    pkt: *mut ffi::AVPacket,
    width: c_int,
    height: c_int,
    /// The video encoder's time base and one frame's ticks in it (found-defects 823): `den/num` of the
    /// rate and 1, or the caller's base and the frame's length in it.
    vtb: ffi::AVRational,
    frame_ticks: i64,
    video_pts: i64,
    audio_pts: i64,
    /// Audio accumulator up to the encoder frame size (aac = 1024).
    audio_fifo: Vec<f32>,
    /// The sample format the audio encoder is fed (D7); `aenc` is null with no audio stream.
    audio_fmt: ffi::AVSampleFormat,
    /// The conversion's matrix and range (D6), for every converter this muxer makes.
    colour: (crate::format::Matrix, crate::format::Range),
    /// The encoder's input format as the closed set names it (export-host-copy D3).
    input_pix: crate::encoders::PixFmt,
    frame_size: usize,
    encoder_name: String,
    report: crate::OpenReport,
    /// Encoded packet bytes written so far: [video, audio].
    bytes: [u64; 2],
    /// A device encoder's surface get and upload so far, ns (export-host-copy D1): apart from the
    /// encode, so a trace says which binds.
    surface_upload_ns: u64,
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
        let form = &cfg.format;
        // a container that cannot hold the asked codecs is refused before a file exists (D2)
        if let Some(why) = form.refusal() {
            return Err(FfError::Av(why));
        }
        let mut fmt: *mut ffi::AVFormatContext = core::ptr::null_mut();
        let cname = CString::new(form.container.av_name()).unwrap();
        let r = unsafe {
            (api.avformat_alloc_output_context2)(
                &mut fmt,
                core::ptr::null(),
                cname.as_ptr(),
                path.as_ptr(),
            )
        };
        if r < 0 || fmt.is_null() {
            return Err(averr(api, &format!("creating the {} context", form.container.label()), r));
        }
        // the rate mode actually asked: a bitrate without a mode is the former VBR; an intra-only
        // codec has none (D4)
        use crate::format::RateMode;
        let intra = form.video.intra_only();
        let (rate, bps) = match (intra, form.rate, cfg.video_bitrate) {
            (true, _, _) | (false, _, None) => (RateMode::Quality, 0),
            (false, RateMode::Quality, Some(b)) => (RateMode::Vbr, b),
            (false, r, Some(b)) => (r, b),
        };

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
        let mut opened: Option<(String, *mut ffi::AVCodecContext, crate::encoders::PixFmt)> = None;
        // the device and frames contexts of an opened device candidate (D3)
        let mut hw_opened: Option<(&'static HwApi, *mut ffi::AVBufferRef, *mut ffi::AVBufferRef, String)> = None;
        let mut last_err = String::new();
        // every candidate that exists here and failed, with its code, and the opened one's facts
        let mut report = crate::OpenReport::default();
        let cascade = video_encoders(form.video);
        // a forced name outside this system's cascade is never tried: say so, or the error
        // read "last error: " with nothing after it (h264_vaapi on Windows, CI of 94e36e9e)
        if let Some(pref) = &cfg.force_encoder
            && !cascade.iter().any(|s| s.name == pref.as_str())
        {
            let names: Vec<&str> = cascade.iter().map(|s| s.name).collect();
            last_err = format!("{pref}: not an encoder this system tries ({})", names.join(", "));
        }
        for spec in cascade {
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
            let skip = |report: &mut crate::OpenReport, last_err: &mut String, why: String| {
                *last_err = format!("{name}: {why}");
                report.skipped.push((name.to_string(), why));
            };
            // WHAT IT CANNOT DO IS NAMED (render-settings D3): the depth and the rate mode asked
            let pix = match spec.input(form) {
                Ok(p) => p,
                Err(why) => {
                    skip(&mut report, &mut last_err, why.to_string());
                    continue;
                }
            };
            if !spec.has_rate(rate) {
                skip(&mut report, &mut last_err, "no CBR".to_string());
                continue;
            }
            // A DRIVER FIRST MEETS A THROWAWAY PROCESS (D10): a fault in it ends that process, not
            // this render. Without a render node there is no driver to load and nothing to ask.
            if spec.trial
                && !crate::vaapi_device_paths().is_empty()
                && let crate::hw_trial::Verdict::Refused(why) = crate::hw_trial::ask_encoder(name, cfg)
            {
                skip(&mut report, &mut last_err, why);
                continue;
            }
            // A DEVICE CANDIDATE IS ONE STEP: device → frames → open (D3). What fails before
            // the open skips it WITHOUT `avcodec_open2` — the encoder's failed-init path stays
            // out of the cascade (ktprompts v26.10 T16).
            let mut device: Option<(&'static HwApi, *mut ffi::AVBufferRef, *mut ffi::AVBufferRef, String)> = None;
            if spec.ctx == crate::encoders::HwKind::Vaapi {
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
                    (*fc).sw_format = av_pix(pix);
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
                (*ctx).time_base = video_time_base(cfg);
                (*ctx).framerate = tb(cfg.frame_rate.num as c_int, cfg.frame_rate.den.max(1) as c_int);
                (*ctx).pix_fmt = av_pix(pix);
                if let Some((hw, _, frames, _)) = &device {
                    (*ctx).pix_fmt = ffi::AV_PIX_FMT_VAAPI;
                    (*ctx).hw_frames_ctx = (hw.av_buffer_ref)(*frames);
                }
                // the keyframe interval and the B-frames asked (D1); intra-only: every frame a key
                (*ctx).gop_size = if intra { 1 } else { form.gop_frames(cfg.frame_rate.rounded()) as c_int };
                (*ctx).max_b_frames = if intra { 0 } else { form.b_frames as c_int };
                // the tags state the conversion (D6), the chroma's place too (export-host-copy D2: the
                // conversion sites it left, as a player assumes of an untagged stream)
                (*ctx).chroma_sample_location = ffi::AVCHROMA_LOC_LEFT;
                (*ctx).colorspace = match form.matrix_at(width as u32, height as u32) {
                    crate::format::Matrix::Bt601 => ffi::AVCOL_SPC_SMPTE170M,
                    crate::format::Matrix::Bt709 => ffi::AVCOL_SPC_BT709,
                };
                (*ctx).color_range = match form.range {
                    crate::format::Range::Limited => ffi::AVCOL_RANGE_MPEG,
                    crate::format::Range::Full => ffi::AVCOL_RANGE_JPEG,
                };
                (*ctx).color_primaries = ffi::AVCOL_PRI_BT709;
                (*ctx).color_trc = ffi::AVCOL_TRC_BT709;
                if global_header {
                    (*ctx).flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
                }
            }
            let mut vopts: *mut ffi::AVDictionary = core::ptr::null_mut();
            if rate != RateMode::Quality {
                // target bitrate mode (item 9); the maximum and the buffer are the options' (D4)
                unsafe { (*ctx).bit_rate = bps as i64 };
            }
            let opts = spec.options(form, rate, cfg.quality, bps);
            // the encoder's own threads (export-throughput D3): the AVCodecContext default is ONE
            // thread, and libx264 encoded a 1080p export on one core of 32. `auto` lets the
            // encoder size itself; a device encoder ignores it.
            let mut opts = opts;
            opts.push(("threads", if cfg.threads == 0 { "auto".to_string() } else { cfg.threads.to_string() }));
            for (k, v) in &opts {
                set(&mut vopts, k, v);
            }
            let r = unsafe { (api.avcodec_open2)(ctx, codec, &mut vopts) };
            unsafe { (api.av_dict_free)(&mut vopts) };
            if r >= 0 {
                report.encoder = name.to_string();
                report.hardware = spec.hardware;
                report.pix_fmt = pix.name().to_string();
                report.options = opts.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
                report.device = device.as_ref().map(|d| d.3.clone());
                opened = Some((name.to_string(), ctx, pix));
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
        let input_pix = vpix;
        let vpix = av_pix(vpix);

        // --- audio encoder of the asked codec, its own sample format (D7); none — no stream ---
        let audio_fmt = audio_sample_fmt(form.audio);
        let mut aenc: *mut ffi::AVCodecContext = core::ptr::null_mut();
        let mut frame_size = 1024;
        if let Some(aname) = form.audio.encoder() {
            let cn = CString::new(aname).unwrap();
            let acodec = unsafe { (api.avcodec_find_encoder_by_name)(cn.as_ptr()) };
            if acodec.is_null() {
                let mut v = venc;
                unsafe { (api.avcodec_free_context)(&mut v) };
                return Err(FfError::Av(format!("system FFmpeg has no {} encoder ({aname})", form.audio.label())));
            }
            aenc = unsafe { (api.avcodec_alloc_context3)(acodec) };
            unsafe {
                (*aenc).sample_rate = AUDIO_SAMPLE_RATE as c_int;
                (*aenc).sample_fmt = audio_fmt;
                if form.audio.has_bitrate() {
                    (*aenc).bit_rate = cfg.audio_bitrate as i64;
                }
                if matches!(form.audio, crate::format::AudioCodec::Pcm24 | crate::format::AudioCodec::Flac) {
                    (*aenc).bits_per_raw_sample = 24;
                }
                (*aenc).time_base = tb(1, AUDIO_SAMPLE_RATE as c_int);
                (api.av_channel_layout_default)(&mut (*aenc).ch_layout, AUDIO_CHANNELS as c_int);
                if ((*(*fmt).oformat).flags & AVFMT_GLOBALHEADER) != 0 {
                    (*aenc).flags |= AV_CODEC_FLAG_GLOBAL_HEADER;
                }
            }
            let r = unsafe { (api.avcodec_open2)(aenc, acodec, core::ptr::null_mut()) };
            if r < 0 {
                let mut v = venc;
                unsafe {
                    (api.avcodec_free_context)(&mut aenc);
                    (api.avcodec_free_context)(&mut v);
                }
                return Err(averr(api, &format!("opening {}", form.audio.label()), r));
            }
            let fs = unsafe { (*aenc).frame_size };
            if fs > 0 {
                frame_size = fs as usize;
            }
        }

        // --- streams ---
        let vst = unsafe { (api.avformat_new_stream)(fmt, core::ptr::null()) };
        let ast = if aenc.is_null() { core::ptr::null_mut() } else { unsafe { (api.avformat_new_stream)(fmt, core::ptr::null()) } };
        if vst.is_null() || (!aenc.is_null() && ast.is_null()) {
            return Err(FfError::Av("failed to create streams".into()));
        }
        unsafe {
            (*vst).time_base = video_time_base(cfg);
            // Matroska keeps a frame's time in whole milliseconds: without the stream's own rate a file of
            // 30 fps was one of 30.30 to whoever opened it (found-defects 981)
            if matches!(form.container, crate::format::Container::Mkv | crate::format::Container::Webm) {
                (*vst).avg_frame_rate = tb(cfg.frame_rate.num as c_int, cfg.frame_rate.den.max(1) as c_int);
            }
            (api.avcodec_parameters_from_context)((*vst).codecpar, venc);
            // H.265 in MP4/MOV as `hvc1`: Apple's players refuse the default `hev1` (D8)
            if form.video == crate::format::VideoCodec::H265
                && matches!(form.container, crate::format::Container::Mp4 | crate::format::Container::Mov)
            {
                (*(*vst).codecpar).codec_tag = u32::from_le_bytes(*b"hvc1");
            }
            if !ast.is_null() {
                (*ast).time_base = tb(1, AUDIO_SAMPLE_RATE as c_int);
                (api.avcodec_parameters_from_context)((*ast).codecpar, aenc);
            }
        }
        let (vstream_idx, astream_idx) = unsafe { ((*vst).index, if ast.is_null() { -1 } else { (*ast).index }) };
        // Matroska draws a random SegmentUID for every file unless asked to be bit-exact (invariant
        // 11; found by t2805). Only there: in MP4 the flag would also drop the muxer's name tag and
        // move the bytes of every former export.
        if matches!(form.container, crate::format::Container::Mkv | crate::format::Container::Webm) {
            const AVFMT_FLAG_BITEXACT: c_int = 0x0400;
            unsafe { (*fmt).flags |= AVFMT_FLAG_BITEXACT };
        }
        // file metadata; no creation time is written (invariant 11)
        for (k, v) in [("title", &form.title), ("comment", &form.comment)] {
            if !v.is_empty() {
                let (ck, cv) = (CString::new(k).unwrap(), CString::new(v.replace('\0', "")).unwrap());
                unsafe { (api.av_dict_set)(&mut (*fmt).metadata, ck.as_ptr(), cv.as_ptr(), 0) };
            }
        }

        // --- opening the file and header (+faststart) ---
        let r = unsafe { (api.avio_open)(&mut (*fmt).pb, path.as_ptr(), AVIO_FLAG_WRITE) };
        if r < 0 {
            return Err(averr(api, "creating file", r));
        }
        let mut mopts: *mut ffi::AVDictionary = core::ptr::null_mut();
        // the trial's sink cannot be read back for the rewrite
        // (a source copy, whose frames carry their own times, is read from this disk alone: the rewrite
        // would write its gigabytes twice)
        if cfg.path != crate::hw_trial::TRIAL_SINK && form.container.faststart() && cfg.time_base.is_none() {
            set(&mut mopts, "movflags", "+faststart");
        }
        let r = unsafe { (api.avformat_write_header)(fmt, &mut mopts) };
        unsafe { (api.av_dict_free)(&mut mopts) };
        if r < 0 {
            return Err(averr(api, "writing header", r));
        }

        // --- working buffers ---
        let vframe = VideoFrame::alloc(api, vpix, width, height)?;
        let aframe = unsafe { (api.av_frame_alloc)() };
        unsafe {
            (*aframe).format = audio_fmt;
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
        let colour = (form.matrix_at(width as u32, height as u32), form.range);
        let conv = VideoConverter::new(api, width, height, vpix, colour)?;
        Ok(Muxer {
            api,
            fmt,
            venc,
            aenc,
            vstream_idx,
            astream_idx,
            conv,
            vframe,
            aframe,
            pkt: unsafe { (api.av_packet_alloc)() },
            width,
            height,
            vtb: video_time_base(cfg),
            frame_ticks: frame_ticks(cfg),
            video_pts: 0,
            audio_pts: 0,
            audio_fifo: Vec::new(),
            audio_fmt,
            colour,
            input_pix,
            frame_size,
            encoder_name: enc_name.to_string(),
            report,
            bytes: [0, 0],
            surface_upload_ns: 0,
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

    pub fn surface_upload_ns(&self) -> u64 {
        self.surface_upload_ns
    }

    /// The encoder's planes, packed (export-host-copy D4): copied into the frame and encoded, no
    /// conversion here.
    pub fn write_video_planes(&mut self, planes: &[u8]) -> Result<(), FfError> {
        self.vframe.fill_packed(planes)?;
        let f = self.vframe.frame;
        self.encode_video_frame(f)
    }

    pub fn write_video(&mut self, rgba: &[u8]) -> Result<(), FfError> {
        self.conv.convert(rgba, &mut self.vframe)?;
        let f = self.vframe.frame;
        self.encode_video_frame(f)
    }

    /// A converter of this muxer's size and encoder pixel format (export-throughput D4): the
    /// pipelined export runs several, one frame each, on their own threads. The conversion is a
    /// pure function of the RGBA bytes, so which converter did a frame does not show in it.
    pub fn video_converter(&self) -> Result<VideoConverter, FfError> {
        VideoConverter::new(self.api, self.width, self.height, self.conv.pix, self.colour)
    }

    /// A frame a `VideoConverter` fills, of this muxer's size and pixel format.
    /// What the encoder takes (export-host-copy D3): its input format, the matrix and the range — the
    /// layout an exact conversion writes for it.
    pub fn input_layout(&self) -> (crate::encoders::PixFmt, crate::format::Matrix, crate::format::Range) {
        (self.input_pix, self.colour.0, self.colour.1)
    }

    pub fn video_size(&self) -> (u32, u32) {
        (self.width as u32, self.height as u32)
    }

    pub fn alloc_video_frame(&self) -> Result<VideoFrame, FfError> {
        VideoFrame::alloc(self.api, self.conv.pix, self.width, self.height)
    }

    /// One converted frame to the encoder, in order: the next pts, the device upload for a
    /// hardware encoder. The frame may be refilled after this returns.
    pub fn write_video_frame(&mut self, f: &VideoFrame) -> Result<(), FfError> {
        unsafe {
            if (*f.frame).width != self.width || (*f.frame).height != self.height || (*f.frame).format != self.conv.pix {
                return Err(FfError::Av("a converted frame of another size or format".into()));
            }
        }
        self.encode_video_frame(f.frame)
    }

    /// `write_video_frame` at `pts` in the video's time base (`MuxConfig.time_base`): a proxy keeps its
    /// source's times, variable rate included (found-defects 823, change `proxies`). Later frames written
    /// without a pts follow it a frame apart.
    pub fn write_video_frame_at(&mut self, f: &VideoFrame, pts: i64) -> Result<(), FfError> {
        self.video_pts = pts;
        self.write_video_frame(f)
    }

    fn encode_video_frame(&mut self, frame: *mut ffi::AVFrame) -> Result<(), FfError> {
        let api = self.api;
        unsafe {
            (*frame).pts = self.video_pts;
            // a device encoder: the NV12 frame goes to a surface of the frames context (D4)
            if let Some(hw) = self.hw {
                let since = std::time::Instant::now();
                let r = (hw.av_hwframe_get_buffer)(self.hw_frames, self.hwframe, 0);
                if r < 0 {
                    return Err(averr(api, "getting a VAAPI surface", r));
                }
                let r = (hw.av_hwframe_transfer_data)(self.hwframe, frame, 0);
                if r < 0 {
                    (api.av_frame_unref)(self.hwframe);
                    return Err(averr(api, "uploading a frame to the surface", r));
                }
                (*self.hwframe).pts = self.video_pts;
                self.surface_upload_ns += since.elapsed().as_nanos() as u64;
            }
        }
        self.video_pts += self.frame_ticks;
        let f = if self.hw.is_some() { self.hwframe } else { frame };
        self.encode(f, true)
    }

    pub fn write_audio(&mut self, samples: &[f32]) -> Result<(), FfError> {
        // no audio stream: the samples have nowhere to go
        if self.aenc.is_null() {
            return Ok(());
        }
        self.audio_fifo.extend_from_slice(samples);
        while self.audio_fifo.len() >= self.frame_size * AUDIO_CHANNELS {
            let n = self.frame_size * AUDIO_CHANNELS;
            unsafe {
                let src = &self.audio_fifo[..n];
                // interleaved f32 → the encoder's own format (D7)
                match self.audio_fmt {
                    ffi::AV_SAMPLE_FMT_FLTP => {
                        for ch in 0..AUDIO_CHANNELS {
                            let dst = (*self.aframe).data[ch] as *mut f32;
                            for i in 0..self.frame_size {
                                *dst.add(i) = src[i * AUDIO_CHANNELS + ch];
                            }
                        }
                    }
                    ffi::AV_SAMPLE_FMT_FLT => {
                        let dst = (*self.aframe).data[0] as *mut f32;
                        core::ptr::copy_nonoverlapping(src.as_ptr(), dst, n);
                    }
                    ffi::AV_SAMPLE_FMT_S16 => {
                        let dst = (*self.aframe).data[0] as *mut i16;
                        for (i, &x) in src.iter().enumerate() {
                            *dst.add(i) = quantize(x, 32767.0) as i16;
                        }
                    }
                    _ => {
                        // S32 holding 24 significant bits
                        let dst = (*self.aframe).data[0] as *mut i32;
                        for (i, &x) in src.iter().enumerate() {
                            *dst.add(i) = quantize(x, 8_388_607.0) << 8;
                        }
                    }
                }
                (*self.aframe).pts = self.audio_pts;
            }
            self.audio_pts += self.frame_size as i64;
            self.audio_fifo.drain(..self.frame_size * AUDIO_CHANNELS);
            let f = self.aframe;
            self.encode(f, false)?;
        }
        Ok(())
    }

    fn encode(&mut self, frame: *mut ffi::AVFrame, video: bool) -> Result<(), FfError> {
        let api = self.api;
        let (enc, stream_idx, tb_src) = if video {
            (self.venc, self.vstream_idx, self.vtb)
        } else {
            (self.aenc, self.astream_idx, tb(1, AUDIO_SAMPLE_RATE as c_int))
        };
        // ffmpeg-9: the frame says how long it lasts — one tick of its encoder time base for
        // video, `frame_size` samples for audio. Without it the LAST video sample was written
        // with zero duration on every major (the track ended at that frame's start); FFmpeg 6–8
        // kept the frame only because the edit list was rounded up to milliseconds, FFmpeg 9
        // writes an exact one and the frame was cut off (t33: 59 of 60).
        unsafe { (*frame).duration = if video { self.frame_ticks } else { self.frame_size as i64 } };
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
                    (*self.pkt).duration = if video { self.frame_ticks } else { self.frame_size as i64 };
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

    /// `finish`, and the encoded video bytes in the end.
    pub fn finish_counted(mut self) -> Result<u64, FfError> {
        self.finish_mut()?;
        Ok(self.bytes[0])
    }

    /// Flush the tails, close the file.
    pub fn finish(mut self) -> Result<(), FfError> {
        self.finish_mut()
    }

    fn finish_mut(&mut self) -> Result<(), FfError> {
        let api = self.api;
        // pad the incomplete audio frame with silence
        if !self.aenc.is_null() && !self.audio_fifo.is_empty() {
            let need = self.frame_size * AUDIO_CHANNELS - self.audio_fifo.len();
            self.audio_fifo.extend(std::iter::repeat_n(0.0, need));
            let fifo = std::mem::take(&mut self.audio_fifo);
            self.write_audio(&fifo)?;
        }
        // flush the encoders
        unsafe { (api.avcodec_send_frame)(self.venc, core::ptr::null()) };
        self.drain(self.venc, self.vstream_idx, self.vtb)?;
        if !self.aenc.is_null() {
            unsafe { (api.avcodec_send_frame)(self.aenc, core::ptr::null()) };
            self.drain(self.aenc, self.astream_idx, tb(1, AUDIO_SAMPLE_RATE as c_int))?;
        }
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

/// RGBA → the encoder's pixel format (export-throughput D4): its own `SwsContext`, so several run
/// at once on different frames. `Muxer::video_converter` makes them with the muxer's parameters.
pub struct VideoConverter {
    api: &'static Api,
    sws: *mut ffi::SwsContext,
    width: c_int,
    height: c_int,
    pix: ffi::AVPixelFormat,
}

/// The matrix and range of a conversion (render-settings D6).
type Colour = (crate::format::Matrix, crate::format::Range);

// SAFETY: the context is used by one thread at a time (`&mut self`), never shared.
unsafe impl Send for VideoConverter {}

impl VideoConverter {
    fn new(api: &'static Api, width: c_int, height: c_int, pix: ffi::AVPixelFormat, colour: Colour) -> Result<VideoConverter, FfError> {
        let sws = unsafe {
            (api.sws_getContext)(
                width,
                height,
                ffi::AV_PIX_FMT_RGBA,
                width,
                height,
                pix,
                ffi::SWS_BILINEAR as c_int,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                core::ptr::null(),
            )
        };
        if sws.is_null() {
            return Err(FfError::Av("sws_getContext failed (RGBA → encoder format)".into()));
        }
        // the default (BT.601 limited) is swscale's own: left untouched, so the former bytes stay
        if colour != Default::default() {
            const SWS_CS_ITU709: c_int = 1;
            const SWS_CS_ITU601: c_int = 5;
            let cs = match colour.0 {
                crate::format::Matrix::Bt601 => SWS_CS_ITU601,
                crate::format::Matrix::Bt709 => SWS_CS_ITU709,
            };
            let full = (colour.1 == crate::format::Range::Full) as c_int;
            unsafe {
                let table = (api.sws_getCoefficients)(cs);
                // source RGB is full range; brightness 0, contrast and saturation 1.0 (16.16)
                (api.sws_setColorspaceDetails)(sws, table, 1, table, full, 0, 1 << 16, 1 << 16);
            }
        }
        Ok(VideoConverter { api, sws, width, height, pix })
    }

    /// Convert one RGBA frame (width×height×4 bytes) into `out`.
    pub fn convert(&mut self, rgba: &[u8], out: &mut VideoFrame) -> Result<(), FfError> {
        let api = self.api;
        let expected = self.width as usize * self.height as usize * 4;
        if rgba.len() != expected {
            return Err(FfError::Av(format!("frame is {} bytes, expected {expected}", rgba.len())));
        }
        unsafe {
            // the encoder may still hold a reference to the buffer it was last sent
            let r = (api.av_frame_make_writable)(out.frame);
            if r < 0 {
                return Err(averr(api, "making the frame writable", r));
            }
            let src: [*const u8; 4] = [rgba.as_ptr(), core::ptr::null(), core::ptr::null(), core::ptr::null()];
            let src_ls: [c_int; 4] = [self.width * 4, 0, 0, 0];
            (api.sws_scale)(
                self.sws,
                src.as_ptr(),
                src_ls.as_ptr(),
                0,
                self.height,
                (*out.frame).data.as_ptr(),
                (*out.frame).linesize.as_ptr(),
            );
        }
        Ok(())
    }
}

impl Drop for VideoConverter {
    fn drop(&mut self) {
        unsafe { (self.api.sws_freeContext)(self.sws) };
    }
}

/// A frame in the encoder's pixel format (export-throughput D4), owned and reusable.
pub struct VideoFrame {
    api: &'static Api,
    frame: *mut ffi::AVFrame,
}

// SAFETY: owned exclusively; moved between the converter and the writer threads, never shared.
unsafe impl Send for VideoFrame {}

impl VideoFrame {
    fn alloc(api: &'static Api, pix: ffi::AVPixelFormat, width: c_int, height: c_int) -> Result<VideoFrame, FfError> {
        unsafe {
            let mut frame = (api.av_frame_alloc)();
            if frame.is_null() {
                return Err(FfError::Av("av_frame_alloc failed".into()));
            }
            (*frame).format = pix;
            (*frame).width = width;
            (*frame).height = height;
            let r = (api.av_frame_get_buffer)(frame, 0);
            if r < 0 {
                (api.av_frame_free)(&mut frame);
                return Err(averr(api, "allocating a video frame", r));
            }
            Ok(VideoFrame { api, frame })
        }
    }
}

impl VideoFrame {
    /// Fill the frame from the encoder format's planes packed tightly one after another, rows of the
    /// picture's width, in the order the frame takes them (export-host-copy D3): a copy row by row
    /// into the frame's own line sizes. 16-bit samples little-endian.
    pub fn fill_packed(&mut self, packed: &[u8]) -> Result<(), FfError> {
        let api = self.api;
        unsafe {
            let r = (api.av_frame_make_writable)(self.frame);
            if r < 0 {
                return Err(averr(api, "making the frame writable", r));
            }
            let f = &mut *self.frame;
            let (w, h) = (f.width as usize, f.height as usize);
            let (cw, ch2) = (w.div_ceil(2), h.div_ceil(2));
            // (row bytes, rows) per plane of the format
            let planes: Vec<(usize, usize)> = match f.format {
                x if x == ffi::AV_PIX_FMT_YUV420P => vec![(w, h), (cw, ch2), (cw, ch2)],
                x if x == ffi::AV_PIX_FMT_NV12 => vec![(w, h), (cw * 2, ch2)],
                x if x == ffi::AV_PIX_FMT_YUV420P10LE => vec![(w * 2, h), (cw * 2, ch2), (cw * 2, ch2)],
                x if x == ffi::AV_PIX_FMT_P010LE => vec![(w * 2, h), (cw * 4, ch2)],
                x if x == ffi::AV_PIX_FMT_YUV422P10LE => vec![(w * 2, h), (cw * 2, h), (cw * 2, h)],
                other => return Err(FfError::Av(format!("no packed layout for the pixel format {other}"))),
            };
            let need: usize = planes.iter().map(|(b, r)| b * r).sum();
            if packed.len() < need {
                return Err(FfError::Av(format!("planes of {} bytes, the frame takes {need}", packed.len())));
            }
            let mut at = 0;
            for (i, (row, rows)) in planes.into_iter().enumerate() {
                let dst = f.data[i];
                let ls = f.linesize[i] as usize;
                for y in 0..rows {
                    core::ptr::copy_nonoverlapping(packed.as_ptr().add(at + y * row), dst.add(y * ls), row);
                }
                at += row * rows;
            }
        }
        Ok(())
    }
}

impl Drop for VideoFrame {
    fn drop(&mut self) {
        unsafe { (self.api.av_frame_free)(&mut self.frame) };
    }
}
