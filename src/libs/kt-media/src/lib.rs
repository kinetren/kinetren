//! Media sources: the `MediaSource` trait, synthetic sources for tests,
//! rsmpeg/FFmpeg backend (feature `ffmpeg`, M3) and the sidecar index DB (M3,
//! see docs/media-index.md).

use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("out of range")]
    OutOfRange,
    #[error("decode failed: {0}")]
    Decode(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    pub duration: Tick,
    pub has_video: bool,
    pub has_audio: bool,
    /// Source frame rate (numerator/denominator), if there is video.
    pub fps: Option<(u32, u32)>,
    /// The source's native frame size (0×0 if there is no video) — for “Max (across
    /// sources)” in the resolution choice (item 11, v1 port).
    pub width: u32,
    pub height: u32,
    /// Pixel aspect ratio (SAR, num/den; R.62). The frame's FORM =
    /// `width × sar`: all conforming, thumbs and “Max (across sources)” are computed from
    /// the FORM, otherwise anamorphic footage (DV 4:3, SAR 16:15) would be yet another
    /// silent stretch. Default 1:1.
    pub sar: (u32, u32),
    /// A SINGLE PICTURE (R.67): one frame at any tick, with no frame grid
    /// and no end of material. An event of any length shows it whole, and
    /// “Loading…” does not hang over it.
    pub still: bool,
}

impl MediaInfo {
    /// Frame form in square pixels: (width × SAR, height).
    pub fn view_size(&self) -> (f64, f64) {
        let (n, d) = self.sar;
        let k = if n > 0 && d > 0 { n as f64 / d as f64 } else { 1.0 };
        (self.width as f64 * k, self.height as f64)
    }
}

/// The requested video quality (the size at which the decoder returns a frame).
/// `None` = the source's full resolution. Part of the cache key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Quality {
    pub width: u32,
    pub height: u32,
}

/// Audio burst: exactly `AUDIO_BURST_FRAMES` frames, 48 kHz stereo f32 interleaved.
/// Engine invariant no. 1: after decoding, audio in another format does not exist.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioBurst {
    /// The burst number (covers samples [idx*1024, (idx+1)*1024)).
    pub idx: i64,
    pub data: Box<[f32; AUDIO_BURST_FRAMES * AUDIO_CHANNELS]>,
}

impl AudioBurst {
    pub fn silence(idx: i64) -> AudioBurst {
        AudioBurst { idx, data: Box::new([0.0; AUDIO_BURST_FRAMES * AUDIO_CHANNELS]) }
    }
}

/// Video frame: RGBA8; premultiplied alpha is not required at this level.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoFrame {
    pub pts: Tick,
    pub width: u32,
    pub height: u32,
    pub rgba: Box<[u8]>,
    pub has_alpha: bool,
}

/// Media source. Implementations: `FfmpegSource` (feature `ffmpeg`, M3),
/// synthetic sources (tests). Exactly one MediaWorker thread owns an instance
/// (invariant no. 5) — hence `&mut self` and only `Send`.
pub trait MediaSource: Send {
    fn info(&self) -> &MediaInfo;
    /// Sample-accurate: burst `idx` covers the source's samples [idx*1024, (idx+1)*1024).
    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError>;
    fn read_video_frame(
        &mut self,
        pts: Tick,
        quality: Option<Quality>,
    ) -> Result<VideoFrame, MediaError>;
    fn keyframe_before(&self, pts: Tick) -> Option<Tick>;

    /// Accept NEW parameters WITHOUT recreating the source (R.53, item 410).
    ///
    /// The contract is strict: `true` means the parameters are applied AND `info()` did NOT
    /// change; `false` means the source cannot do it, and the caller must honestly
    /// recreate it (`Player::replace_media`). There is no intermediate state:
    /// `false` changes nothing.
    ///
    /// Why: a generator's parameters live in the media path, and until this item their
    /// edit looked to the engine like a FILE SWAP — the decoder stream was stopped,
    /// the cache was cleared, the renderers were rebuilt twice.
    /// For file sources the method stays `false`:
    /// changing the path there IS changing the file.
    fn retune(&mut self, _path: &str) -> bool {
        false
    }

    /// Accept NEW GENERATOR VALUES without recreation (R.59, stage 2).
    ///
    /// A twin of `retune` with the same strict contract (`true` ⇒ applied AND
    /// `info()` did not change),
    /// only the input is the event's state, not a string.
    /// A separate method, not “`retune` with serialization into the path”:
    /// curves do not fit into a path, and serializing values into a string
    /// just to parse them right back is a second parser on the same hot path.
    fn retune_gen(&mut self, _state: &kt_model::GenState) -> bool {
        false
    }

    /// Span: a frame for EVERY key (keys are aligned to the grid, strictly
    /// increasing). The frame semantics for key k are as in read_video_frame
    /// (the greatest pts <= k). The scheduler puts ALL frames into the cache — the GOP
    /// is decoded once per span (item 14/15). The default is a loop of single calls.
    fn read_video_span(
        &mut self,
        keys: &[Tick],
        quality: Option<Quality>,
        emit: &mut dyn FnMut(Tick, VideoFrame),
    ) -> Result<(), MediaError> {
        for &k in keys {
            match self.read_video_frame(k, quality) {
                Ok(f) => emit(k, f),
                Err(MediaError::OutOfRange) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

/// Frames in a level-0 block of the peak pyramid (docs/media-index.md).
pub const PEAK_BLOCK_FRAMES: usize = 256;

pub mod factory;
pub mod ffmpeg;
pub mod generator;
pub mod index;
#[cfg(feature = "ffmpeg")]
pub mod muxer;
pub(crate) mod pixfont;
pub mod placeholder;
#[cfg(feature = "ffmpeg")]
pub mod sync;
pub mod synthetic;
