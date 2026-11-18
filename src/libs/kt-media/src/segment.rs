//! Video segments of the preview's disk tier (render-cache-disk D2; the owner's decision kkt-26.11.18b U4:
//! "сделай ты сжатие видео нормальное"). A segment is a run of consecutive preview composites written
//! as a short video by the machine's HARDWARE encoder (the export's own cascade, in-process) and read
//! back by our libav decoder. The engine knows only the traits: kt-engine depends on no FFmpeg, the host
//! hands it a [`SegmentCodec`] (kt-app, kt-cli: [`ffmpeg::FfmpegSegments`]).

use std::path::Path;

/// Writes one segment: frames of one size, in order.
pub trait SegmentWriter: Send {
    /// Encode the next frame (straight RGBA of the segment's size).
    fn push(&mut self, rgba: &[u8]) -> Result<(), String>;
    /// Close the file: every pushed frame is readable once this returns `Ok`.
    fn finish(self: Box<Self>) -> Result<u64, String>;
    /// The encoder that opened (the entry's header and the trace name it).
    fn encoder(&self) -> &str;
}

/// Reads frames of one segment by index. Owned by one thread (invariant 5).
pub trait SegmentReader: Send {
    /// Frame `index` as straight RGBA of `w`×`h` into `out`.
    fn frame(&mut self, index: u32, w: u32, h: u32, out: &mut Vec<u8>) -> Result<(), String>;
}

/// What makes and reads segments on this machine.
pub trait SegmentCodec: Send + Sync {
    /// The codec's name in the entries' headers (`h264`, `hevc`, a test's name).
    fn codec(&self) -> &str;
    /// Open a writer of `w`×`h` at `fps`; `Err` — no encoder this codec accepts opens (no hardware
    /// encoder: the tier is off on this machine, and the reason says so).
    fn writer(&self, path: &Path, w: u32, h: u32, fps: u32) -> Result<Box<dyn SegmentWriter>, String>;
    fn reader(&self, path: &Path) -> Result<Box<dyn SegmentReader>, String>;
    /// The segment's file extension.
    fn extension(&self) -> &str {
        "mkv"
    }
}

/// Raw RGBA frames one after another (tests, and the measure of what a codec saves): no encoder.
pub struct RawSegments;

struct RawWriter {
    file: std::io::BufWriter<std::fs::File>,
    n: u64,
}

impl SegmentWriter for RawWriter {
    fn push(&mut self, rgba: &[u8]) -> Result<(), String> {
        use std::io::Write;
        self.n += rgba.len() as u64;
        self.file.write_all(rgba).map_err(|e| e.to_string())
    }
    fn finish(self: Box<Self>) -> Result<u64, String> {
        let n = self.n;
        self.file.into_inner().map_err(|e| e.to_string())?.sync_all().ok();
        Ok(n)
    }
    fn encoder(&self) -> &str {
        "raw"
    }
}

struct RawReader(std::fs::File);

impl SegmentReader for RawReader {
    fn frame(&mut self, index: u32, w: u32, h: u32, out: &mut Vec<u8>) -> Result<(), String> {
        use std::io::{Read, Seek, SeekFrom};
        let len = w as usize * h as usize * 4;
        out.resize(len, 0);
        self.0.seek(SeekFrom::Start(index as u64 * len as u64)).map_err(|e| e.to_string())?;
        self.0.read_exact(out).map_err(|e| e.to_string())
    }
}

impl SegmentCodec for RawSegments {
    fn codec(&self) -> &str {
        "raw"
    }
    fn writer(&self, path: &Path, _w: u32, _h: u32, _fps: u32) -> Result<Box<dyn SegmentWriter>, String> {
        let f = std::fs::File::create(path).map_err(|e| e.to_string())?;
        Ok(Box::new(RawWriter { file: std::io::BufWriter::new(f), n: 0 }))
    }
    fn reader(&self, path: &Path) -> Result<Box<dyn SegmentReader>, String> {
        Ok(Box::new(RawReader(std::fs::File::open(path).map_err(|e| e.to_string())?)))
    }
    fn extension(&self) -> &str {
        "rgba"
    }
}

#[cfg(feature = "ffmpeg")]
pub mod ffmpeg {
    //! Segments through our runtime-loaded libav (invariants 9, 10): the export's encoder cascade with
    //! a real open check, every frame a keyframe (any moment decodes alone: a loop's wrap and a seek
    //! start at once), no audio, Matroska. Only a HARDWARE encoder is accepted unless the host forces
    //! one (`KT_RENDER_CACHE_ENCODER`, tests and the live check on a machine without one): the owner's
    //! rule, no processor codec in the preview's path.
    use super::*;
    use crate::{MediaSource, Quality};
    use kt_ffmpeg::format::{AudioCodec, Container, OutputFormat, RateMode, VideoCodec};
    use kt_time::Tick;

