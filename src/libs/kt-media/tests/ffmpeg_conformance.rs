//! Test 10: conformance of FfmpegSource on real
//! fixtures (src/fixtures/gen.sh, generated on the first run).
#![cfg(feature = "ffmpeg")]

use kt_media::ffmpeg::FfmpegSource;
use kt_media::{MediaSource, Quality};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, Tick};

const CH: usize = AUDIO_CHANNELS;

/// gen.sh runs once per process (tests run in parallel).
static GEN: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn fixture(name: &str) -> String {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    // on Windows the fixtures come generated with the test archive (no bash to run gen.sh)
    let pregenerated = cfg!(windows) && dir.join("out").is_dir();
    GEN.get_or_init(|| {
        if pregenerated {
            return;
        }
        let st = std::process::Command::new("bash")
            .arg(dir.join("gen.sh"))
            .status()
            .expect("run fixtures/gen.sh (needs ffmpeg in PATH)");
        assert!(st.success(), "gen.sh failed");
    });
    dir.join("out").join(name).to_string_lossy().to_string()
}

// ---- Audio ------------------------------------------------------------------

#[test]
fn t10_flac_sequential_matches_seeked_reads_exactly() {
    // Lossless 48k: burst n after seek == burst n when decoded from scratch, exact.
    let path = fixture("sine48k_stereo.flac");
    let mut seq = FfmpegSource::open(&path).unwrap();
    let mut reference = Vec::new();
    for idx in 0..40 {
        reference.push(seq.read_audio_burst(idx).unwrap());
    }
    // a fresh source, scattered reads (each one — via the seek path)
    let mut rnd = FfmpegSource::open(&path).unwrap();
    for &idx in &[25i64, 3, 39, 0, 17, 8, 30, 1] {
        let b = rnd.read_audio_burst(idx).unwrap();
        assert_eq!(
            b.data[..],
            reference[idx as usize].data[..],
            "burst {idx}: the seek path must be sample-accurate (lossless)"
        );
    }
}

#[test]
fn t10_aac_441_resampled_alignment_and_length() {
    let path = fixture("sine44k_mono.m4a");
    let mut src = FfmpegSource::open(&path).unwrap();
    assert!(src.info().has_audio);

    // length: 2 s @ 44.1k → ~96000 frames at 48k (the encoder's priming/padding
    // is allowed within a pair of bursts)
    let mut total = 0i64;
    let mut idx = 0i64;
    while let Ok(b) = src.read_audio_burst(idx) {
        // end: a fully zero burst after the signal
        let silent = b.data.iter().all(|&s| s == 0.0);
        idx += 1;
        if silent && total > 0 {
            break;
        }
        total = idx * AUDIO_BURST_FRAMES as i64;
        if idx > 120 {
            break;
        }
    }
    assert!(
        (total - 2 * AUDIO_SAMPLE_RATE as i64).abs() <= 3 * AUDIO_BURST_FRAMES as i64,
        "2 s 44.1k → ~96000 frames 48k, got {total}"
    );

    // sample-accurate alignment: seek-read against sequential, we allow a lossy
    // difference in value, but NOT in phase: we search for the shift of maximum
    // correlation — it must be 0.
    let mut seq = FfmpegSource::open(&path).unwrap();
    let mut reference = Vec::new();
    for i in 0..30 {
        reference.push(seq.read_audio_burst(i).unwrap());
    }
    let mut rnd = FfmpegSource::open(&path).unwrap();
    let probe = rnd.read_audio_burst(20).unwrap();
    let r = &reference[20].data;
    let best_shift = (-8i64..=8)
        .max_by_key(|&shift| {
            let mut acc = 0f64;
            for f in 8..(AUDIO_BURST_FRAMES as i64 - 8) {
                let a = probe.data[(f * CH as i64) as usize] as f64;
                let b = r[((f + shift) * CH as i64) as usize] as f64;
                acc += a * b;
            }
            (acc * 1e6) as i64
        })
        .unwrap();
    assert_eq!(best_shift, 0, "the AAC seek path shifted by {best_shift} samples");
}

// ---- Video ------------------------------------------------------------------

