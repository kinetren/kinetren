//! Test 315 (R.46, item 321): SHARPNESS. The meaning of the whole round in one metric —
//! a zoom above 100% on a file with native headroom must show the details
//! of the file, not the blur of a frame compressed to Quality already at decode.
//!
//! The file is self-contained ON PURPOSE (its own source, only stable APIs):
//! it is copied into a worktree at the previous commit, and it is confirmed to be
//! red there — otherwise the test proves nothing.

use kt_media::{MediaError, MediaInfo, MediaSource, Quality, VideoFrame};
use kt_model::*;
use kt_time::Tick;

const SEC: i64 = 90_000;
const W: u32 = 128;
const H: u32 = 72;
/// The native is four times larger than the project: headroom for scale 2 is more than enough.
const NW: u32 = 512;
const NH: u32 = 288;

/// A source with real texture: stripes 2 px wide AT NATIVE and honest
/// box minification. Halving keeps them, quartering blurs them to grey.
/// (A copy of `kt_media::synthetic::CheckerSource`: the test must compile
/// on the commit BEFORE the round where it does not yet exist.)
struct Checker;

impl Checker {
    fn native_px(x: u32) -> u8 {
        if (x / 2).is_multiple_of(2) { 16 } else { 239 }
    }
}

impl MediaSource for Checker {
    fn info(&self) -> &MediaInfo {
        static INFO: std::sync::OnceLock<MediaInfo> = std::sync::OnceLock::new();
        INFO.get_or_init(|| MediaInfo {
            duration: Tick(10 * SEC),
            has_video: true,
            has_audio: false,
            fps: Some((30, 1)),
            width: NW,
            height: NH,
            sar: (1, 1),
            still: false,
        })
    }

    fn read_audio_burst(&mut self, idx: i64) -> Result<kt_media::AudioBurst, MediaError> {
        Ok(kt_media::AudioBurst::silence(idx))
    }

    fn read_video_frame(
        &mut self,
        pts: Tick,
        quality: Option<Quality>,
    ) -> Result<VideoFrame, MediaError> {
        let q = quality.unwrap_or(Quality { width: NW, height: NH });
        let (w, h) = (q.width.max(2) & !1, q.height.max(2) & !1);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for x in 0..w {
            let x0 = (x as u64 * NW as u64 / w as u64) as u32;
            let x1 = (((x as u64 + 1) * NW as u64).div_ceil(w as u64) as u32).max(x0 + 1).min(NW);
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

/// Mean horizontal contrast of the red channel (0..255): a measure of
/// whether the texture survived to the output at all.
fn contrast(rgba: &[u8], w: u32, h: u32) -> f64 {
    let mut sum = 0u64;
    let mut n = 0u64;
    for y in 0..h {
        for x in 1..w {
            let i = ((y * w + x) * 4) as usize;
            let j = ((y * w + x - 1) * 4) as usize;
            sum += (rgba[i] as i64 - rgba[j] as i64).unsigned_abs();
            n += 1;
        }
    }
    sum as f64 / n as f64
}

fn project(fx: Vec<FxInstance>) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: W,
        height: H,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "checker"));
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(SEC),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx,
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

/// First frame of the project's export (the same path as the preview).
fn first_frame(p: &Project) -> Vec<u8> {
    let sources: Vec<(MediaId, Box<dyn MediaSource>)> = vec![(MediaId(1), Box::new(Checker))];
    let mut out = Vec::new();
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    kt_engine::export::render_raw_video_range_with(
        p,
        sources,
        Quality { width: W, height: H },
        Some((Tick(0), Tick(3000))),
        &Default::default(),
        &cancel,
        |_| {},
        |frame| {
            if out.is_empty() {
                out.extend_from_slice(frame);
            }
            Ok(())
        },
    )
    .expect("render");
    out
}

/// Test 315: a ×2 zoom on a file with native headroom must be SHARP.
///
/// Project 128×72, file 512×288. Before R.46 the decoder gave the frame straight
/// at 128×72 — the file's texture (2 px native stripes) was averaged into an even
/// grey BEFORE the chain, so there was nothing left to zoom. With the decode scale
/// the frame arrives at 256×144, where the stripes are alive, and the ×2 zoom puts
/// them on the output one to one.
#[test]
fn t315_zoom_beyond_quality_is_sharp() {
    let zoomed = first_frame(&project(vec![FxInstance::geometry_zoom(2.0)]));
    let c = contrast(&zoomed, W, H);
    println!("t315 contrast: {c:.1}");
    // before R.46 it is exactly 0.0 (grey); the threshold has a wide margin below the
    // measured value (full stripe contrast ≈ 223)
    assert!(c >= 100.0, "zoom past 100 % must show the texture of the file, contrast {c:.1}");

    // a control: without zoom there is no scale, and the frame is honestly blurry — the feature
    // cannot be “won” by inflating the decode for everything
    let plain = first_frame(&project(vec![]));
    let cp = contrast(&plain, W, H);
    println!("t315 plain contrast: {cp:.1}");
    assert!(cp < 1.0, "without net zoom the decode stays at Quality, contrast {cp:.1}");
}
