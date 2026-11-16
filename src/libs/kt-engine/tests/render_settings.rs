//! Change render-settings (capability export, F-EXP-006): the chosen output format reaches the
//! written file. Every check reads the file back with OUR libav reader
//! (`kt_media::muxer::probe_output`), never an external binary. Requires the system FFmpeg;
//! an encoder this FFmpeg lacks is reported as skipped by the test, never passed silently.

use kt_engine::export::{ExportEvent, ExportOptions};
use kt_media::muxer::format::{AudioCodec, BitDepth, Container, Matrix, Range, RateMode, Speed, VideoCodec};
use kt_media::muxer::{OutputFormat, OutputProbe, probe_output};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::Tick;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

fn project(secs: i64) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    for (id, kind) in [(1u64, ClipKind::Video), (2, ClipKind::Audio)] {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            masks: Default::default(),
            id: ClipId(id),
            link_group: Some(GroupId(1)),
            kind,
            lane: 0,
            start: Tick(0),
            duration: Tick(90_000 * secs),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.validate().unwrap();
    p
}

fn sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    vec![(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 10), AudioPattern::Sine { hz: 440.0 })) as Box<dyn MediaSource>)]
}

fn has_encoder(name: &str) -> bool {
    kt_media::muxer::availability().video.iter().any(|(_, l)| l.iter().any(|e| e.name == name))
}

fn has_audio(a: AudioCodec) -> bool {
    kt_media::muxer::availability().has_audio(a)
}

/// The stats, the open report's skipped candidates and the announced frame total of one export.
type Run = (kt_engine::export::ExportStats, Vec<(String, String)>, u64);

/// One export with `opts`: the open report's skips and the stats, or the error.
fn export(
    p: &Project,
    out: &std::path::Path,
    opts: &ExportOptions,
) -> Result<Run, String> {
    let skipped = Arc::new(std::sync::Mutex::new(Vec::new()));
    let total = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let stats = {
        let skipped = skipped.clone();
        let total = total.clone();
        kt_engine::export_mp4_with_backends(
            p,
            sources(),
            sources(),
            &out.to_string_lossy(),
            None,
            opts,
            &kt_gpu::testkit::backends(),
            &Arc::new(AtomicBool::new(false)),
            &Arc::new(AtomicBool::new(false)),
            move |e| match e {
                ExportEvent::Opened(r) => skipped.lock().unwrap().extend(r.skipped.clone()),
                ExportEvent::Progress(pr) => total.store(pr.total_video_frames, std::sync::atomic::Ordering::Relaxed),
                _ => {}
            },
        )
        .map_err(|e| e.to_string())?
    };
    let s = skipped.lock().unwrap().clone();
    Ok((stats, s, total.load(std::sync::atomic::Ordering::Relaxed)))
}

fn probe(out: &std::path::Path) -> OutputProbe {
    probe_output(&out.to_string_lossy()).expect("our reader opens the output")
}

