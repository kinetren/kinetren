//! Video encoder candidates per OS (change windows-build, capability export).
//!
//! The cascade is tried in order with a REAL `avcodec_open2` (export spec): having an encoder
//! in the build does not mean it works. The list differs per OS — VAAPI exists only on Linux;
//! on Windows AMD (`h264_amf`) and every machine (`h264_mf`, Media Foundation) have their own,
//! and without them a FFmpeg build lacking libx264 fell back to `mpeg4`. `h264_mf` comes after
//! libx264: its quality is worse. QSV wants NV12, not YUV420P (it did not open at all).
//! VAAPI accepts only its own surfaces: it needs a device and a frames context, and the frames
//! are uploaded from NV12 (hw-encode-context D2).
//!
//! Pure data, so t858 checks the lists of BOTH OSes on any machine.

use crate::search::Os;

/// Pixel format the encoder is fed with (swscale converts RGBA into it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixFmt {
    Yuv420p,
    Nv12,
}

/// The context a candidate needs besides its codec context (hw-encode-context D2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HwKind {
    /// System-memory frames in `pix` — software encoders, and the hardware ones whose
    /// library uploads by itself (NVENC, AMF, Media Foundation, QSV with NV12).
    None,
    /// A VAAPI device and a frames context of VAAPI surfaces; `pix` is the software format
    /// each frame is converted to before it is uploaded.
    Vaapi,
}

/// One candidate of the cascade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderSpec {
    pub name: &'static str,
    pub hardware: bool,
    pub pix: PixFmt,
    pub ctx: HwKind,
}

impl EncoderSpec {
    /// The size of the frames context of a device candidate for an export of `width`×`height`
    /// (the encoder's even size); `None` — the candidate needs no frames context.
    pub fn frames_size(&self, width: u32, height: u32) -> Option<(u32, u32)> {
        match self.ctx {
            HwKind::None => None,
            HwKind::Vaapi => Some((width.max(2) & !1, height.max(2) & !1)),
        }
    }

    /// Encoder options for the quality mode (`quality` on the CRF-like 0..51 scale, lower =
    /// better — the scale of `MuxConfig::quality`).
    pub fn quality_options(&self, quality: u32) -> Vec<(&'static str, String)> {
        let q = quality.to_string();
        match self.name {
            "libx264" => vec![("preset", "veryfast".into()), ("crf", q)],
            "h264_nvenc" => vec![("preset", "p4".into()), ("rc", "vbr".into()), ("cq", q)],
            "h264_amf" => vec![("rc", "cqp".into()), ("qp_i", q.clone()), ("qp_p", q)],
            "h264_qsv" => vec![("global_quality", q)],
            "h264_vaapi" => vec![("qp", q)],
            // Media Foundation: 0..100, higher = better
            "h264_mf" => vec![
                ("rate_control", "quality".into()),
                ("quality", (100u32.saturating_sub(quality * 2)).clamp(1, 100).to_string()),
            ],
            // mpeg4: the codec's own defaults (the last resort)
            _ => vec![],
        }
    }

    /// Encoder options for the target-bitrate mode (the bitrate itself goes into the context).
    pub fn bitrate_options(&self, bps: u32) -> Vec<(&'static str, String)> {
        match self.name {
            "libx264" => vec![
                ("preset", "veryfast".into()),
                ("maxrate", (bps as u64 * 3 / 2).to_string()),
                ("bufsize", (bps as u64 * 2).to_string()),
            ],
            "h264_nvenc" => vec![("preset", "p4".into()), ("rc", "vbr".into())],
            "h264_amf" => vec![("rc", "vbr_peak".into())],
            "h264_mf" => vec![("rate_control", "u_vbr".into())],
            // without a rate control the bitrate mode silently used the driver's default (D7)
            "h264_vaapi" => vec![("rc_mode", "VBR".into())],
            _ => vec![],
        }
    }
}

const fn hw(name: &'static str, pix: PixFmt) -> EncoderSpec {
    EncoderSpec { name, hardware: true, pix, ctx: HwKind::None }
}
const fn vaapi(name: &'static str) -> EncoderSpec {
    EncoderSpec { name, hardware: true, pix: PixFmt::Nv12, ctx: HwKind::Vaapi }
}
const fn sw(name: &'static str) -> EncoderSpec {
    EncoderSpec { name, hardware: false, pix: PixFmt::Yuv420p, ctx: HwKind::None }
}

/// The cascade of `os`, in order of preference.
pub fn video_encoder_candidates(os: Os) -> Vec<EncoderSpec> {
    match os {
        Os::Linux => vec![
            hw("h264_nvenc", PixFmt::Yuv420p),
            vaapi("h264_vaapi"),
            hw("h264_qsv", PixFmt::Nv12),
            sw("libx264"),
            sw("mpeg4"),
        ],
        Os::Windows => vec![
            hw("h264_nvenc", PixFmt::Yuv420p),
            hw("h264_amf", PixFmt::Yuv420p),
            hw("h264_qsv", PixFmt::Nv12),
            sw("libx264"),
            hw("h264_mf", PixFmt::Yuv420p),
            sw("mpeg4"),
        ],
    }
}
