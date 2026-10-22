//! A bridge to the system FFmpeg through kt-ffmpeg (runtime loading, dlopen).
//!
//! Only the adaptation to the `MediaSource` trait is here; all the work with libav*
//! (including the choice of the major version 6/7/8) is in the kt-ffmpeg crate.
//! INVARIANT: the binary is not linked against libav*, so the same executable
//! works on systems with different FFmpeg versions.

use crate::{AudioBurst, MediaError, MediaInfo, MediaSource, Quality, VideoFrame};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};

pub struct FfmpegSource {
    inner: kt_ffmpeg::Source,
    info: MediaInfo,
    /// The source frame size: used when quality = None.
    src_size: (u32, u32),
    /// The stream format has alpha (R.67): the frame goes into the composite with it.
    has_alpha: bool,
}

impl From<kt_ffmpeg::FfError> for MediaError {
    fn from(e: kt_ffmpeg::FfError) -> MediaError {
        match e {
            kt_ffmpeg::FfError::OutOfRange => MediaError::OutOfRange,
            other => MediaError::Decode(other.to_string()),
        }
    }
}

impl FfmpegSource {
    pub fn open(path: &str) -> Result<FfmpegSource, MediaError> {
        Self::from_inner(kt_ffmpeg::Source::open(path)?)
    }

    /// Opening through the sidecar index DB: the packet index (the pts of audio packets
    /// and video keyframes) is taken from the DB if the file has not changed, and the full
    /// scan pass over the file disappears (a fast opening of large files).
    /// After a fresh scan the index is saved into the DB. Any DB error is a
    /// CACHE error: we silently fall back to the ordinary open with a scan.
    pub fn open_indexed(
        path: &str,
        db: Option<&mut crate::index::MediaIndexDb>,
    ) -> Result<FfmpegSource, MediaError> {
        let Some(db) = db else { return Self::open(path) };
        let Ok((mid, status)) = db.ensure_media(std::path::Path::new(path)) else {
            return Self::open(path);
        };
        if status == crate::index::MediaEntryStatus::Reused
            && let (Ok(Some(audio)), Ok(Some(video))) =
                (db.packet_index(mid, "audio"), db.packet_index(mid, "video"))
        {
            let prebuilt = kt_ffmpeg::PrebuiltIndex { audio_pts: audio, video_keyframes: video };
            return Self::from_inner(kt_ffmpeg::Source::open_with_index(path, Some(prebuilt))?);
        }
        let src = Self::open(path)?;
        let (audio, video) = src.inner.packet_index();
        let _ = db.put_packet_index(mid, "audio", audio);
        let _ = db.put_packet_index(mid, "video", video);
        Ok(src)
    }

    /// The index came from the DB (no scan was performed): diagnostics/test 34.
    pub fn index_was_prebuilt(&self) -> bool {
        self.inner.index_was_prebuilt()
    }

    /// (pts of audio packets, pts of video keyframes): a check in test 34.
    pub fn packet_index(&self) -> (&[i64], &[i64]) {
        self.inner.packet_index()
    }

    fn from_inner(inner: kt_ffmpeg::Source) -> Result<FfmpegSource, MediaError> {
        let p = inner.probe();
        let info = MediaInfo {
            duration: p.duration,
            has_video: p.has_video,
            has_audio: p.has_audio,
            fps: p.fps,
            width: p.width,
            height: p.height,
            sar: p.sar,
            still: p.still,
        };
        let src_size = (p.width.max(2), p.height.max(2));
        let has_alpha = p.has_alpha;
        Ok(FfmpegSource { inner, info, src_size, has_alpha })
    }
}

impl FfmpegSource {
    /// (backward_reopens, frames_decoded): diagnostics of test 21.
    pub fn decode_debug_stats(&self) -> (u64, u64) {
        self.inner.video_debug_stats()
    }
}

/// The version of the loaded FFmpeg (for the perf panel/diagnostics).
pub fn backend_info() -> Option<kt_ffmpeg::BackendInfo> {
    kt_ffmpeg::backend_info()
}

/// The reason FFmpeg did not load (for the message to the user).
pub fn backend_error() -> Option<String> {
    kt_ffmpeg::backend_error()
}

/// The cumulative (opens, packet_reads) of the FFmpeg layer: the perf panel
/// differentiates them by time (a port of the v1 avcpp metrics).
pub fn io_stats() -> (u64, u64) {
    kt_ffmpeg::iostats::snapshot()
}

impl MediaSource for FfmpegSource {
    fn info(&self) -> &MediaInfo {
        &self.info
    }

    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        let mut burst = AudioBurst::silence(idx);
        // A PICTURE has no sound by nature, and this is not a read error (R.67):
        // an event with a still MUST play silence, like a placeholder, and not
        // bring down the mix with an “out of range” refusal.
        if self.info.still {
            return Ok(burst);
        }
        self.inner.read_audio_burst(idx, &mut burst.data[..])?;
        Ok(burst)
    }

    fn read_video_frame(
        &mut self,
        pts: Tick,
        quality: Option<Quality>,
    ) -> Result<VideoFrame, MediaError> {
        // A PICTURE has the same frame at any tick (R.67): asking the demuxer for a
        // position that is not in the file is a sure way to get
        // “no frame” and an eternal “Loading…” over the event.
        let pts = if self.info.still { Tick::ZERO } else { pts };
        let (w, h) = match quality {
            Some(q) => (q.width, q.height),
            None => self.src_size, // the source’s native resolution
        };
        let f = self.inner.read_video_frame(pts, w, h)?;
        Ok(VideoFrame {
            pts: f.pts,
            width: f.width,
            height: f.height,
            rgba: f.rgba.into_boxed_slice(),
            // The TRUTH about alpha (R.67): the stream format knows it, and the composite
            // can mix it: a hard-coded `false` discarded the transparency
            has_alpha: self.has_alpha,
        })
    }

    fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        self.inner.keyframe_before(pts)
    }

    fn read_video_span(
        &mut self,
        keys: &[Tick],
        quality: Option<Quality>,
        emit: &mut dyn FnMut(Tick, VideoFrame),
    ) -> Result<(), MediaError> {
        let (w, h) = match quality {
            Some(q) => (q.width, q.height),
            None => self.src_size,
        };
        let has_alpha = self.has_alpha;
        self.inner
            .read_video_span(keys, w, h, &mut |k, f| {
                emit(
                    k,
                    VideoFrame {
                        pts: f.pts,
                        width: f.width,
                        height: f.height,
                        rgba: f.rgba.into_boxed_slice(),
                        has_alpha,
                    },
                )
            })
            .map_err(MediaError::from)
    }
}

const _: () = assert!(AUDIO_BURST_FRAMES * AUDIO_CHANNELS > 0);
