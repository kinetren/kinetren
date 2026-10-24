//! Video encoder candidates per OS (change windows-build, capability export).
//!
//! The cascade is tried in order with a REAL `avcodec_open2` (export spec): having an encoder
//! in the build does not mean it works. The list differs per OS — VAAPI exists only on Linux;
//! on Windows AMD (`h264_amf`) and every machine (`h264_mf`, Media Foundation) have their own,
//! and without them a FFmpeg build lacking libx264 fell back to `mpeg4`. `h264_mf` comes after
//! libx264: its quality is worse. QSV wants NV12, not YUV420P (it did not open at all).
//!
//! Pure data, so t858 checks the lists of BOTH OSes on any machine.

use crate::search::Os;

/// Pixel format the encoder is fed with (swscale converts RGBA into it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixFmt {
    Yuv420p,
    Nv12,
}

/// One candidate of the cascade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderSpec {
    pub name: &'static str,
    pub hardware: bool,
    pub pix: PixFmt,
}

impl EncoderSpec {
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
            _ => vec![],
        }
    }
}

const fn hw(name: &'static str, pix: PixFmt) -> EncoderSpec {
    EncoderSpec { name, hardware: true, pix }
}
const fn sw(name: &'static str) -> EncoderSpec {
    EncoderSpec { name, hardware: false, pix: PixFmt::Yuv420p }
}

/// The cascade of `os`, in order of preference.
pub fn video_encoder_candidates(os: Os) -> Vec<EncoderSpec> {
    match os {
        Os::Linux => vec![
            hw("h264_nvenc", PixFmt::Yuv420p),
            hw("h264_vaapi", PixFmt::Yuv420p),
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