/// t2802 (render-settings): the container, codec, profile, pixel format, keyframe interval,
/// B-frames, rate, audio codec and title of the asked format are read back from the file; a
/// container refuses a codec it cannot hold before a file exists.
#[test]
fn t2802_the_chosen_settings_are_read_back_from_the_file() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let p = project(1);
    let mut skipped = Vec::new();

    // libx264 VBR into MKV with FLAC, keyframes every 0.5 s, 2 B-frames, a title
    if has_encoder("libx264") && has_audio(AudioCodec::Flac) {
        let out = dir.path().join("a.mkv");
        let opts = ExportOptions {
            force_encoder: Some("libx264".into()),
            video_bitrate: Some(400_000),
            format: OutputFormat {
                container: Container::Mkv,
                rate: RateMode::Vbr,
                max_bitrate: Some(600_000),
                keyint_tenths: 5,
                b_frames: 2,
                audio: AudioCodec::Flac,
                title: "Render settings".into(),
                comment: "t2802".into(),
                ..Default::default()
            },
            ..Default::default()
        };
        export(&p, &out, &opts).expect("libx264 into MKV");
        let f = probe(&out);
        assert!(f.format.contains("matroska"), "{f:?}");
        assert_eq!((f.video_codec.as_str(), f.profile, f.pix_fmt.as_str()), ("h264", 100, "yuv420p"), "High, 8-bit");
        assert_eq!(f.packets.len(), 30);
        // the interval is the encoder's MAXIMUM: a scene cut may bring a key earlier
        let keys = f.keyframes();
        assert!(keys.len() >= 2 && keys.windows(2).all(|w| w[1] - w[0] <= 15), "a keyframe at least every 0.5 s: {keys:?}");
        assert!(f.reordered(), "B-frames were asked");
        assert!(f.video_bitrate() <= 600_000.0 * 1.25, "the maximum holds: {}", f.video_bitrate());
        assert_eq!(f.audio_codec.as_deref(), Some("flac"));
        assert_eq!(f.audio_bits, 24, "FLAC carries 24 bits");
        assert_eq!(f.title.as_deref(), Some("Render settings"));
        assert_eq!(f.comment.as_deref(), Some("t2802"));
    } else {
        skipped.push("libx264+flac");
    }

    // the default file: MP4, H.264 without B-frames, 2 s keyframes, AAC — now tagged BT.601 limited
    if has_encoder("libx264") {
        let out = dir.path().join("d.mp4");
        export(&p, &out, &ExportOptions { force_encoder: Some("libx264".into()), ..Default::default() }).unwrap();
        let f = probe(&out);
        assert!(!f.reordered(), "no B-frames by default");
        // the synthetic picture changes colour every frame: x264 cuts scenes; the bound is two seconds
        assert_eq!(f.keyframes()[0], 0);
        assert_eq!(f.audio_codec.as_deref(), Some("aac"));
        assert_eq!(f.color, (6, 1, 1, 1), "smpte170m, limited, bt709 primaries and transfer");
        assert_eq!(f.title, None, "no metadata asked, none written");
    }

    // libx265 10-bit into MP4: Main 10, `hvc1`
    if has_encoder("libx265") {
        let out = dir.path().join("b.mp4");
        let opts = ExportOptions {
            force_encoder: Some("libx265".into()),
            format: OutputFormat { video: VideoCodec::H265, depth: BitDepth::Ten, ..Default::default() },
            ..Default::default()
        };
        export(&p, &out, &opts).expect("libx265 10-bit");
        let f = probe(&out);
        assert_eq!((f.video_codec.as_str(), f.profile, f.pix_fmt.as_str()), ("hevc", 2, "yuv420p10le"), "Main 10");
        assert_eq!(f.video_tag, u32::from_le_bytes(*b"hvc1"));
    } else {
        skipped.push("libx265");
    }

    // libvpx-vp9 into WebM with Opus
    if has_encoder("libvpx-vp9") && has_audio(AudioCodec::Opus) {
        let out = dir.path().join("c.webm");
        let opts = ExportOptions {
            format: OutputFormat {
                container: Container::Webm,
                video: VideoCodec::Vp9,
                audio: AudioCodec::Opus,
                speed: Speed::Fastest,
                ..Default::default()
            },
            ..Default::default()
        };
        let (stats, _, _) = export(&p, &out, &opts).expect("VP9 into WebM");
        assert_eq!(stats.encoder, "libvpx-vp9", "VP9's cascade, Auto");
        let f = probe(&out);
        assert_eq!((f.video_codec.as_str(), f.profile), ("vp9", 0));
        assert_eq!(f.audio_codec.as_deref(), Some("opus"));
    } else {
        skipped.push("libvpx-vp9+opus");
    }

    // prores_ks into MOV with PCM 24
    if has_encoder("prores_ks") {
        let out = dir.path().join("e.mov");
        let opts = ExportOptions {
            format: OutputFormat { container: Container::Mov, video: VideoCodec::ProRes, audio: AudioCodec::Pcm24, ..Default::default() },
            ..Default::default()
        };
        export(&p, &out, &opts).expect("ProRes into MOV");
        let f = probe(&out);
        assert_eq!((f.video_codec.as_str(), f.profile, f.pix_fmt.as_str()), ("prores", 3, "yuv422p10le"), "ProRes HQ");
        assert_eq!(f.keyframes().len(), 30, "intra-only");
        assert_eq!(f.audio_codec.as_deref(), Some("pcm_s24le"));
    } else {
        skipped.push("prores_ks");
    }

    // no audio stream
    if has_encoder("libx264") {
        let out = dir.path().join("n.mp4");
        let opts = ExportOptions {
            force_encoder: Some("libx264".into()),
            format: OutputFormat { audio: AudioCodec::None, ..Default::default() },
            ..Default::default()
        };
        export(&p, &out, &opts).expect("no audio");
        assert_eq!(probe(&out).audio_codec, None);
    }

    // a container refuses what it cannot hold, before a file exists
    for (f, words) in [
        (OutputFormat { video: VideoCodec::ProRes, ..Default::default() }, "MP4 cannot hold ProRes 422 HQ"),
        (OutputFormat { container: Container::Webm, video: VideoCodec::Vp9, ..Default::default() }, "WebM cannot hold AAC"),
    ] {
        let out = dir.path().join("refused.out");
        let e = export(&p, &out, &ExportOptions { format: f, ..Default::default() }).unwrap_err();
        assert!(e.contains(words), "{e}");
        assert!(!out.exists(), "no file for a refused pair");
    }
    if !skipped.is_empty() {
        eprintln!("t2802: not in this FFmpeg, not checked: {skipped:?}");
    }
}

