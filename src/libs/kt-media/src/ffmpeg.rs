//! A bridge to the system FFmpeg through kt-ffmpeg (runtime loading, dlopen).
//!
//! Only the adaptation to the `MediaSource` trait is here; all the work with libav*
//! (including the choice of the major version 6/7/8/9) is in the kt-ffmpeg crate.
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
    /// The file's path, for a second decoder (export-throughput D9).
    path: String,
    /// The file as it was opened (durable-moment-key D1): a decoder made again reopens its codec or the
    /// same path, the source is replaced when the file is.
    ident: Option<crate::SourceIdent>,
    /// More decoders of this file, opened by a multi-quality span when one file is read at two
    /// places at once (a picture-in-picture of the same footage, a reel): one decoder
    /// ping-ponged between them, seeking and decoding a GOP for every frame. Owned like `inner`.
    cursors: Vec<kt_ffmpeg::Source>,
    /// When each decoder last read a run (a counter of runs, not a clock).
    cursor_used: [u64; MAX_CURSORS],
    runs: u64,
    /// Opened for the device (hw-segment-decode): the decoders added later open the same way.
    hw: bool,
}

/// A file as it lies now: its NAME (the store lies in the project's folder and must still hit after that
/// folder moved), size and modification time.
fn file_ident(path: &str) -> Option<crate::SourceIdent> {
    let p = std::path::Path::new(path);
    let md = std::fs::metadata(p).ok()?;
    let t = md.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?;
    let name = p.file_name()?.to_string_lossy();
    Some(crate::SourceIdent::of(&format!("file|{name}|{}|{}", md.len(), t.as_nanos())))
}

/// Decoders per file at most (D9): each 4K decoder holds its threads' frame pools.
const MAX_CURSORS: usize = 4;

/// The pictures a file's decoders may stand on together (decode-waste D3): two decoders of a 4K
/// file, as many decoders of a smaller one as fit the same pixels, at least two, at most
/// `MAX_CURSORS`. A decoder's frame pool grows with its picture, so a 1080p file's four
/// decoders hold what a 4K file's two do.
const CURSOR_PIXELS: u64 = 2 * 3840 * 2160;