#[test]
fn t10_h264_backward_seek_is_deterministic() {
    let path = fixture("testsrc_h264.mp4");
    let mut a = FfmpegSource::open(&path).unwrap();
    assert!(a.info().has_video);
    assert_eq!(a.info().fps, Some((30, 1)));

    let frame_tick = |n: i64| Tick(n * 3000); // 30 fps → 3000 ticks/frame
    let q = Some(Quality { width: 320, height: 180 });

    // direct access to frame 5 from a fresh source
    let direct = a.read_video_frame(frame_tick(5), q).unwrap();
    // the same source: go forward to frame 40, return to 5 (backward seek
    // + decoder recreation — MT H.264 path)
    let _far = a.read_video_frame(frame_tick(40), q).unwrap();
    let back = a.read_video_frame(frame_tick(5), q).unwrap();
    assert_eq!(direct.rgba, back.rgba, "backward seek H.264 must be deterministic");
    assert_eq!(direct.width, 320);
    assert_eq!(direct.height, 180);
}

#[test]
fn t10_vp9_decodes_and_seeks() {
    let path = fixture("testsrc_vp9.webm");
    let mut src = FfmpegSource::open(&path).unwrap();
    let q = Some(Quality { width: 160, height: 90 });
    let f10 = src.read_video_frame(Tick(10 * 3000), q).unwrap();
    let f2 = src.read_video_frame(Tick(2 * 3000), q).unwrap();
    let f10b = src.read_video_frame(Tick(10 * 3000), q).unwrap();
    assert_eq!(f10.rgba, f10b.rgba, "VP9 (threads=1) seek there-and-back is deterministic");
    assert_ne!(f2.rgba, f10.rgba, "testsrc frames 2 and 10 differ");
}

#[test]
fn t10_keyframe_index_and_quality_ladder() {
    let path = fixture("muxed_av.mp4");
    let mut src = FfmpegSource::open(&path).unwrap();
    assert!(src.info().has_video && src.info().has_audio);

    // keyframe_before from the packet index (g=15 → a keyframe every 0.5 s)
    let kf = src.keyframe_before(Tick(90_000)).expect("keyframe before 1.0 s");
    assert!(kf.0 <= 90_000 && kf.0 >= 90_000 - 16 * 3000, "kf={kf:?}");

    // quality ladder: one pts, different sizes
    let big =
        src.read_video_frame(Tick(30 * 3000), Some(Quality { width: 320, height: 180 })).unwrap();
    let small =
        src.read_video_frame(Tick(30 * 3000), Some(Quality { width: 160, height: 90 })).unwrap();
    assert_eq!((big.width, big.height), (320, 180));
    assert_eq!((small.width, small.height), (160, 90));

    // and audio is read from a muxed file
    let b = src.read_audio_burst(10).unwrap();
    assert!(b.data.iter().any(|&s| s.abs() > 0.05), "audio of the muxed file is audible");
}

// ---- ogg with cover art (regression item 26) ----------------------------------------

#[test]
fn t10_ogg_cover_art_no_abort_and_decodes() {
    // Previously: the mjpeg cover was counted as a video stream, and its stream_index from
    // the scan context went into av_seek_frame of a new ogg context →
    // av_assert0(stream_index < ogg->nstreams) → abort of the WHOLE process.
    let mut src = FfmpegSource::open(&fixture("cover_art.ogg")).expect("opening ogg");
    let info = src.info().clone();
    assert!(info.has_audio, "the vorbis stream must be visible");
    assert!(!info.has_video, "cover art (attached_pic) is not video");

    // The audio decodes and is not silence.
    let b = src.read_audio_burst(0).expect("burst 0");
    let energy: f32 = b.data.iter().map(|s| s * s).sum();
    assert!(energy > 1e-3, "vorbis burst 0 must not be silence, energy={energy}");

    // Seek forward and back does not crash (that very av_seek_frame path).
    let far = (AUDIO_SAMPLE_RATE as i64) / (AUDIO_BURST_FRAMES as i64); // ~1 c
    let b1 = src.read_audio_burst(far).expect("burst after seek forward");
    let b0 = src.read_audio_burst(0).expect("burst after seek backward");
    let e1: f32 = b1.data.iter().map(|s| s * s).sum();
    let e0: f32 = b0.data.iter().map(|s| s * s).sum();
    assert!(e1 > 1e-3 && e0 > 1e-3);

    // A video frame request on an audio file is an error, not a crash.
    assert!(
        src.read_video_frame(Tick(0), Some(Quality { width: 64, height: 64 })).is_err(),
        "read_video_frame on a file without video must return an error"
    );
    let _ = CH;
}

