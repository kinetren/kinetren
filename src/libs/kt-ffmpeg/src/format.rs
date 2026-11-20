//! The output format of an export (change render-settings, capability export, F-EXP-006).
//!
//! What the muxer writes beyond the size, the rate, the threads and the quality/bitrate/encoder
//! fields of `MuxConfig`: the container, the codec family, the rate mode, the GOP, the B-frames,
//! the encoder speed, the bit depth, the colour and the audio codec. Pure data, so the container
//! table and the defaults are tested on any machine (t2801). `Default` is the file every export
//! wrote before this change (D1): MP4, H.264, Quality, 2 s keyframes, no B-frames, `veryfast`,
//! 8-bit, BT.601 limited, AAC.

/// The file's container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Container {
    #[default]
    Mp4,
    Mov,
    Mkv,
    Webm,
}

/// The video codec family; each has its own cascade (`encoders::video_encoder_candidates`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VideoCodec {
    #[default]
    H264,
    H265,
    Av1,
    Vp9,
    /// ProRes 422 HQ: intra-only, a fixed rate per profile, 10-bit 4:2:2.
    ProRes,
}

/// How the encoder spends bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RateMode {
    /// Constant quality (CRF/CQ/QP — the encoder's own word): `MuxConfig.quality`.
    #[default]
    Quality,
    /// An average (`MuxConfig.video_bitrate`) with a maximum (`OutputFormat.max_bitrate`).
    Vbr,
    /// A constant rate (`MuxConfig.video_bitrate`).
    Cbr,
}

/// Five steps of the encoder's speed against its compression, mapped per encoder (D5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Speed {
    Fastest,
    /// `veryfast` / `p4` — what every export used before the choice existed.
    #[default]
    Fast,
    Balanced,
    Slow,
    Slowest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BitDepth {
    #[default]
    Eight,
    Ten,
}

/// The RGB→YUV matrix: used by the conversion AND written as the tag (D6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Matrix {
    /// What `sws` did by default and what the preview's decode does: the default.
    #[default]
    Bt601,
    Bt709,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Range {
    #[default]
    Limited,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioCodec {
    #[default]
    Aac,
    Pcm16,
    Pcm24,
    Flac,
    Opus,
    /// No audio stream in the file.
    None,
}

/// The output format (D1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFormat {
    pub container: Container,
    pub video: VideoCodec,
    pub rate: RateMode,
    /// The VBR maximum, bit/s; `None` — 1.5× the average.
    pub max_bitrate: Option<u32>,
    /// The keyframe interval in tenths of a second (20 = 2 s); at least one frame.
    pub keyint_tenths: u32,
    pub b_frames: u32,
    pub speed: Speed,
    pub depth: BitDepth,
    pub matrix: Matrix,
    pub range: Range,
    pub audio: AudioCodec,
    /// File metadata; empty — not written.
    pub title: String,
    pub comment: String,
}

impl Default for OutputFormat {
    fn default() -> OutputFormat {
        OutputFormat {
            container: Container::Mp4,
            video: VideoCodec::H264,
            rate: RateMode::Quality,
            max_bitrate: None,
            keyint_tenths: 20,
            b_frames: 0,
            speed: Speed::Fast,
            depth: BitDepth::Eight,
            matrix: Matrix::Bt601,
            range: Range::Limited,
            audio: AudioCodec::Aac,
            title: String::new(),
            comment: String::new(),
        }
    }
}

impl OutputFormat {
    /// The GOP in frames at `fps` (at least 1).
    pub fn gop_frames(&self, fps: u32) -> u32 {
        ((self.keyint_tenths as u64 * fps as u64 + 5) / 10).max(1) as u32
    }

    /// The VBR maximum for an average of `bps`.
    pub fn max_for(&self, bps: u32) -> u32 {
        self.max_bitrate.unwrap_or((bps as u64 * 3 / 2).min(u32::MAX as u64) as u32).max(bps)
    }

    /// Why this container cannot hold the asked codecs, or `None`.
    pub fn refusal(&self) -> Option<String> {
        if !self.container.allows_video(self.video) {
            return Some(format!("{} cannot hold {}", self.container.label(), self.video.label()));
        }
        if !self.container.allows_audio(self.audio) {
            return Some(format!("{} cannot hold {}", self.container.label(), self.audio.label()));
        }
        None
    }
}