pub fn cursors_for_size((w, h): (u32, u32)) -> usize {
    let px = (w as u64 * h as u64).max(1);
    ((CURSOR_PIXELS / px) as usize).clamp(2, MAX_CURSORS)
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
        Self::from_inner(path, kt_ffmpeg::Source::open(path)?)
    }

    /// Open for decoding on the platform's device (hw-segment-decode): the disk tier's segments,
    /// which the owner wants off the processor. The processor only where no device opens.
    pub fn open_hw(path: &str) -> Result<FfmpegSource, MediaError> {
        let mut s = Self::from_inner(path, kt_ffmpeg::Source::open_hw(path, None)?)?;
        s.hw = true;
        Ok(s)
    }

    /// What decodes this source's pictures: the device only when every decoder of the file is on it
    /// (a decoder `cursor_for` added opens as the first did).
    pub fn video_decode(&self) -> kt_ffmpeg::VideoDecode {
        std::iter::once(&self.inner)
            .chain(&self.cursors)
            .map(|s| s.video_decode())
            .find(|d| matches!(d, kt_ffmpeg::VideoDecode::Software(_)))
            .unwrap_or_else(|| self.inner.video_decode())
    }

    /// Opening through the sidecar index DB: the packet index (the pts of audio packets
    /// and video keyframes) is taken from the DB if the file has not changed, and the full
    /// scan pass over the file disappears (a fast opening of large files).
    /// After a fresh scan the index is saved into the DB. Any DB error is a
    /// CACHE error: we silently fall back to the ordinary open with a scan.
    pub fn open_indexed(
        path: &str,
        db: Option<&mut crate::index::MediaStore>,
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
            return Self::from_inner(path, kt_ffmpeg::Source::open_with_index(path, Some(prebuilt))?);
        }
        let src = Self::open(path)?;
        let (audio, video) = src.inner.packet_index();
        let _ = db.put_packet_index(mid, "audio", audio);
        let _ = db.put_packet_index(mid, "video", video);
        Ok(src)
    }

    /// The stream's pictures have alpha (R.67).
    pub fn has_alpha(&self) -> bool {
        self.has_alpha
    }

    /// The index came from the DB (no scan was performed): diagnostics/test 34.
    pub fn index_was_prebuilt(&self) -> bool {
        self.inner.index_was_prebuilt()
    }

    /// (pts of audio packets, pts of video keyframes): a check in test 34.
    pub fn packet_index(&self) -> (&[i64], &[i64]) {
        self.inner.packet_index()
    }

    fn from_inner(path: &str, inner: kt_ffmpeg::Source) -> Result<FfmpegSource, MediaError> {
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
        Ok(FfmpegSource {
            inner,
            info,
            src_size,
            has_alpha,
            ident: file_ident(path),
            path: path.to_string(),
            cursors: Vec::new(),
            cursor_used: [0; MAX_CURSORS],
            runs: 0,
            hw: false,
        })
    }

    /// The decoder for a run of keys starting at `start` (D9): the one that reaches it by
    /// decoding on (the nearest behind it, within the forward bound). Else a fresh one while the
    /// file has fewer than `cursors_for_size` (decode-waste D3), when no decoder is free to go:
    /// every one stands ahead of `start` (a place behind the others), has read a key of this
    /// request (`since`: the request's first run) or is the one a later key of it would be given.
    /// Else the free one standing longest unused — a key ahead of it is a seek forward, so a sweep
    /// (the filmstrip, a jump) keeps one decoder. Ties go to the lower index — the choice is a
    /// function of the requests, never of timing.
    fn cursor_for(&mut self, start: Tick, later: &[(Tick, u32, u32)], since: u64) -> usize {
        let all = 1 + self.cursors.len();
        let src = |i: usize| if i == 0 { &self.inner } else { &self.cursors[i - 1] };
        let at = |i: usize| src(i).video_cursor();
        let nearest = |t: Tick| (0..all).filter_map(|i| src(i).video_reach(t).map(|d| (d, i))).min().map(|(_, i)| i);
        if let Some(i) = nearest(start) {
            return i;
        }
        if let Some(i) = (0..all).find(|&i| at(i).is_none()) {
            return i;
        }
        let wanted = |i: usize| later.iter().any(|k| nearest(k.0) == Some(i));
        let held = |i: usize| self.cursor_used[i] >= since || wanted(i);
        let behind_all = (0..all).all(|i| at(i).is_some_and(|t| t.0 > start.0));
        if (behind_all || (0..all).all(held)) && all < cursors_for_size(self.src_size) {
            let (a, v) = self.inner.packet_index();
            let prebuilt = kt_ffmpeg::PrebuiltIndex { audio_pts: a.to_vec(), video_keyframes: v.to_vec() };
            let opened = if self.hw {
                kt_ffmpeg::Source::open_hw(&self.path, Some(prebuilt))
            } else {
                kt_ffmpeg::Source::open_with_index(&self.path, Some(prebuilt))
            };
            if let Ok(s) = opened {
                self.cursors.push(s);
                return all;
            }
        }
        // the decoder this request still holds keeps its place: the least recently used one may
        // stand right behind the next run
        (0..all).min_by_key(|&i| (wanted(i), self.cursor_used[i] >= since, self.cursor_used[i], i)).unwrap_or(0)
    }

    /// Does decoder `c`, having read `at`, read `next` on in the same run: it reaches it by
    /// decoding on and no other decoder already stands nearer (decode-waste D2).
    fn goes_on(&self, c: usize, at: Tick, next: Tick) -> bool {
        let src = |i: usize| if i == 0 { &self.inner } else { &self.cursors[i - 1] };
        let Some(d) = src(c).video_reach_from(at, next) else { return false };
        !(0..1 + self.cursors.len()).any(|i| i != c && src(i).video_reach(next).is_some_and(|e| e < d))
    }
}