// ---- Span decode (test 21, items 14/15) ------------------------------------------

#[test]
fn t21_ffmpeg_span_equals_per_frame_and_no_reopen() {
    use kt_time::TICK_HZ;
    let path = fixture("testsrc_h264.mp4");
    let step = TICK_HZ / 30;
    let keys: Vec<kt_time::Tick> = (0..60).map(|i| kt_time::Tick(i * step)).collect();
    let q = Some(Quality { width: 160, height: 90 });

    // reference: per-frame calls
    let mut per_frame = FfmpegSource::open(&path).unwrap();
    let mut expected = Vec::new();
    for &k in &keys {
        expected.push(per_frame.read_video_frame(k, q).expect("per-frame decode"));
    }

    // span: one forward pass
    let mut span_src = FfmpegSource::open(&path).unwrap();
    let mut got: Vec<(kt_time::Tick, kt_media::VideoFrame)> = Vec::new();
    span_src.read_video_span(&keys, q, &mut |k, f| got.push((k, f))).expect("span decode");

    assert_eq!(got.len(), keys.len(), "the span must cover every keyframe");
    for (i, (k, f)) in got.iter().enumerate() {
        assert_eq!(*k, keys[i]);
        assert_eq!(f.rgba, expected[i].rgba, "frame of keyframe {i} diverged from the per-frame reference");
    }
    // the GOP was decoded once: after the initial seek — zero recreations,
    // frames decoded not much more than the number of keys
    let (reopens, decoded) = span_src.decode_debug_stats();
    assert!(reopens <= 1, "the span must not recreate the decoder: reopens={reopens}");
    assert!(decoded <= 62, "the span decodes each frame once: decoded={decoded}");
}

// ---- Packet index in the sidecar DB -------------------------------------------