impl Container {
    pub const ALL: [Container; 4] = [Container::Mp4, Container::Mov, Container::Mkv, Container::Webm];

    /// The libavformat short name.
    pub fn av_name(self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
            Container::Mov => "mov",
            Container::Mkv => "matroska",
            Container::Webm => "webm",
        }
    }

    pub fn extension(self) -> &'static str {
        match self {
            Container::Mp4 => "mp4",
            Container::Mov => "mov",
            Container::Mkv => "mkv",
            Container::Webm => "webm",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Container::Mp4 => "MP4",
            Container::Mov => "MOV",
            Container::Mkv => "MKV",
            Container::Webm => "WebM",
        }
    }

    /// The container table (D2).
    pub fn allows_video(self, v: VideoCodec) -> bool {
        use VideoCodec::*;
        match self {
            Container::Mp4 => matches!(v, H264 | H265 | Av1 | Vp9),
            Container::Mov => matches!(v, H264 | H265 | ProRes),
            Container::Mkv => true,
            Container::Webm => matches!(v, Vp9 | Av1),
        }
    }

    pub fn allows_audio(self, a: AudioCodec) -> bool {
        use AudioCodec::*;
        match self {
            Container::Mp4 => matches!(a, Aac | None),
            Container::Mov => matches!(a, Aac | Pcm16 | Pcm24 | None),
            Container::Mkv => true,
            Container::Webm => matches!(a, Opus | None),
        }
    }

    /// The `moov` atom first, so a file plays while it downloads.
    pub fn faststart(self) -> bool {
        matches!(self, Container::Mp4 | Container::Mov)
    }
}

impl VideoCodec {
    pub const ALL: [VideoCodec; 5] = [VideoCodec::H264, VideoCodec::H265, VideoCodec::Av1, VideoCodec::Vp9, VideoCodec::ProRes];

    pub fn label(self) -> &'static str {
        match self {
            VideoCodec::H264 => "H.264",
            VideoCodec::H265 => "H.265",
            VideoCodec::Av1 => "AV1",
            VideoCodec::Vp9 => "VP9",
            VideoCodec::ProRes => "ProRes 422 HQ",
        }
    }

    /// Every frame is a keyframe: no GOP, no B-frames, no rate mode.
    pub fn intra_only(self) -> bool {
        self == VideoCodec::ProRes
    }
}

impl AudioCodec {
    pub const ALL: [AudioCodec; 6] =
        [AudioCodec::Aac, AudioCodec::Pcm16, AudioCodec::Pcm24, AudioCodec::Flac, AudioCodec::Opus, AudioCodec::None];

    /// The encoder name; `None` for no audio.
    pub fn encoder(self) -> Option<&'static str> {
        match self {
            AudioCodec::Aac => Some("aac"),
            AudioCodec::Pcm16 => Some("pcm_s16le"),
            AudioCodec::Pcm24 => Some("pcm_s24le"),
            AudioCodec::Flac => Some("flac"),
            AudioCodec::Opus => Some("libopus"),
            AudioCodec::None => None,
        }
    }

    /// A bitrate is asked of it (the lossy ones).
    pub fn has_bitrate(self) -> bool {
        matches!(self, AudioCodec::Aac | AudioCodec::Opus)
    }

    pub fn label(self) -> &'static str {
        match self {
            AudioCodec::Aac => "AAC",
            AudioCodec::Pcm16 => "PCM 16-bit",
            AudioCodec::Pcm24 => "PCM 24-bit",
            AudioCodec::Flac => "FLAC",
            AudioCodec::Opus => "Opus",
            AudioCodec::None => "none",
        }
    }
}

/// What the loaded FFmpeg can write (for the window, D2): asked once per opening.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Availability {
    /// Containers whose muxer this FFmpeg has.
    pub containers: Vec<Container>,
    /// Per codec family, the encoders that exist (a device candidate only with its device).
    pub video: Vec<(VideoCodec, Vec<crate::AvailableEncoder>)>,
    pub audio: Vec<AudioCodec>,
}