    /// The quality the encoders are asked for (CRF/CQ/QP: lower is better; 23 ≈ visually lossless).
    pub const QUALITY: u32 = 16;

    pub struct FfmpegSegments {
        /// `Some(name)` — this encoder whatever it is (the forced test path); `None` — the cascade,
        /// hardware only.
        pub forced: Option<String>,
    }

    impl FfmpegSegments {
        /// The cascade, hardware only; `KT_RENDER_CACHE_ENCODER` forces an encoder by name.
        pub fn from_env() -> FfmpegSegments {
            FfmpegSegments { forced: std::env::var("KT_RENDER_CACHE_ENCODER").ok().filter(|s| !s.is_empty()) }
        }
    }

    struct Writer {
        mux: Option<kt_ffmpeg::Muxer>,
        path: std::path::PathBuf,
        encoder: String,
    }

    impl SegmentWriter for Writer {
        fn push(&mut self, rgba: &[u8]) -> Result<(), String> {
            self.mux.as_mut().ok_or("closed")?.write_video(rgba).map_err(|e| e.to_string())
        }
        fn finish(mut self: Box<Self>) -> Result<u64, String> {
            self.mux.take().ok_or("closed")?.finish().map_err(|e| e.to_string())?;
            std::fs::metadata(&self.path).map(|m| m.len()).map_err(|e| e.to_string())
        }
        fn encoder(&self) -> &str {
            &self.encoder
        }
    }

    impl Drop for Writer {
        fn drop(&mut self) {
            // an abandoned segment leaves no half file behind
            if let Some(m) = self.mux.take() {
                let _ = m.finish();
                let _ = std::fs::remove_file(&self.path);
            }
        }
    }

    struct Reader {
        src: crate::ffmpeg::FfmpegSource,
        step: i64,
    }

    impl SegmentReader for Reader {
        fn frame(&mut self, index: u32, w: u32, h: u32, out: &mut Vec<u8>) -> Result<(), String> {
            let f = self
                .src
                .read_video_frame(Tick(index as i64 * self.step), Some(Quality { width: w, height: h }))
                .map_err(|e| format!("{e:?}"))?;
            if (f.width, f.height) != (w, h) {
                return Err(format!("a frame of {}×{} in a {w}×{h} segment", f.width, f.height));
            }
            out.clear();
            out.extend_from_slice(&f.rgba[..w as usize * h as usize * 4]);
            Ok(())
        }
    }

    impl SegmentCodec for FfmpegSegments {
        fn codec(&self) -> &str {
            "h264"
        }

        fn writer(&self, path: &Path, w: u32, h: u32, fps: u32) -> Result<Box<dyn SegmentWriter>, String> {
            let format = OutputFormat {
                container: Container::Mkv,
                video: VideoCodec::H264,
                rate: RateMode::Quality,
                // every frame a keyframe: any moment decodes alone
                keyint_tenths: 0,
                b_frames: 0,
                audio: AudioCodec::None,
                ..OutputFormat::default()
            };
            let cfg = kt_ffmpeg::MuxConfig {
                path: path.to_string_lossy().into_owned(),
                width: w,
                height: h,
                fps: fps.max(1),
                quality: QUALITY,
                video_bitrate: None,
                audio_bitrate: 0,
                force_encoder: self.forced.clone(),
                threads: 1,
                format,
            };
            let mux = kt_ffmpeg::Muxer::open(&cfg).map_err(|e| e.to_string())?;
            let report = mux.open_report().clone();
            if self.forced.is_none() && !report.hardware {
                let _ = mux.finish();
                let _ = std::fs::remove_file(path);
                let tried: Vec<String> = report.tried.iter().map(|(n, c)| format!("{n} ({c})")).collect();
                return Err(format!(
                    "no hardware encoder opens (the cascade reached {}; failed: {})",
                    report.encoder,
                    if tried.is_empty() { "none".to_string() } else { tried.join(", ") }
                ));
            }
            Ok(Box::new(Writer { mux: Some(mux), path: path.to_path_buf(), encoder: report.encoder }))
        }

        fn reader(&self, path: &Path) -> Result<Box<dyn SegmentReader>, String> {
            let src = crate::ffmpeg::FfmpegSource::open(&path.to_string_lossy()).map_err(|e| format!("{e:?}"))?;
            let (num, den) = src.info().fps.unwrap_or((25, 1));
            let step = kt_time::TICK_HZ * den.max(1) as i64 / num.max(1) as i64;
            Ok(Box::new(Reader { src, step }))
        }
    }
}
