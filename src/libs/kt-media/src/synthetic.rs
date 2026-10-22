//! Synthetic sources for tests: deterministic, without FFmpeg.
//! They live in the main crate (not under #[cfg(test)]) so that downstream crates
//! can use them in their tests.

use crate::{AudioBurst, MediaError, MediaInfo, MediaSource, Quality, VideoFrame};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, Tick};

/// What signal the source's audio part generates.
#[derive(Debug, Clone, Copy)]
pub enum AudioPattern {
    /// Silence.
    Silence,
    /// A sine of the given frequency, amplitude 0.5, identical in both channels.
    Sine { hz: f64 },
    /// The sample value = its global number (for sample-exact seam tests).
    /// Left channel = n, right = -n, scale 1e-6 to stay in a reasonable range.
    Ramp,
    /// A constant value (for crossfade tests).
    Dc { value: f32 },
    /// A unit impulse at sample 0, then silence (for FX tail tests).
    Impulse,
    /// Clicks over a quiet background (R.47): the same bursts as Clicks, plus a
    /// constant 200 Hz sine of amplitude 0.02 (−34 dBFS) in both channels.
    /// A dynamics stimulus: the gate has something to suppress BETWEEN the clicks
    /// (hold/threshold are audible), and the compressor's release pumps the background.
    ClicksBed,
    /// Wideband stereo clicks (R.47): a burst of 4 frames every 4800 frames,
    /// channels alternating. A stimulus for the audio FX skeleton: filters have HF,
    /// stereo parameters have the S component, and curves have events along the
    /// whole length; a width of 4 frames — so that a sample holder (bitcrusher) catches
    /// the click at any phase of its grid, including reverse.
    Clicks,
}

/// Shared call counters: the source lives in the worker thread, and the test
/// observes from outside (tests 13, 21).
#[derive(Default)]
pub struct SyntheticStats {
    pub audio_reads: std::sync::atomic::AtomicU64,
    pub video_reads: std::sync::atomic::AtomicU64,
    /// Video requests with a pts LESS than the previous one (for a real decoder this is
    /// a backward seek + recreation — test 21 requires zero on a sequence).
    pub video_backward_reads: std::sync::atomic::AtomicU64,
    pub last_video_pts: std::sync::atomic::AtomicI64,
    pub audio_backward_reads: std::sync::atomic::AtomicU64,
    pub last_audio_idx: std::sync::atomic::AtomicI64,
}

/// Synthetic source: a deterministic function of the burst number / pts.
pub struct SyntheticSource {
    info: MediaInfo,
    pattern: AudioPattern,
    /// Call counter — for regression tests “zero decodes” (test 13).
    pub audio_reads: u64,
    pub video_reads: u64,
    /// Optional shared counters (tests through MediaWorker).
    pub stats: Option<std::sync::Arc<SyntheticStats>>,
    /// An artificial delay of the video decode (test 27: “slow 4K”).
    pub video_delay: std::time::Duration,
    /// true: frames beyond duration → OutOfRange (test 27: EOF).
    pub video_eof: bool,
    /// Audio also ENDS at `duration` (R.63 item 2): a real file does not deliver bursts
    /// past its end, while synthetic material by default extends the pattern
    /// infinitely — the material's edge cannot be reproduced on it.
    pub audio_eof: bool,
    /// The source's own FRAME GRID (item 14, round 14). By default there is none:
    /// the synthetic source delivers a frame exactly at the requested tick, so floor and ceil
    /// are indistinguishable on it and frame-rate conversion cannot be reproduced.
    /// With the grid enabled, the frame is aligned down to the SOURCE's grid, and the
    /// color encodes the source frame NUMBER — by the finished composition frame the
    /// test sees which exact frame reached the output.
    pub video_fps: Option<u32>,
}

impl SyntheticSource {
    pub fn new(duration: Tick, pattern: AudioPattern) -> SyntheticSource {
        SyntheticSource {
            info: MediaInfo {
                duration,
                has_video: true,
                has_audio: true,
                fps: Some((30, 1)),
                width: 64,
                height: 36,
                sar: (1, 1),
                still: false,
            },
            pattern,
            audio_reads: 0,
            video_reads: 0,
            stats: None,
            video_delay: std::time::Duration::ZERO,
            video_eof: false,
            audio_eof: false,
            video_fps: None,
        }
    }

    pub fn with_video_delay(mut self, d: std::time::Duration) -> SyntheticSource {
        self.video_delay = d;
        self
    }

    /// Enable the source's own frame grid (item 14, round 14).
    /// The declared frame native (R.62): the synthetic source paints a fill at any
    /// requested size, while its form (`info().width/height`) is the input
    /// of the conform rule.
    pub fn with_video_size(mut self, w: u32, h: u32) -> Self {
        self.info.width = w.max(1);
        self.info.height = h.max(1);
        self
    }

