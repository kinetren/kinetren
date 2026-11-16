//! Video encoder candidates per OS and per codec (changes windows-build, render-settings;
//! capability export).
//!
//! The cascade is tried in order with a REAL `avcodec_open2` (export spec): having an encoder
//! in the build does not mean it works. The list differs per OS — VAAPI exists only on Linux;
//! on Windows AMD (`*_amf`) and every machine (`*_mf`, Media Foundation) have their own,
//! and without them a FFmpeg build lacking libx264 fell back to `mpeg4`. `h264_mf` comes after
//! libx264: its quality is worse. QSV wants NV12, not YUV420P (it did not open at all).
//! VAAPI accepts only its own surfaces: it needs a device and a frames context, and the frames
//! are uploaded from NV12 (hw-encode-context D2).
//!
//! Every codec family has its own cascade (render-settings D3); a candidate states its 10-bit
//! input and its rate modes, and the muxer SKIPS one that lacks what was asked, with the reason.
//! The options are the encoder's own words for the rate mode (D4) and the speed step (D5); the
//! default speed `Fast` gives exactly the options every export had before (`veryfast`, `p4`).
//!
//! Pure data, so t858/t2801 check the lists of BOTH OSes on any machine.

use crate::format::{OutputFormat, RateMode, Speed, VideoCodec};
use crate::search::Os;

/// Pixel format the encoder is fed with (swscale converts RGBA into it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixFmt {
    Yuv420p,
    Nv12,
    /// 10-bit 4:2:0 planar, little-endian (software encoders).
    Yuv420p10,
    /// 10-bit 4:2:0 semi-planar (NVENC).
    P010,
    /// 10-bit 4:2:2 planar (ProRes).
    Yuv422p10,
}