/// t2803 (render-settings D3): a candidate that cannot take 10-bit is skipped with the reason, a
/// forced one fails naming it; CBR reaches the file.
#[test]
fn t2803_what_a_candidate_cannot_do_is_named() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) || !has_encoder("libx264") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let p = project(1);
    let ten = OutputFormat { depth: BitDepth::Ten, ..Default::default() };
    let (stats, skipped, _) =
        export(&p, &dir.path().join("a.mp4"), &ExportOptions { format: ten.clone(), ..Default::default() }).expect("Auto 10-bit");
    assert_eq!(stats.encoder, "libx264");
    let present: Vec<String> = kt_media::muxer::encoder_cascade().into_iter().filter(|n| has_encoder(n) && n != "libx264" && n != "mpeg4").collect();
    for n in &present {
        assert!(
            skipped.iter().any(|(s, why)| s == n && why == "no 10-bit input"),
            "{n} skipped with its reason: {skipped:?}"
        );
    }
    assert_eq!(probe(&dir.path().join("a.mp4")).pix_fmt, "yuv420p10le");

    let forced = ExportOptions { force_encoder: Some("h264_vaapi".into()), format: ten, ..Default::default() };
    let e = export(&p, &dir.path().join("b.mp4"), &forced).unwrap_err();
    assert!(e.contains("h264_vaapi: no 10-bit input"), "{e}");

    // CBR through libx264's HRD: the file's rate is the asked one
    let out = dir.path().join("c.mp4");
    let cbr = ExportOptions {
        force_encoder: Some("libx264".into()),
        video_bitrate: Some(500_000),
        format: OutputFormat { rate: RateMode::Cbr, ..Default::default() },
        ..Default::default()
    };
    export(&project(2), &out, &cbr).expect("CBR");
    let rate = probe(&out).video_bitrate();
    assert!((rate - 500_000.0).abs() < 500_000.0 * 0.25, "CBR at {rate}");
}

/// t2804 (render-settings D1): a frame-rate override renders that many frames at that rate.
#[test]
fn t2804_a_frame_rate_override_renders_that_many_frames() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) || !has_encoder("libx264") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("f.mp4");
    let opts = ExportOptions { force_encoder: Some("libx264".into()), fps: Some(25), ..Default::default() };
    let (stats, _, total) = export(&project(1), &out, &opts).unwrap();
    assert_eq!((stats.video_frames, total), (25, 25));
    let f = probe(&out);
    assert_eq!(f.packets.len(), 25);
    assert_eq!(f.fps.0 as f64 / f.fps.1 as f64, 25.0);
}

