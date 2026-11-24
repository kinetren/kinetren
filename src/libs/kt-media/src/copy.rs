//! Copies of heavy sources (persistent-cache D3): an intra-frame H.264 picture of the size the preview
//! decodes anyway, in the project's cache folder, read instead of a file that decodes too slowly.
//!
//! - A copy holds the PICTURE only, every frame at its own time; the sound is always the original's own
//!   ([`CopySource`] reads it there), and so are the file's native size, rate and duration: a plan with a
//!   copy is the plan without one, a softer picture aside.
//! - A copy is named by the original's content and its size: `media/<id>/copy-<w>x<h>.mov`. QuickTime,
//!   for its time scale: Matroska keeps milliseconds, and a picture a third of a millisecond late is
//!   another picture on a grid that meets its time.
//! - No export reads one: an export opens the media's path.

use crate::cache_folder::Folder;
use crate::index::ContentId;
use crate::{AudioBurst, MediaError, MediaInfo, MediaSource, Quality, SourceIdent, VideoFrame};
use kt_time::Tick;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Which copies the preview plays from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CopyMode {
    /// The size the preview decodes at.
    #[default]
    AsPreview,
    Half,
    Quarter,
    /// No copy is read or made.
    Off,
}

/// What a file is, as far as a copy of it goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CopyFacts {
    pub id: ContentId,
    /// The native size.
    pub size: (u32, u32),
    /// Most of its frames need others to decode.
    pub long_gop: bool,
    /// Frames of the file.
    pub frames: u64,
    /// A copy can stand for it: moving pictures, no alpha.
    pub copyable: bool,
}

/// Larger than this, a long-GOP file is heavy without being measured.
const HEAVY_PIXELS: u64 = 1920 * 1080;

impl CopyFacts {
    /// A file that gets a copy without a click: long-GOP above 1920×1080, or one the player measured
    /// decoding slower than real time (`slow`).
    pub fn heavy(&self, slow: bool) -> bool {
        self.copyable && (slow || (self.long_gop && self.size.0 as u64 * self.size.1 as u64 > HEAVY_PIXELS))
    }

    /// The size of the copy of one of `1/n` (even, as 4:2:0 takes it).
    pub fn size_at(&self, n: u32) -> (u32, u32) {
        let even = |v: u32| ((v / n).max(2) + 1) & !1;
        (even(self.size.0), even(self.size.1))
    }

    /// Which part of the native the preview wants (`1/n`): its decode size `decode` at the preview's
    /// quality, at most 1/2 and at least 1/8; `None` — the original plays (`full`: the preview is 1/1,
    /// or the decode asks more than a half).
    pub fn wanted(&self, mode: CopyMode, full: bool, decode: (u32, u32)) -> Option<u32> {
        match mode {
            CopyMode::Off => None,
            CopyMode::Half => Some(2),
            CopyMode::Quarter => Some(4),
            CopyMode::AsPreview if full => None,
            CopyMode::AsPreview => {
                // the largest step down that still holds every pixel the decode asks
                let holds = |n: u32| {
                    let (w, h) = self.size_at(n);
                    w >= decode.0 && h >= decode.1
                };
                [8, 4, 2].into_iter().find(|&n| holds(n))
            }
        }
    }
}

/// The copies of a media that lie in the folder, the smallest first.
pub fn copies_of(folder: &Folder, id: ContentId) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = std::fs::read_dir(folder.root().join(format!("media/{id}")))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().into_owned();
            let (w, h) = n.strip_prefix("copy-")?.strip_suffix(".mov")?.split_once('x')?;
            Some((w.parse().ok()?, h.parse().ok()?))
        })
        .collect();
    out.sort_by_key(|(w, h)| *w as u64 * *h as u64);
    out
}

/// The copy to read when `want` is wanted: the smallest at least as large, else the largest there is.
pub fn best_copy(have: &[(u32, u32)], want: (u32, u32)) -> Option<(u32, u32)> {
    have.iter().find(|c| c.0 >= want.0 && c.1 >= want.1).or(have.last()).copied()
}

/// A copy's entry in the folder.
pub fn copy_rel(id: ContentId, (w, h): (u32, u32)) -> String {
    format!("media/{id}/copy-{w}x{h}.mov")
}

const CAMERA: (&[u8; 4], u32) = (b"KTCM", 1);

/// The camera's copy a person bound to a media.
pub fn camera_copy(folder: &Folder, id: ContentId) -> Option<PathBuf> {
    let b = folder.read(&format!("media/{id}/camera"), CAMERA.0, CAMERA.1)?;
    let p = PathBuf::from(String::from_utf8(b).ok()?);
    p.is_file().then_some(p)
}