impl PixFmt {
    pub fn name(self) -> &'static str {
        match self {
            PixFmt::Yuv420p => "yuv420p",
            PixFmt::Nv12 => "nv12",
            PixFmt::Yuv420p10 => "yuv420p10le",
            PixFmt::P010 => "p010le",
            PixFmt::Yuv422p10 => "yuv422p10le",
        }
    }
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
    /// The 10-bit input it takes, `None` — 8-bit only here (render-settings D3).
    pub pix10: Option<PixFmt>,
    pub ctx: HwKind,
    /// Opening it loads a libva driver into the process, so the first contact happens in a
    /// throwaway one (`hw_trial`, hw-encode-context D10).
    pub trial: bool,
    /// It has a constant-rate mode (every candidate has Quality and VBR).
    pub cbr: bool,
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

    pub fn has_rate(&self, r: RateMode) -> bool {
        r != RateMode::Cbr || self.cbr
    }

    /// The pixel format it is fed for `fmt`, or why it cannot take it (D3).
    pub fn input(&self, fmt: &OutputFormat) -> Result<PixFmt, &'static str> {
        match fmt.depth {
            crate::format::BitDepth::Eight => Ok(self.pix),
            crate::format::BitDepth::Ten => self.pix10.ok_or("no 10-bit input"),
        }
    }

    /// The encoder's options for the speed step (D5); empty — it has no such knob.
    pub fn speed_options(&self, speed: Speed) -> Vec<(&'static str, String)> {
        let i = speed as usize;
        let pick = |v: [&str; 5]| v[i].to_string();
        match self.name {
            "libx264" | "libx265" => vec![("preset", pick(["ultrafast", "veryfast", "medium", "slow", "veryslow"]))],
            "h264_nvenc" | "hevc_nvenc" | "av1_nvenc" => vec![("preset", pick(["p1", "p4", "p5", "p6", "p7"]))],
            "h264_qsv" | "hevc_qsv" | "av1_qsv" | "vp9_qsv" => {
                // `Fast` is the library's own default, so the former file is unchanged
                vec![("preset", pick(["veryfast", "medium", "medium", "slow", "veryslow"]))]
            }
            "h264_amf" | "hevc_amf" | "av1_amf" => vec![("quality", pick(["speed", "speed", "balanced", "quality", "quality"]))],
            "libsvtav1" => vec![("preset", pick(["12", "10", "8", "6", "4"]))],
            "libaom-av1" => vec![("cpu-used", pick(["8", "6", "4", "2", "1"]))],
            "libvpx-vp9" => vec![
                ("deadline", pick(["realtime", "good", "good", "good", "best"])),
                ("cpu-used", pick(["8", "5", "3", "1", "0"])),
                ("row-mt", "1".into()),
            ],
            _ => vec![],
        }
    }

    /// Encoder options for the quality mode (`quality` on the CRF-like 0..51 scale, lower =
    /// better — the scale of `MuxConfig::quality`), at the default speed.
    pub fn quality_options(&self, quality: u32) -> Vec<(&'static str, String)> {
        let mut o = self.speed_options(Speed::Fast);
        o.extend(self.rate_options(RateMode::Quality, quality, 0, 0));
        o
    }

    /// Encoder options for the target-bitrate mode (the bitrate itself goes into the context),
    /// at the default speed and maximum.
    pub fn bitrate_options(&self, bps: u32) -> Vec<(&'static str, String)> {
        let mut o = self.speed_options(Speed::Fast);
        o.extend(self.rate_options(RateMode::Vbr, 0, bps, (bps as u64 * 3 / 2) as u32));
        o
    }

    /// The rate mode in the encoder's own words (D4). `bps`/`max` are bit/s; the average itself
    /// goes into the context's `bit_rate`.
    pub fn rate_options(&self, rate: RateMode, quality: u32, bps: u32, max: u32) -> Vec<(&'static str, String)> {
        let q = quality.to_string();
        let soft_vbv = |o: &mut Vec<(&'static str, String)>| {
            o.push(("maxrate", max.to_string()));
            o.push(("bufsize", (bps as u64 * 2).to_string()));
        };
        let soft_cbr = |o: &mut Vec<(&'static str, String)>| {
            o.push(("maxrate", bps.to_string()));
            o.push(("bufsize", bps.to_string()));
        };
        let mut o = Vec::new();
        match (self.name, rate) {
            // ProRes: a fixed rate per profile, nothing to ask (the window disables the row)
            ("prores_ks", _) => {}
            ("libx264" | "libx265", RateMode::Quality) => o.push(("crf", q)),
            ("libx264" | "libx265", RateMode::Vbr) => soft_vbv(&mut o),
            ("libx264", RateMode::Cbr) => {
                soft_cbr(&mut o);
                o.push(("x264-params", "nal-hrd=cbr".into()));
            }
            ("libx265", RateMode::Cbr) => soft_cbr(&mut o),
            ("libsvtav1", RateMode::Quality) => o.push(("crf", q)),
            ("libsvtav1", _) => soft_vbv(&mut o),
            ("libaom-av1" | "libvpx-vp9", RateMode::Quality) => {
                o.push(("crf", q));
                o.push(("b", "0".into()));
            }
            ("libaom-av1" | "libvpx-vp9", RateMode::Vbr) => soft_vbv(&mut o),
            ("libaom-av1" | "libvpx-vp9", RateMode::Cbr) => {
                o.push(("minrate", bps.to_string()));
                soft_cbr(&mut o);
            }
            ("h264_nvenc" | "hevc_nvenc" | "av1_nvenc", RateMode::Quality) => {
                o.push(("rc", "vbr".into()));
                o.push(("cq", q));
            }
            ("h264_nvenc" | "hevc_nvenc" | "av1_nvenc", RateMode::Vbr) => o.push(("rc", "vbr".into())),
            ("h264_nvenc" | "hevc_nvenc" | "av1_nvenc", RateMode::Cbr) => o.push(("rc", "cbr".into())),
            ("h264_amf" | "hevc_amf" | "av1_amf", RateMode::Quality) => {
                o.push(("rc", "cqp".into()));
                o.push(("qp_i", q.clone()));
                o.push(("qp_p", q));
            }
            ("h264_amf" | "hevc_amf" | "av1_amf", RateMode::Vbr) => o.push(("rc", "vbr_peak".into())),
            ("h264_amf" | "hevc_amf" | "av1_amf", RateMode::Cbr) => o.push(("rc", "cbr".into())),
            ("h264_qsv" | "hevc_qsv" | "av1_qsv" | "vp9_qsv", RateMode::Quality) => o.push(("global_quality", q)),
            // QSV picks VBR from a bitrate, CBR from a maximum equal to it
            ("h264_qsv" | "hevc_qsv" | "av1_qsv" | "vp9_qsv", RateMode::Vbr) => o.push(("maxrate", max.to_string())),
            ("h264_qsv" | "hevc_qsv" | "av1_qsv" | "vp9_qsv", RateMode::Cbr) => o.push(("maxrate", bps.to_string())),
            ("h264_vaapi" | "hevc_vaapi" | "av1_vaapi" | "vp9_vaapi", RateMode::Quality) => o.push(("qp", q)),
            // without a rate control the bitrate mode silently used the driver's default (D7)
            ("h264_vaapi" | "hevc_vaapi" | "av1_vaapi" | "vp9_vaapi", RateMode::Vbr) => {
                o.push(("rc_mode", "VBR".into()));
                o.push(("maxrate", max.to_string()));
            }
            ("h264_vaapi" | "hevc_vaapi" | "av1_vaapi" | "vp9_vaapi", RateMode::Cbr) => o.push(("rc_mode", "CBR".into())),
            // Media Foundation: 0..100, higher = better
            ("h264_mf" | "hevc_mf", RateMode::Quality) => {
                o.push(("rate_control", "quality".into()));
                o.push(("quality", (100u32.saturating_sub(quality * 2)).clamp(1, 100).to_string()));
            }
            ("h264_mf" | "hevc_mf", RateMode::Vbr) => o.push(("rate_control", "u_vbr".into())),
            ("h264_mf" | "hevc_mf", RateMode::Cbr) => o.push(("rate_control", "cbr".into())),
            // mpeg4: the codec's own defaults (the last resort)
            _ => {}
        }
        o
    }

    /// Options that do not depend on the rate or the speed: ProRes's profile, NVENC HEVC's
    /// Main 10 at 10-bit (the profile follows the depth, D3).
    pub fn fixed_options(&self, fmt: &OutputFormat) -> Vec<(&'static str, String)> {
        match self.name {
            "prores_ks" => vec![("profile", "3".into())],
            "hevc_nvenc" if fmt.depth == crate::format::BitDepth::Ten => vec![("profile", "main10".into())],
            _ => vec![],
        }
    }

    /// Every option of an open for `fmt` (speed, rate, fixed), in that order.
    pub fn options(&self, fmt: &OutputFormat, rate: RateMode, quality: u32, bps: u32) -> Vec<(&'static str, String)> {
        let mut o = self.speed_options(fmt.speed);
        o.extend(self.rate_options(rate, quality, bps, fmt.max_for(bps)));
        o.extend(self.fixed_options(fmt));
        o
    }
}