/// Test 34: persistence of the packet index.
/// A second opening of the same file does NOT scan it again (the index comes
/// from the DB), the index is byte-for-byte equal to a fresh scan and functionally working
/// (keyframe_before, sample-accurate audio). A change of the file invalidates the
/// index — a full scan again.
#[test]
fn t34_packet_index_persisted_and_reused() {
    use kt_media::index::MediaIndexDb;

    // a copy of the fixture — the test changes the file at the invalidation step
    let dir = std::env::temp_dir().join(format!("kt_t34_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let media = dir.join("muxed_av.mp4");
    std::fs::copy(fixture("muxed_av.mp4"), &media).unwrap();
    let media_s = media.to_string_lossy().to_string();
    let db_path = dir.join("t34.ktindex.db");
    let _ = std::fs::remove_file(&db_path);
    let mut db = MediaIndexDb::open(&db_path).unwrap();

    // 1st opening: scan + persist
    let s1 = FfmpegSource::open_indexed(&media_s, Some(&mut db)).unwrap();
    assert!(!s1.index_was_prebuilt(), "the first open must scan");
    let (a1, v1) = s1.packet_index();
    assert!(!a1.is_empty() && !v1.is_empty(), "muxed_av has both streams");

    // 2nd opening: index from the DB, no scan
    let mut s2 = FfmpegSource::open_indexed(&media_s, Some(&mut db)).unwrap();
    assert!(s2.index_was_prebuilt(), "a repeated open must not scan the file");
    assert_eq!(s1.packet_index(), s2.packet_index(), "index from the DB != fresh scan");

    // the index from the DB is functionally equivalent: keyframe_before and audio
    let mut fresh = FfmpegSource::open(&media_s).unwrap();
    for t in [0i64, 30_000, 90_000, 150_000] {
        assert_eq!(
            s2.keyframe_before(Tick(t)),
            fresh.keyframe_before(Tick(t)),
            "keyframe_before({t}) diverged"
        );
    }
    let want = fresh.read_audio_burst(7).unwrap();
    let got = s2.read_audio_burst(7).unwrap();
    assert_eq!(&got.data[..], &want.data[..], "decode with the index from the DB diverged");

    // file edit → identity does not match → scan again
    {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new().append(true).open(&media).unwrap();
        f.write_all(&[0u8]).unwrap();
    }
    let s3 = FfmpegSource::open_indexed(&media_s, Some(&mut db)).unwrap();
    assert!(!s3.index_was_prebuilt(), "a changed file must be rescanned");

    let _ = std::fs::remove_dir_all(&dir);
}

/// Test 37 (I/O part, performance panel): the kt-ffmpeg counters are live — opening
/// and reading the file move opens/packet_reads (port of v1's avcpp metrics; in v1
/// a third of the panel's metrics were dead zeros — this test keeps the panel
/// from showing dead series again).
#[test]
fn t37_io_counters_alive() {
    let (o0, r0) = kt_media::ffmpeg::io_stats();
    let mut s = FfmpegSource::open(&fixture("muxed_av.mp4")).unwrap();
    let _ = s.read_audio_burst(0);
    let (o1, r1) = kt_media::ffmpeg::io_stats();
    assert!(o1 > o0, "open must increment opens (was {o0}, now {o1})");
    assert!(r1 > r0, "scan/decode must increment packet_reads (was {r0}, now {r1})");
}

/// Test 112 (item 14, round 14): frame sampling — ONE semantics, floor.
///
/// The source is 25 fps, keys on the project's 60 fps grid: the grids do not match,
/// so previously `read_video_frame` went sometimes by floor (after a seek), sometimes by
/// ceil (the forward path, the previous frame already handed out and freed) — frames in
/// export went `f0, f1, f0, f2, f1, f3…`, the user saw step-backs.
/// The span was always floor, so it serves as the reference.
#[test]
fn t112_video_frame_selection_is_floor_on_mismatched_fps() {
    use kt_time::TICK_HZ;
    let path = fixture("testsrc_25fps.mp4");
    let step = TICK_HZ / 60; // the PROJECT grid is finer than the source's grid
    let keys: Vec<kt_time::Tick> = (0..90).map(|i| kt_time::Tick(i * step)).collect();
    let q = Some(Quality { width: 160, height: 90 });

    let mut span_src = FfmpegSource::open(&path).unwrap();
    let mut expected: Vec<kt_media::VideoFrame> = Vec::new();
    span_src.read_video_span(&keys, q, &mut |_, f| expected.push(f)).expect("span decode");
    assert_eq!(expected.len(), keys.len());

    let mut per_frame = FfmpegSource::open(&path).unwrap();
    let mut got = Vec::new();
    for &k in &keys {
        got.push(per_frame.read_video_frame(k, q).expect("per-frame decode"));
    }

    // 1) the per-frame path matches the span FRAME FOR FRAME
    for i in 0..keys.len() {
        assert_eq!(
            got[i].rgba, expected[i].rgba,
            "keyframe {i}: per-frame decode returned a different frame than the span (floor)"
        );
    }

    // 2) frames do not step back: each next one is either the same or a new one,
    //    once a frame has changed it does not return
    let mut seen: Vec<&[u8]> = Vec::new();
    for f in &got {
        let cur: &[u8] = &f.rgba;
        if seen.last().map(|p| *p == cur).unwrap_or(false) {
            continue;
        }
        assert!(!seen.contains(&cur), "a frame repeated AFTER the switch — this is a backward step");
        seen.push(cur);
    }
    // a 25 fps source over 1.5 s of a 60 fps grid gives ~38 different frames
    assert!(seen.len() >= 30, "too few distinct frames: {}", seen.len());

    // 3) the forward pass does not suit the seek storm
    let (reopens, _) = per_frame.decode_debug_stats();
    assert!(reopens <= 1, "the per-frame path recreated the decoder {reopens} times");

    // 4) TAIL: keys at the very end of the source, and beyond it,
    // must return the last frame, not an error — otherwise the export
    // draws BLACK frames at the end
    // (checked live: the old code gave “missing layers 2” and a black last frame).
    let dur = kt_media::MediaSource::info(&per_frame).duration;
    let mut tail = FfmpegSource::open(&path).unwrap();
    for i in 0..4 {
        let k = kt_time::Tick(dur.0 - step + i * step);
        assert!(
            tail.read_video_frame(k, q).is_ok(),
            "keyframe at the end of the source ({}) returned no frame — the export would be black",
            k.0
        );
    }
}

// ---- Pixel aspect ratio (R.62, t711) ----------------------------------------

/// Test 711: SAR travels from the stream to the frame's form. Two files with the SAME
/// 640×480 pixels and SAR 1:1 / 2:1: `info()` gives different forms (4:3 and
/// 8:3), while decoding at equal Quality is byte-for-byte one. Before R.62
/// anamorphic was inexpressible: both files gave the same stretched frame.
#[test]
fn t711_sar_reaches_the_view_and_only_the_view() {
    let mut a = FfmpegSource::open(&fixture("testsrc_sar11.mp4")).unwrap();
    let mut b = FfmpegSource::open(&fixture("testsrc_sar21.mp4")).unwrap();
    assert_eq!((a.info().width, a.info().height), (640, 480));
    assert_eq!((b.info().width, b.info().height), (640, 480));
    assert_eq!(a.info().sar, (1, 1), "square pixel — 1:1");
    assert_eq!(b.info().sar, (2, 1), "stream sample_aspect_ratio read");
    assert_eq!(a.info().view_size(), (640.0, 480.0), "4:3 view");
    assert_eq!(b.info().view_size(), (1280.0, 480.0), "8:3 view: width × SAR");
    // the pixels are the same: SAR does not touch the decode, it touches only the FORM
    let q = Some(Quality { width: 320, height: 240 });
    let fa = a.read_video_frame(Tick(3600 * 5), q).unwrap();
    let fb = b.read_video_frame(Tick(3600 * 5), q).unwrap();
    assert_eq!((fa.width, fa.height), (320, 240));
    assert_eq!(fa.rgba, fb.rgba, "frame of exactly the requested quality; SAR does not stretch the decode");
}

// ---- R.67: single picture ------------------------------------------------

/// Test 796: a SINGLE PICTURE is a full-fledged source. One frame at ANY tick
/// (it has no frame grid of its own), a “practically infinite” duration (an event of any
/// length shows it whole), audio is silence, and a PNG with alpha
/// comes through with its transparency.
///
/// The owner: “static pictures do not display at all, ever.. there is an eternal
/// Loading...”. There was not a single fixture with stills — the path
/// had never been under test.
#[test]
fn t796_a_single_image_is_a_source_with_one_frame_at_any_tick() {
    for name in ["still.jpg", "still_alpha.png"] {
        let path = fixture(name);
        let mut src = FfmpegSource::open(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
        let info = src.info().clone();
        assert!(info.has_video, "{name}: still image without video");
        assert!(!info.has_audio, "{name}: the still image got audio");
        assert!(info.still, "{name}: source not recognized as a single still image");
        assert_eq!(info.fps, None, "{name}: a still image has no frame grid of its own");
        assert!(
            info.duration.0 >= 90_000 * 3600 * 100,
            "{name}: duration {} — an event longer than it would show empty space",
            info.duration.0
        );
        assert_eq!((info.width, info.height), (320, 180), "{name}: wrong size");

        // the frame arrives at ANY tick and it is ONE AND THE SAME
        let q = Quality { width: 320, height: 180 };
        let a = src.read_video_frame(Tick(0), Some(q)).unwrap_or_else(|e| panic!("{name} @0: {e}"));
        for t in [1_i64, 3 * 90_000, 9 * 90_000 + 7, 600 * 90_000] {
            let f = src
                .read_video_frame(Tick(t), Some(q))
                .unwrap_or_else(|e| panic!("{name} @{t}: {e}"));
            assert_eq!(f.rgba, a.rgba, "{name}: at tick {t} a different frame arrived");
        }
        assert!(a.rgba.iter().any(|&b| b != 0), "{name}: frame is empty");

        // audio — silence, not an error
        let burst = src.read_audio_burst(0).expect("a still image must have a silent burst");
        assert!(burst.data.iter().all(|&s| s == 0.0), "{name}: sound in the silence of a still image");
    }

    // PNG WITH ALPHA: the transparency reaches the frame
    let mut png = FfmpegSource::open(&fixture("still_alpha.png")).unwrap();
    let f = png.read_video_frame(Tick(0), Some(Quality { width: 320, height: 180 })).unwrap();
    let alphas: Vec<u8> = f.rgba.chunks_exact(4).map(|p| p[3]).collect();
    assert!(
        alphas.iter().any(|&a| a < 250),
        "PNG alpha dropped: frame is fully opaque (first: {:?})",
        &alphas[..8.min(alphas.len())]
    );
}