impl Availability {
    /// Everything — a test's or a seam's answer.
    pub fn everything() -> Availability {
        Availability {
            containers: Container::ALL.to_vec(),
            video: VideoCodec::ALL
                .iter()
                .map(|&v| {
                    let os = crate::search::Os::NATIVE;
                    let list = crate::encoders::codec_candidates(os, v)
                        .into_iter()
                        .filter(|s| s.ctx == crate::encoders::HwKind::None)
                        .map(|s| crate::AvailableEncoder { name: s.name.to_string(), device: None })
                        .collect();
                    (v, list)
                })
                .collect(),
            audio: AudioCodec::ALL.to_vec(),
        }
    }

    pub fn encoders(&self, v: VideoCodec) -> &[crate::AvailableEncoder] {
        self.video.iter().find(|(c, _)| *c == v).map(|(_, l)| l.as_slice()).unwrap_or(&[])
    }

    pub fn has_video(&self, v: VideoCodec) -> bool {
        !self.encoders(v).is_empty()
    }

    pub fn has_audio(&self, a: AudioCodec) -> bool {
        a == AudioCodec::None || self.audio.contains(&a)
    }

    pub fn has_container(&self, c: Container) -> bool {
        self.containers.contains(&c)
    }

    /// Some encoder of `v` here takes 10-bit input.
    pub fn has_ten_bit(&self, v: VideoCodec) -> bool {
        let specs = crate::encoders::codec_candidates(crate::search::Os::NATIVE, v);
        self.encoders(v).iter().any(|e| specs.iter().any(|s| s.name == e.name && s.pix10.is_some()))
    }

    /// Some encoder of `v` here has the rate mode.
    pub fn has_rate(&self, v: VideoCodec, r: RateMode) -> bool {
        let specs = crate::encoders::codec_candidates(crate::search::Os::NATIVE, v);
        self.encoders(v).iter().any(|e| specs.iter().any(|s| s.name == e.name && s.has_rate(r)))
    }
}

/// What our own reader finds in a written file (t2802): the test's eyes, not an external binary.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OutputProbe {
    /// libavformat's name of the demuxer (`mov,mp4,m4a,3gp,3g2,mj2`, `matroska,webm`).
    pub format: String,
    /// `h264`, `hevc`, `av1`, `vp9`, `prores`, `mpeg4`.
    pub video_codec: String,
    pub profile: i32,
    pub pix_fmt: String,
    pub width: u32,
    pub height: u32,
    /// The stream's frame rate (num, den).
    pub fps: (i32, i32),
    /// `colorspace`, `color_range`, `color_primaries`, `color_trc` as libavutil numbers.
    pub color: (u32, u32, u32, u32),
    /// The chroma's place (`AVChromaLocation`: 1 — left), the conversion's siting (export-host-copy D2).
    pub chroma_location: u32,
    /// The codec tag (fourcc) of the video stream.
    pub video_tag: u32,
    pub audio_codec: Option<String>,
    pub audio_sample_fmt: i32,
    pub audio_bits: i32,
    pub title: Option<String>,
    pub comment: Option<String>,
    /// The video packets in decode order: (pts, dts, keyframe, size).
    pub packets: Vec<(i64, i64, bool, u32)>,
    /// Seconds of video (the packets' span at the stream's time base).
    pub video_secs: f64,
}

impl OutputProbe {
    /// Frame indices (in presentation order) of the keyframes.
    pub fn keyframes(&self) -> Vec<usize> {
        let mut pts: Vec<(i64, bool)> = self.packets.iter().map(|p| (p.0, p.2)).collect();
        pts.sort();
        pts.iter().enumerate().filter(|(_, p)| p.1).map(|(i, _)| i).collect()
    }

    /// Some frame is shown out of its decode order (B-frames).
    pub fn reordered(&self) -> bool {
        self.packets.windows(2).any(|w| w[1].0 < w[0].0)
    }

    /// The video bitrate over the stream, bit/s.
    pub fn video_bitrate(&self) -> f64 {
        let bytes: u64 = self.packets.iter().map(|p| p.3 as u64).sum();
        if self.video_secs > 0.0 { bytes as f64 * 8.0 / self.video_secs } else { 0.0 }
    }
}