const fn hw(name: &'static str, pix: PixFmt) -> EncoderSpec {
    EncoderSpec { name, hardware: true, pix, pix10: None, ctx: HwKind::None, trial: false, cbr: true }
}
const fn nvenc10(name: &'static str) -> EncoderSpec {
    EncoderSpec { name, hardware: true, pix: PixFmt::Yuv420p, pix10: Some(PixFmt::P010), ctx: HwKind::None, trial: false, cbr: true }
}
const fn vaapi(name: &'static str) -> EncoderSpec {
    EncoderSpec { name, hardware: true, pix: PixFmt::Nv12, pix10: None, ctx: HwKind::Vaapi, trial: true, cbr: true }
}
/// System NV12 frames, but the library's own session reaches the same VA driver on Linux.
const fn qsv_on_va(name: &'static str) -> EncoderSpec {
    EncoderSpec { name, hardware: true, pix: PixFmt::Nv12, pix10: None, ctx: HwKind::None, trial: true, cbr: true }
}
const fn sw(name: &'static str) -> EncoderSpec {
    EncoderSpec { name, hardware: false, pix: PixFmt::Yuv420p, pix10: Some(PixFmt::Yuv420p10), ctx: HwKind::None, trial: false, cbr: true }
}
const fn sw8(name: &'static str) -> EncoderSpec {
    EncoderSpec { name, hardware: false, pix: PixFmt::Yuv420p, pix10: None, ctx: HwKind::None, trial: false, cbr: false }
}