    /// Pixel aspect ratio (R.62): frame form = width × sar.
    pub fn with_sar(mut self, num: u32, den: u32) -> Self {
        self.info.sar = (num.max(1), den.max(1));
        self
    }

    pub fn with_video_fps(mut self, fps: u32) -> Self {
        self.video_fps = Some(fps);
        self.info.fps = Some((fps, 1));
        self
    }

    /// A source with a real end: both video and audio end at
    /// `duration`, as with a file.
    pub fn with_eof(mut self) -> SyntheticSource {
        self.video_eof = true;
        self.audio_eof = true;
        self
    }

    pub fn with_video_eof(mut self) -> SyntheticSource {
        self.video_eof = true;
        self
    }

    pub fn with_stats(
        duration: Tick,
        pattern: AudioPattern,
        stats: std::sync::Arc<SyntheticStats>,
    ) -> SyntheticSource {
        let mut s = SyntheticSource::new(duration, pattern);
        s.stats = Some(stats);
        s
    }

    /// The sample value `n` of channel `ch` for the pattern — the reference for assertions in tests.
    pub fn sample_at(pattern: AudioPattern, n: i64, ch: usize) -> f32 {
        match pattern {
            AudioPattern::Silence => 0.0,
            AudioPattern::Sine { hz } => {
                (0.5 * (2.0 * std::f64::consts::PI * hz * n as f64 / AUDIO_SAMPLE_RATE as f64)
                    .sin()) as f32
            }
            AudioPattern::Ramp => {
                let v = n as f32 * 1e-6;
                if ch == 0 { v } else { -v }
            }
            AudioPattern::Dc { value } => value,
            AudioPattern::Impulse => {
                if n == 0 {
                    1.0
                } else {
                    0.0
                }
            }
            AudioPattern::Clicks => {
                if n >= 0 && n % 4800 < 4 && ((n / 4800) % 2) as usize == ch {
                    0.8
                } else {
                    0.0
                }
            }
            AudioPattern::ClicksBed => {
                let click = if n >= 0 && n % 4800 < 4 && ((n / 4800) % 2) as usize == ch {
                    0.8
                } else {
                    0.0
                };
                let bed = 0.02
                    * (2.0 * std::f64::consts::PI * 200.0 * n as f64 / AUDIO_SAMPLE_RATE as f64)
                        .sin() as f32;
                click + bed
            }
        }
    }
}

impl MediaSource for SyntheticSource {
    fn info(&self) -> &MediaInfo {
        &self.info
    }

    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        if idx < 0 {
            return Err(MediaError::OutOfRange);
        }
        self.audio_reads += 1;
        if let Some(st) = &self.stats {
            use std::sync::atomic::Ordering::Relaxed;
            st.audio_reads.fetch_add(1, Relaxed);
            let last = st.last_audio_idx.swap(idx, Relaxed);
            if idx < last {
                st.audio_backward_reads.fetch_add(1, Relaxed);
            }
        }
        let base = idx * AUDIO_BURST_FRAMES as i64;
        if self.audio_eof && base >= self.info.duration.to_audio_frames() {
            return Err(MediaError::OutOfRange);
        }
        let mut burst = AudioBurst::silence(idx);
        for f in 0..AUDIO_BURST_FRAMES {
            for ch in 0..AUDIO_CHANNELS {
                burst.data[f * AUDIO_CHANNELS + ch] =
                    Self::sample_at(self.pattern, base + f as i64, ch);
            }
        }
        Ok(burst)
    }

    fn read_video_frame(
        &mut self,
        pts: Tick,
        quality: Option<Quality>,
    ) -> Result<VideoFrame, MediaError> {
        if self.video_eof && pts >= self.info.duration {
            return Err(MediaError::OutOfRange);
        }
        if !self.video_delay.is_zero() {
            std::thread::sleep(self.video_delay);
        }
        self.video_reads += 1;
        if let Some(st) = &self.stats {
            use std::sync::atomic::Ordering::Relaxed;
            st.video_reads.fetch_add(1, Relaxed);
            let last = st.last_video_pts.swap(pts.0, Relaxed);
            if pts.0 < last {
                st.video_backward_reads.fetch_add(1, Relaxed);
            }
        }
        let q = quality.unwrap_or(Quality { width: 64, height: 36 });
        // Without its own grid: the frame is uniquely determined by pts (color = tick
        // number). With its own grid: floor on the source grid, color = the source frame's
        // NUMBER (item 14, round 14).
        let (code, out_pts) = match self.video_fps {
            Some(fps) => {
                let step = kt_time::TICK_HZ / fps.max(1) as i64;
                let k = pts.0.div_euclid(step);
                (k, Tick(k * step))
            }
            None => (pts.0, pts),
        };
        let r = (code & 0xff) as u8;
        let g = ((code >> 8) & 0xff) as u8;
        let b = ((code >> 16) & 0xff) as u8;
        let px = [r, g, b, 255u8];
        let rgba: Vec<u8> =
            px.iter().copied().cycle().take((q.width * q.height * 4) as usize).collect();
        Ok(VideoFrame {
            pts: out_pts,
            width: q.width,
            height: q.height,
            rgba: rgba.into_boxed_slice(),
            has_alpha: false,
        })
    }

    fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        Some(Tick(pts.0.max(0)))
    }
}