/// Bind a camera's copy, checked by [`CopyMaker::check_camera`].
pub fn bind_camera(folder: &Folder, id: ContentId, file: &Path) -> std::io::Result<()> {
    folder.write(&format!("media/{id}/camera"), CAMERA.0, CAMERA.1, file.to_string_lossy().as_bytes())
}

pub fn unbind_camera(folder: &Folder, id: ContentId) {
    let rel = format!("media/{id}/camera");
    let _ = std::fs::remove_file(folder.root().join(&rel));
    folder.gone(&rel);
}

/// Why a camera's copy does not fit.
#[derive(Debug, Clone, PartialEq)]
pub enum Mismatch {
    /// Seconds of the original and of the copy.
    Duration(f64, f64),
    /// Frames of the original and of the copy.
    Frames(u64, u64),
    /// Seconds of the original's sound and of the copy's.
    Audio(f64, f64),
    Unreadable(String),
}

/// What the job does at its next frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Go {
    On,
    /// The transport plays or an export runs.
    Wait,
    Stop,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyError {
    Stopped,
    /// No encoder of the codec opens here: the reason.
    NoEncoder(String),
    Failed(String),
}

/// A written copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Copied {
    pub frames: u64,
    pub encoder: String,
    /// What the original took to decode a frame, µs: what a read of the copy saves.
    pub decode_us: u64,
}

/// What makes and opens copies: the host's libav ([`ffmpeg::FfmpegCopies`]), a test's own.
pub trait CopyMaker: Send + Sync {
    /// `None` — the file cannot be read.
    fn facts(&self, folder: &Arc<Folder>, path: &Path) -> Option<CopyFacts>;
    /// Write the copy of `path` at `size` into `dest` with a short-lived source of its own. `go` is asked
    /// before each frame, with the frames written. A stopped or failed job leaves no file.
    fn make(&self, path: &Path, dest: &Path, size: (u32, u32), go: &mut dyn FnMut(u64) -> Go) -> Result<Copied, CopyError>;
    fn original(&self, folder: &Arc<Folder>, path: &Path) -> Result<Box<dyn MediaSource>, String>;
    fn copy(&self, path: &Path) -> Result<Box<dyn MediaSource>, String>;
    /// Does a camera's own copy stand for `path`?
    fn check_camera(&self, path: &Path, camera: &Path) -> Result<(), Mismatch>;
}

/// A media read from a copy: the picture there, everything else from the original.
pub struct CopySource {
    original: Box<dyn MediaSource>,
    copy: Box<dyn MediaSource>,
    /// The copy's entry, whose reads the folder counts.
    entry: Option<(Arc<Folder>, String)>,
}

impl CopySource {
    pub fn new(original: Box<dyn MediaSource>, copy: Box<dyn MediaSource>, entry: Option<(Arc<Folder>, String)>) -> CopySource {
        CopySource { original, copy, entry }
    }

    fn count(&self) {
        if let Some((f, e)) = &self.entry {
            f.used(e);
        }
    }

    /// A read with no size asked is at the original's: a copy is a lower decode step, not another file.
    fn sized(&self, q: Option<Quality>) -> Option<Quality> {
        let i = self.original.info();
        Some(q.unwrap_or(Quality { width: i.width.max(2), height: i.height.max(2) }))
    }
}

impl MediaSource for CopySource {
    fn info(&self) -> &MediaInfo {
        self.original.info()
    }

    /// Another picture than the original's: its own name in memory and on disk.
    fn identity(&self) -> Option<SourceIdent> {
        let (o, c) = (self.original.identity()?, self.copy.identity()?);
        Some(SourceIdent::of(&format!("copy|{:x}|{:x}", o.key, c.key)))
    }

    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        self.original.read_audio_burst(idx)
    }

    fn read_video_frame(&mut self, pts: Tick, quality: Option<Quality>) -> Result<VideoFrame, MediaError> {
        self.count();
        let q = self.sized(quality);
        self.copy.read_video_frame(pts, q)
    }

    fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        self.copy.keyframe_before(pts)
    }

    fn read_video_span(
        &mut self,
        keys: &[Tick],
        quality: Option<Quality>,
        emit: &mut dyn FnMut(Tick, VideoFrame),
    ) -> Result<(), MediaError> {
        self.count();
        let q = self.sized(quality);
        self.copy.read_video_span(keys, q, emit)
    }

    fn read_video_span_qualities(
        &mut self,
        keys: &[(Tick, Quality)],
        emit: &mut dyn FnMut(Tick, Quality, VideoFrame),
    ) -> Result<(), MediaError> {
        self.count();
        self.copy.read_video_span_qualities(keys, emit)
    }
}