/// t2805 (render-settings, invariant 11): the same settings write the same bytes with the new
/// software encoders at fixed threads. AV1 is not covered.
#[test]
fn t2805_the_same_settings_write_the_same_bytes() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let p = project(1);
    for (name, f) in [
        ("libx265", OutputFormat { video: VideoCodec::H265, b_frames: 2, ..Default::default() }),
        ("libvpx-vp9", OutputFormat { container: Container::Mkv, video: VideoCodec::Vp9, audio: AudioCodec::Pcm16, ..Default::default() }),
        ("prores_ks", OutputFormat { container: Container::Mov, video: VideoCodec::ProRes, ..Default::default() }),
    ] {
        if !has_encoder(name) {
            eprintln!("t2805: {name} not in this FFmpeg, not checked");
            continue;
        }
        let opts = ExportOptions { force_encoder: Some(name.into()), encoder_threads: Some(1), format: f.clone(), ..Default::default() };
        let ext = f.container.extension();
        let (a, b) = (dir.path().join(format!("{name}-1.{ext}")), dir.path().join(format!("{name}-2.{ext}")));
        export(&p, &a, &opts).unwrap();
        export(&p, &b, &opts).unwrap();
        assert!(std::fs::read(&a).unwrap() == std::fs::read(&b).unwrap(), "{name}: two runs differ");
    }
}

/// t2806 (render-settings D6): the tags state the conversion. A saturated red written with each
/// matrix and range is tagged so, and read back by a BT.601 reader the BT.709 file's red comes back
/// shifted — the conversion itself changed, not only the label.
#[test]
fn t2806_the_colour_tags_are_the_conversion() {
    if !has_encoder("libx264") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let red: Vec<u8> = [230u8, 20, 20, 255].repeat(64 * 64);
    let mut back = Vec::new();
    for (i, (m, r, tags)) in [
        (Matrix::Bt601, Range::Limited, (6, 1, 1, 1)),
        (Matrix::Bt709, Range::Limited, (1, 1, 1, 1)),
        (Matrix::Bt601, Range::Full, (6, 2, 1, 1)),
    ]
    .into_iter()
    .enumerate()
    {
        let path = dir.path().join(format!("c{i}.mp4")).to_string_lossy().into_owned();
        let mut mux = kt_media::muxer::Muxer::open(&kt_media::muxer::MuxConfig {
            path: path.clone(),
            width: 64,
            height: 64,
            fps: 30,
            quality: 0,
            video_bitrate: None,
            audio_bitrate: 128_000,
            force_encoder: Some("libx264".into()),
            threads: 1,
            format: OutputFormat { matrix: m, range: r, ..Default::default() },
        })
        .unwrap();
        for _ in 0..3 {
            mux.write_video(&red).unwrap();
        }
        mux.finish().unwrap();
        let f = probe_output(&path).unwrap();
        assert_eq!(f.color, tags, "{m:?} {r:?}");
        let mut src = kt_media::ffmpeg::FfmpegSource::open(&path).unwrap();
        let fr = src.read_video_frame(Tick(0), Some(Quality { width: 64, height: 64 })).unwrap();
        let c = 32 * 64 * 4 + 32 * 4;
        back.push([fr.rgba[c] as i32, fr.rgba[c + 1] as i32, fr.rgba[c + 2] as i32]);
    }
    let d = |a: [i32; 3], b: [i32; 3]| (0..3).map(|k| (a[k] - b[k]).abs()).max().unwrap();
    assert!(d(back[0], [230, 20, 20]) <= 6, "the default file comes back as written: {:?}", back[0]);
    assert!(d(back[1], back[0]) > 5, "BT.709 is another conversion: {:?} vs {:?}", back[1], back[0]);
}