/// A source with a REAL native resolution and honest minification
/// (R.46): `SyntheticSource` fills any requested size, so on it it is impossible to measure
/// the loss of detail when decoding at Quality, nor its return when decoding larger.
///
/// The texture — vertical stripes 2 px wide IN NATIVE: halving keeps them (box takes
/// pairs of one color), quartering turns them into an even gray.
/// Therefore “decode at Quality” and “decode larger” are distinguishable
/// right by the contrast of the finished frame. A request LARGER than native
/// stretches with the nearest neighbor — no detail is added,
/// just as with real sws.
pub struct CheckerSource {
    info: MediaInfo,
    /// Native size (detail exists only at this size).
    native: (u32, u32),
}

impl CheckerSource {
    pub fn new(duration: Tick, native_w: u32, native_h: u32) -> CheckerSource {
        CheckerSource {
            info: MediaInfo {
                duration,
                has_video: true,
                has_audio: false,
                fps: Some((30, 1)),
                width: native_w.max(2),
                height: native_h.max(2),
                sar: (1, 1),
                still: false,
            },
            native: (native_w.max(2), native_h.max(2)),
        }
    }

    /// The texture value in native coordinates (gray channel).
    fn native_px(x: u32) -> u8 {
        if (x / 2).is_multiple_of(2) { 16 } else { 239 }
    }
}

impl MediaSource for CheckerSource {
    fn info(&self) -> &MediaInfo {
        &self.info
    }

    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        Ok(AudioBurst::silence(idx))
    }

    fn read_video_frame(
        &mut self,
        pts: Tick,
        quality: Option<Quality>,
    ) -> Result<VideoFrame, MediaError> {
        let (nw, nh) = self.native;
        let q = quality.unwrap_or(Quality { width: nw, height: nh });
        let (w, h) = (q.width.max(2) & !1, q.height.max(2) & !1);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for x in 0..w {
            // box over the native range of this output column; when
            // enlarging, the range degenerates to one native pixel
            let x0 = (x as u64 * nw as u64 / w as u64) as u32;
            let x1 = (((x as u64 + 1) * nw as u64).div_ceil(w as u64) as u32).max(x0 + 1).min(nw);
            let mut sum = 0u32;
            for nx in x0..x1 {
                sum += Self::native_px(nx) as u32;
            }
            let v = (sum / (x1 - x0)) as u8;
            for y in 0..h {
                let i = ((y * w + x) * 4) as usize;
                rgba[i] = v;
                rgba[i + 1] = v;
                rgba[i + 2] = v;
                rgba[i + 3] = 255;
            }
        }
        Ok(VideoFrame { pts, width: w, height: h, rgba: rgba.into_boxed_slice(), has_alpha: false })
    }

    fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        Some(Tick(pts.0.max(0)))
    }
}

#[cfg(test)]
mod tests {
    /// R.62 (t711b): pixel aspect ratio is part of the source's facts and
    /// works without FFmpeg: the synthetic source carries the same field, form = width × SAR.
    #[test]
    fn t711b_synthetic_carries_sar_into_the_view() {
        use super::*;
        let s = SyntheticSource::new(Tick(90_000), AudioPattern::Silence);
        assert_eq!(s.info().sar, (1, 1));
        assert_eq!(s.info().view_size(), (64.0, 36.0));
        let s = s.with_sar(2, 1);
        assert_eq!(s.info().sar, (2, 1));
        assert_eq!(s.info().view_size(), (128.0, 36.0));
        // a zero denominator is not accepted: clamped to 1
        let s = SyntheticSource::new(Tick(90_000), AudioPattern::Silence).with_sar(0, 0);
        assert_eq!(s.info().sar, (1, 1));
    }

    use super::*;

    #[test]
    fn ramp_is_globally_continuous_across_bursts() {
        let mut s = SyntheticSource::new(Tick(90_000), AudioPattern::Ramp);
        let b0 = s.read_audio_burst(0).unwrap();
        let b1 = s.read_audio_burst(1).unwrap();
        // the last sample b0 and the first b1 — adjacent values of the ramp
        let last = b0.data[(AUDIO_BURST_FRAMES - 1) * AUDIO_CHANNELS];
        let first = b1.data[0];
        assert_eq!(first, AUDIO_BURST_FRAMES as f32 * 1e-6);
        assert_eq!(last, (AUDIO_BURST_FRAMES as f32 - 1.0) * 1e-6);
        assert_eq!(s.audio_reads, 2);
    }
}