/// The H.264 cascade of `os`, in order of preference.
pub fn video_encoder_candidates(os: Os) -> Vec<EncoderSpec> {
    codec_candidates(os, VideoCodec::H264)
}

/// The cascade of `codec` on `os`, hardware first, ending with a software encoder (D3).
pub fn codec_candidates(os: Os, codec: VideoCodec) -> Vec<EncoderSpec> {
    let svt = EncoderSpec { cbr: false, ..sw("libsvtav1") };
    let prores = EncoderSpec {
        name: "prores_ks",
        hardware: false,
        pix: PixFmt::Yuv422p10,
        pix10: Some(PixFmt::Yuv422p10),
        ctx: HwKind::None,
        trial: false,
        cbr: false,
    };
    match (os, codec) {
        (Os::Linux, VideoCodec::H264) => vec![
            hw("h264_nvenc", PixFmt::Yuv420p),
            vaapi("h264_vaapi"),
            qsv_on_va("h264_qsv"),
            sw("libx264"),
            sw8("mpeg4"),
        ],
        (Os::Windows, VideoCodec::H264) => vec![
            hw("h264_nvenc", PixFmt::Yuv420p),
            hw("h264_amf", PixFmt::Yuv420p),
            hw("h264_qsv", PixFmt::Nv12),
            sw("libx264"),
            hw("h264_mf", PixFmt::Yuv420p),
            sw8("mpeg4"),
        ],
        (Os::Linux, VideoCodec::H265) => {
            vec![nvenc10("hevc_nvenc"), vaapi("hevc_vaapi"), qsv_on_va("hevc_qsv"), sw("libx265")]
        }
        (Os::Windows, VideoCodec::H265) => vec![
            nvenc10("hevc_nvenc"),
            hw("hevc_amf", PixFmt::Yuv420p),
            hw("hevc_qsv", PixFmt::Nv12),
            sw("libx265"),
            hw("hevc_mf", PixFmt::Yuv420p),
        ],
        (Os::Linux, VideoCodec::Av1) => {
            vec![nvenc10("av1_nvenc"), vaapi("av1_vaapi"), qsv_on_va("av1_qsv"), svt, sw("libaom-av1")]
        }
        (Os::Windows, VideoCodec::Av1) => vec![
            nvenc10("av1_nvenc"),
            hw("av1_amf", PixFmt::Yuv420p),
            hw("av1_qsv", PixFmt::Nv12),
            svt,
            sw("libaom-av1"),
        ],
        (Os::Linux, VideoCodec::Vp9) => vec![vaapi("vp9_vaapi"), qsv_on_va("vp9_qsv"), sw("libvpx-vp9")],
        (Os::Windows, VideoCodec::Vp9) => vec![hw("vp9_qsv", PixFmt::Nv12), sw("libvpx-vp9")],
        (_, VideoCodec::ProRes) => vec![prores],
    }
}