#[cfg(feature = "ffmpeg")]
pub mod ffmpeg {
    //! Copies through our runtime-loaded libav (invariants 9, 10), by the disk prerender's rule: a
    //! hardware encoder only, unless the host forces one (`KT_RENDER_CACHE_ENCODER`).
    use super::*;
    use crate::ffmpeg::FfmpegSource;
    use crate::index::MediaStore;
    use crate::muxer::{MuxConfig, Muxer, Rate};
    use kt_ffmpeg::format::{AudioCodec, Container, OutputFormat, RateMode, VideoCodec};

    pub struct FfmpegCopies {
        /// `Some(name)` — this encoder whatever it is; `None` — the cascade, hardware only.
        pub forced: Option<String>,
    }

    impl FfmpegCopies {
        pub fn from_env() -> FfmpegCopies {
            FfmpegCopies { forced: crate::segment::ffmpeg::FfmpegSegments::from_env().forced }
        }
    }

    fn open(folder: &Arc<Folder>, path: &Path) -> Result<FfmpegSource, String> {
        let mut store = MediaStore::of(folder.clone());
        FfmpegSource::open_indexed(&path.to_string_lossy(), Some(&mut store)).map_err(|e| format!("{e:?}"))
    }

    impl CopyMaker for FfmpegCopies {
        fn facts(&self, folder: &Arc<Folder>, path: &Path) -> Option<CopyFacts> {
            let id = MediaStore::of(folder.clone()).ensure_media(path).ok()?.0;
            let src = open(folder, path).ok()?;
            let i = src.info().clone();
            let (n, d) = i.fps.unwrap_or((0, 1));
            let frames = (i.duration.0.max(0) as u128 * n as u128 / (kt_time::TICK_HZ as u128 * d.max(1) as u128)) as u64;
            let keys = src.packet_index().1.len() as u64;
            Some(CopyFacts {
                id,
                size: (i.width, i.height),
                long_gop: keys * 2 < frames,
                frames,
                copyable: i.has_video && !i.still && !src.has_alpha() && n > 0,
            })
        }

        fn make(&self, path: &Path, dest: &Path, (w, h): (u32, u32), go: &mut dyn FnMut(u64) -> Go) -> Result<Copied, CopyError> {
            let failed = |e: &dyn std::fmt::Display| CopyError::Failed(e.to_string());
            let mut src = kt_ffmpeg::Source::open(&path.to_string_lossy()).map_err(|e| failed(&e))?;
            let p = src.probe().clone();
            let time_base = src.video_time_base().ok_or_else(|| CopyError::Failed("no video stream".into()))?;
            let (num, den) = p.fps.unwrap_or((25, 1));
            let cfg = MuxConfig {
                path: dest.to_string_lossy().into_owned(),
                width: w,
                height: h,
                frame_rate: Rate { num, den: den.max(1) },
                // each picture at its own time, in the original's own base: the copy is read by the same
                // comparison as the original
                time_base: Some(time_base),
                quality: crate::segment::ffmpeg::QUALITY,
                video_bitrate: None,
                audio_bitrate: 0,
                force_encoder: self.forced.clone(),
                threads: 1,
                format: OutputFormat {
                    container: Container::Mov,
                    video: VideoCodec::H264,
                    rate: RateMode::Quality,
                    keyint_tenths: 0,
                    b_frames: 0,
                    audio: AudioCodec::None,
                    ..OutputFormat::default()
                },
            };
            let mut mux = Muxer::open(&cfg).map_err(|e| CopyError::NoEncoder(e.to_string()))?;
            let gone = |mux: Muxer, e: CopyError| {
                if kt_time::shutdown::ending() {
                    // the process ends: no trailer, no unlink; the next open sweeps the file
                    std::mem::forget(mux);
                    return Err(e);
                }
                let _ = mux.finish();
                let _ = std::fs::remove_file(dest);
                Err(e)
            };
            let report = mux.open_report().clone();
            if self.forced.is_none() && !report.hardware {
                return gone(mux, CopyError::NoEncoder(format!("no hardware encoder opens (the cascade reached {})", report.encoder)));
            }
            let made = (|| {
                let mut frame = mux.alloc_video_frame().map_err(|e| failed(&e))?;
                let mut conv = mux.video_converter().map_err(|e| failed(&e))?;
                let (mut frames, mut spent) = (0u64, std::time::Duration::ZERO);
                loop {
                    loop {
                        match go(frames) {
                            Go::On => break,
                            Go::Wait => std::thread::sleep(std::time::Duration::from_millis(20)),
                            Go::Stop => return Err(CopyError::Stopped),
                        }
                    }
                    let at = std::time::Instant::now();
                    let next = src.next_video(w, h).map_err(|e| failed(&e))?;
                    spent += at.elapsed();
                    let Some((pts, pic)) = next else { break };
                    conv.convert(&pic.rgba, &mut frame).map_err(|e| failed(&e))?;
                    mux.write_video_frame_at(&frame, pts).map_err(|e| failed(&e))?;
                    frames += 1;
                }
                if frames == 0 {
                    return Err(CopyError::Failed("no picture decodes".into()));
                }
                Ok(Copied { frames, encoder: report.encoder.clone(), decode_us: spent.as_micros() as u64 / frames })
            })();
            match made {
                Ok(c) => match mux.finish() {
                    Ok(()) => Ok(c),
                    Err(e) => {
                        let _ = std::fs::remove_file(dest);
                        Err(CopyError::Failed(e.to_string()))
                    }
                },
                Err(e) => gone(mux, e),
            }
        }