impl FfmpegSource {
    /// How many times the audio sought (t1762).
    pub fn audio_debug_seeks(&self) -> u64 {
        self.inner.audio_debug_seeks()
    }

    /// How many decoders the file holds open (decode-waste D3, t2884).
    pub fn decoders(&self) -> usize {
        1 + self.cursors.len()
    }

    /// (backward_reopens, frames_decoded): diagnostics of test 21.
    /// All decoders of the file together (export-throughput D9).
    pub fn decode_debug_stats(&self) -> (u64, u64) {
        self.cursors.iter().fold(self.inner.video_debug_stats(), |(r, d), c| {
            let (r2, d2) = c.video_debug_stats();
            (r + r2, d + d2)
        })
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
        let (w, h) = self.size_of(quality);
        let mut out = None;
        self.read_runs(&[(pts, w, h)], &mut |_, _, f| out = Some(f))?;
        out.ok_or(MediaError::OutOfRange)
    }

    fn identity(&self) -> Option<crate::SourceIdent> {
        self.ident
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
        let (w, h) = self.size_of(quality);
        let sized: Vec<(Tick, u32, u32)> = keys.iter().map(|k| (*k, w, h)).collect();
        self.read_runs(&sized, &mut |k, _, f| emit(k, f))
    }

    fn read_video_span_qualities(
        &mut self,
        keys: &[(Tick, Quality)],
        emit: &mut dyn FnMut(Tick, Quality, VideoFrame),
    ) -> Result<(), MediaError> {
        let sized: Vec<(Tick, u32, u32)> = keys.iter().map(|(k, q)| (*k, q.width, q.height)).collect();
        self.read_runs(&sized, &mut |k, (width, height), f| emit(k, Quality { width, height }, f))
    }
}

impl FfmpegSource {
    /// The decode size asked: `None` is the source's native one.
    fn size_of(&self, quality: Option<Quality>) -> (u32, u32) {
        quality.map_or(self.src_size, |q| (q.width, q.height))
    }

    /// Every video read of the file, a frame, a span or a span at several sizes (decode-waste D4):
    /// runs, each to its own decoder (D9). A run goes on while its decoder, standing at the run's
    /// last key, is the nearest that reaches the next one (D2) — two places of one file read in one
    /// chunk keep a decoder each instead of one walking between them — and every key decides
    /// between decoding on and seeking (`read_video_span_sizes`).
    fn read_runs(
        &mut self,
        sized: &[(Tick, u32, u32)],
        emit: &mut dyn FnMut(Tick, (u32, u32), VideoFrame),
    ) -> Result<(), MediaError> {
        let has_alpha = self.has_alpha;
        let since = self.runs + 1;
        let mut i = 0;
        while i < sized.len() {
            self.runs += 1;
            let c = self.cursor_for(sized[i].0, &sized[i + 1..], since);
            self.cursor_used[c] = self.runs;
            let mut j = i + 1;
            while j < sized.len() && (sized[j].0 == sized[j - 1].0 || self.goes_on(c, sized[j - 1].0, sized[j].0)) {
                j += 1;
            }
            let src = if c == 0 { &mut self.inner } else { &mut self.cursors[c - 1] };
            src.read_video_span_sizes(&sized[i..j], &mut |k, size, f| {
                // the TRUTH about alpha (R.67): the stream format knows it, and the composite can mix it
                emit(k, size, VideoFrame { pts: f.pts, width: f.width, height: f.height, rgba: f.rgba.into_boxed_slice(), has_alpha })
            })?;
            i = j;
        }
        Ok(())
    }
}

const _: () = assert!(AUDIO_BURST_FRAMES * AUDIO_CHANNELS > 0);