/// The codec family whose cascade on `os` has `name`.
pub fn codec_of(os: Os, name: &str) -> Option<VideoCodec> {
    VideoCodec::ALL.into_iter().find(|&c| codec_candidates(os, c).iter().any(|s| s.name == name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::format::{AudioCodec, Container};

    /// t2801 (render-settings D2, D3): every codec's cascade is hardware first and ends with a
    /// software encoder; ProRes is only `prores_ks`; the container table holds what the
    /// reconnaissance decided; the default speed gives the options every export had before.
    #[test]
    fn t2801_cascades_and_the_container_table() {
        for os in [Os::Linux, Os::Windows] {
            for codec in VideoCodec::ALL {
                let list = codec_candidates(os, codec);
                assert!(!list.is_empty(), "{os:?} {codec:?}");
                let first_sw = list.iter().position(|e| !e.hardware).expect("a software encoder");
                assert!(list[..first_sw].iter().all(|e| e.hardware), "{os:?} {codec:?}: hardware first");
                assert!(list.iter().any(|e| !e.hardware), "{os:?} {codec:?} ends in software");
                for e in &list {
                    // a device candidate stays a trial on Linux, never on Windows
                    assert_eq!(e.trial, os == Os::Linux && (e.ctx == HwKind::Vaapi || e.name.ends_with("_qsv")), "{}", e.name);
                }
            }
            let prores: Vec<_> = codec_candidates(os, VideoCodec::ProRes).iter().map(|e| e.name).collect();
            assert_eq!(prores, ["prores_ks"]);
            assert_eq!(codec_candidates(os, VideoCodec::ProRes)[0].pix, PixFmt::Yuv422p10);
        }
        let names = |c| codec_candidates(Os::Linux, c).iter().map(|e| e.name).collect::<Vec<_>>();
        assert_eq!(names(VideoCodec::H265), ["hevc_nvenc", "hevc_vaapi", "hevc_qsv", "libx265"]);
        assert_eq!(names(VideoCodec::Av1), ["av1_nvenc", "av1_vaapi", "av1_qsv", "libsvtav1", "libaom-av1"]);
        assert_eq!(names(VideoCodec::Vp9), ["vp9_vaapi", "vp9_qsv", "libvpx-vp9"]);

        assert!(!Container::Mp4.allows_video(VideoCodec::ProRes));
        assert!(!Container::Mp4.allows_audio(AudioCodec::Pcm16));
        assert!(!Container::Webm.allows_video(VideoCodec::H264));
        assert!(!Container::Webm.allows_audio(AudioCodec::Aac));
        assert!(Container::Mov.allows_video(VideoCodec::ProRes) && Container::Mov.allows_audio(AudioCodec::Pcm24));
        for v in VideoCodec::ALL {
            assert!(Container::Mkv.allows_video(v));
        }
        for a in AudioCodec::ALL {
            assert!(Container::Mkv.allows_audio(a));
        }
        let d = OutputFormat::default();
        assert_eq!(d.refusal(), None);
        assert_eq!(OutputFormat { video: VideoCodec::ProRes, ..d.clone() }.refusal().unwrap(), "MP4 cannot hold ProRes 422 HQ");

        // the default speed is the former file: veryfast / p4, CRF and the 1.5× maximum
        let x264 = &codec_candidates(Os::Linux, VideoCodec::H264)[3];
        assert_eq!(x264.options(&d, RateMode::Quality, 23, 0), vec![("preset", "veryfast".into()), ("crf", "23".into())]);
        assert_eq!(
            x264.options(&d, RateMode::Vbr, 23, 8_000_000),
            vec![("preset", "veryfast".into()), ("maxrate", "12000000".into()), ("bufsize", "16000000".into())]
        );
        assert_eq!(d.gop_frames(30), 60, "two seconds of keyframes");
        // 10-bit: software takes it, VAAPI does not, h264_nvenc does not, NVENC HEVC does
        let fmt10 = OutputFormat { depth: crate::format::BitDepth::Ten, ..d.clone() };
        assert_eq!(x264.input(&fmt10), Ok(PixFmt::Yuv420p10));
        assert_eq!(codec_candidates(Os::Linux, VideoCodec::H264)[1].input(&fmt10), Err("no 10-bit input"));
        assert_eq!(codec_candidates(Os::Linux, VideoCodec::H264)[0].input(&fmt10), Err("no 10-bit input"));
        assert_eq!(codec_candidates(Os::Linux, VideoCodec::H265)[0].input(&fmt10), Ok(PixFmt::P010));
        assert!(!codec_candidates(Os::Linux, VideoCodec::Av1)[3].has_rate(RateMode::Cbr), "SVT-AV1 has no CBR");
    }
}