        fn original(&self, folder: &Arc<Folder>, path: &Path) -> Result<Box<dyn MediaSource>, String> {
            Ok(Box::new(open(folder, path)?))
        }

        fn copy(&self, path: &Path) -> Result<Box<dyn MediaSource>, String> {
            // by the owner's rule (row 775) the machine's device decodes what a device encoded
            FfmpegSource::open_hw(&path.to_string_lossy()).map(|s| Box::new(s) as _).map_err(|e| format!("{e:?}"))
        }

        fn check_camera(&self, path: &Path, camera: &Path) -> Result<(), Mismatch> {
            let open = |p: &Path| kt_ffmpeg::Source::open(&p.to_string_lossy()).map_err(|e| Mismatch::Unreadable(e.to_string()));
            let frames = |p: &Path| {
                crate::muxer::probe_output(&p.to_string_lossy())
                    .map(|o| o.packets.len() as u64)
                    .map_err(|e| Mismatch::Unreadable(e.to_string()))
            };
            let (o, c) = (open(path)?, open(camera)?);
            let secs = |t: Tick| t.0 as f64 / kt_time::TICK_HZ as f64;
            let (po, pc) = (o.probe().clone(), c.probe().clone());
            let (n, d) = po.fps.unwrap_or((25, 1));
            let one = d as f64 / n.max(1) as f64;
            if (secs(po.duration) - secs(pc.duration)).abs() > one + 1e-9 {
                return Err(Mismatch::Duration(secs(po.duration), secs(pc.duration)));
            }
            let (fo, fc) = (frames(path)?, frames(camera)?);
            if fo.abs_diff(fc) > 1 {
                return Err(Mismatch::Frames(fo, fc));
            }
            if let (Some(a), Some(b)) = (o.audio_end(), c.audio_end())
                && (a.0 - b.0).abs() > kt_time::AUDIO_BURST_TICKS
            {
                return Err(Mismatch::Audio(secs(a), secs(b)));
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rule of a copy's size (persistent-cache D3): the decode's size at most a half and at least an
    /// eighth of the native, the original at 1/1; the copy read is the smallest that holds it, else the
    /// largest there is.
    #[test]
    fn the_size_follows_the_preview() {
        let f = CopyFacts { id: ContentId([0; 16]), size: (3840, 2160), long_gop: true, frames: 100, copyable: true };
        let w = |full, d| f.wanted(CopyMode::AsPreview, full, d);
        assert_eq!(w(true, (3840, 2160)), None, "1/1: the original");
        assert_eq!(w(false, (1920, 1080)), Some(2));
        assert_eq!(w(false, (960, 540)), Some(4));
        assert_eq!(w(false, (640, 360)), Some(4), "between two: the one that holds it");
        assert_eq!(w(false, (320, 180)), Some(8));
        assert_eq!(w(false, (100, 50)), Some(8), "at least an eighth");
        assert_eq!(w(false, (2560, 1440)), None, "more than a half: the original");
        assert_eq!((f.wanted(CopyMode::Half, true, (3840, 2160)), f.wanted(CopyMode::Off, false, (320, 180))), (Some(2), None));
        assert!(f.heavy(false));
        let cheap = CopyFacts { long_gop: false, ..f };
        assert!(!cheap.heavy(false) && cheap.heavy(true));
        assert!(!CopyFacts { size: (1920, 1080), ..f }.heavy(false), "not above 1920×1080");
        assert!(!CopyFacts { copyable: false, ..f }.heavy(true), "a still, or alpha");
        let have = [(480, 270), (960, 540), (1920, 1080)];
        assert_eq!(best_copy(&have, (640, 360)), Some((960, 540)));
        assert_eq!(best_copy(&have[..1], (960, 540)), Some((480, 270)), "a smaller one meanwhile");
        assert_eq!(best_copy(&[], (960, 540)), None);
    }
}
