//! Test 11: golden export.
//! - raw streams (RGBA + f32) are deterministic from run to run;
//! - the hashes match the committed references of THIS platform (a table per os/arch,
//!   windows-build: libm/sinc coefficients may give other bits on another platform, then
//!   its row is revised consciously; t859 keeps a row for every delivered platform);
//! - “preview == export”: a frame from the export stream is byte for byte equal to the
//!   frame of the preview composite at the same Quality (a single path — invariant No. 7).

use kt_engine::{VideoRenderer, render_raw_audio, render_raw_video};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::Tick;

/// FNV-1a: no dependencies, stable.
fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn f32s_as_bytes(s: &[f32]) -> Vec<u8> {
    s.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Test 11 project: concatenation + crossfade + slowed with automation.
fn golden_project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 128,
        height: 72,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    for (id, path) in
        [(1, "synth:ramp"), (2, "synth:dc:0.5"), (3, "synth:impulse"), (4, "synth:ramp")]
    {
        p.media.push(MediaRef::new(MediaId(id), path));
    }
    let sec = 90_000i64;
    p.clips = vec![
        // concatenation end-to-end (the ramp continues the source)
        ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick(0),
            duration: Tick(sec),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        },
        // R.24: the former overlap [1.5 s, 2 s) is converted to an end-to-end join C = 1.75 s +
        // a transition (the window is the same) — the render bytes must match the old
        // GOLDEN (the cornerstone check of the conversion, t136)
        ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(2),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick(sec),
            duration: Tick(sec * 3 / 4),
            source_in: Tick(sec),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        },
        // crossfade: DC through the transition with the second ramp clip
        ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(3),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick(sec * 7 / 4),
            duration: Tick(sec * 3 / 4),
            source_in: Tick(sec / 4),
            media: MediaId(2),
            gain_db: -3.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        },
        // slowed with automation + reverb on a separate track
        ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(4),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 1,
            start: Tick(0),
            duration: Tick(2 * sec),
            source_in: Tick(0),
            media: MediaId(3),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            // slowed automation — through the common curves (R.33): keys in the
            // MATERIAL ticks. The 0.3/0.7 fractions of the former domain at source_in = 0 and
            // a length of 2 s give exactly these ticks, so the sound is byte for byte
            // as before and GOLDEN_AUDIO was not regenerated.
            fx_auto: vec![vec![
                ParamCurve {
                    param: "rate".into(),

                    keys: AutoKey::linear(vec![
                        AutoKey::at(Tick(2 * sec * 3 / 10), 1.0),
                        AutoKey::at(Tick(2 * sec * 7 / 10), 0.5),
                    ]),
                    source: Default::default(),
                },
                ParamCurve {
                    param: "mix".into(),

                    keys: AutoKey::linear(vec![
                        AutoKey::at(Tick(2 * sec * 3 / 10), 0.4),
                        AutoKey::at(Tick(2 * sec * 7 / 10), 0.1),
                    ]),
                    source: Default::default(),
                },
            ]],
            fx: vec![FxInstance::SlowedReverb {
                rate: 0.7,
                mix: 0.4,
                reverb_delay_ms: 85.0,
                damp: 0.32,
                decay: 0.58,
                wet_gain: 0.95,
                predelay_ms: 0.0,
                width: 1.0,
                frames: Default::default(),
                automation: vec![],
            }],
            opacity: 1.0,
            fx_off: false,
        },
        // video: two layers with overlap (z-composite)
        ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(5),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(0),
            duration: Tick(2 * sec),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        },
        ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(6),
            link_group: None,
            kind: ClipKind::Video,
            lane: 1,
            start: Tick(sec / 2),
            duration: Tick(sec),
            source_in: Tick(0),
            media: MediaId(4),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        },
    ];
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(2)),
        b: Some(ClipId(3)),
        in_offset: Tick(sec / 4),
        out_offset: Tick(sec / 4),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.normalize(); // tracks fitted to the content (R.62)
    p.validate().unwrap();
    p
}

fn sources(ids: &[(u64, AudioPattern)]) -> Vec<(MediaId, Box<dyn MediaSource>)> {
    ids.iter()
        .map(|&(id, pat)| {
            (
                MediaId(id),
                Box::new(SyntheticSource::new(Tick(90_000 * 60), pat)) as Box<dyn MediaSource>,
            )
        })
        .collect()
}

fn all_sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    sources(&[
        (1, AudioPattern::Ramp),
        (2, AudioPattern::Dc { value: 0.5 }),
        (3, AudioPattern::Impulse),
        (4, AudioPattern::Ramp),
    ])
}

fn run_export_hashes() -> (u64, u64, u64, u64) {
    let p = golden_project();
    let mut audio_hash = 0xcbf29ce484222325u64;
    let mut audio_frames = 0u64;
    let astats = render_raw_audio(&p, all_sources(), |chunk| {
        let b = f32s_as_bytes(chunk);
        audio_hash ^= fnv(&b);
        audio_hash = audio_hash.wrapping_mul(0x100000001b3);
        audio_frames += (chunk.len() / 2) as u64;
        Ok(())
    })
    .unwrap();
    let q = Quality { width: 128, height: 72 };
    let mut video_hash = 0xcbf29ce484222325u64;
    let vstats = render_raw_video(&p, all_sources(), q, |rgba| {
        video_hash ^= fnv(rgba);
        video_hash = video_hash.wrapping_mul(0x100000001b3);
        Ok(())
    })
    .unwrap();
    assert_eq!(astats.underruns, 0);
    assert_eq!(vstats.missing_video_layers, 0);
    (audio_hash, video_hash, astats.audio_frames, vstats.video_frames)
}

#[test]
fn t11_streams_are_deterministic_across_runs() {
    let a = run_export_hashes();
    let b = run_export_hashes();
    assert_eq!(a, b, "export must be reproducible byte for byte");
    // 2.5 seconds of the timeline (the end of the crossfade clip): 120000 audio frames,
    // 75 video frames
    assert_eq!(a.2, 120_000);
    assert_eq!(a.3, 75);
}

/// The platforms people get a build for (capability `distribution`).
pub const DELIVERED: [(&str, &str); 2] = [("linux", "x86_64"), ("windows", "x86_64")];

/// The committed references PER PLATFORM (windows-build, export spec): (os, arch, audio,
/// video, where they were taken). libm differs between platforms and may move the last bit;
/// a platform's row changes only by a deliberate decision, like the values themselves.
const GOLDEN: &[(&str, &str, u64, u64, &str)] = &[
    ("linux", "x86_64", GOLDEN_AUDIO, GOLDEN_VIDEO, "linux/x86_64 workshop, 2026-09-07 (R.37)"),
    (
        "windows",
        "x86_64",
        // the audio mix differs in the last bits: the C runtime's transcendentals (sin/exp
        // from msvcrt) are not glibc's
        0x0a9c_8e22_f9f1_e8c2,
        GOLDEN_VIDEO,
        "audio measured on windows-latest (CI logs 108933450618, 2026-10-25); video assumed \
         equal to linux until the next windows-latest run confirms it",
    ),
];

#[test]
fn t11_streams_match_committed_golden() {
    let (audio, video, _, _) = run_export_hashes();
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    let Some(&(_, _, ga, gv, prov)) = GOLDEN.iter().find(|g| g.0 == os && g.1 == arch) else {
        eprintln!("skipped: no committed reference for {os}/{arch}; measured audio 0x{audio:016x}, video 0x{video:016x}");
        return;
    };
    // A deliberate change of the render algorithms = a deliberate update of the references
    // (with listening!). Both streams in one assertion: a new platform's row is corrected
    // from ONE run's log.
    assert_eq!(
        (audio, video),
        (ga, gv),
        "the streams diverged from the {os}/{arch} reference ({prov}): measured audio \
         0x{audio:016x}, video 0x{video:016x}"
    );
}

/// t859 (windows-build, export): every delivered platform has its row in the reference
/// tables (export here, the title in title_golden.rs), each with where it was taken; a
/// platform without one would be compared against nothing and pass silently.
#[test]
fn t859_every_delivered_platform_has_references() {
    // whitespace-insensitive: rustfmt may lay the rows out over lines
    let title: String = include_str!("title_golden.rs").chars().filter(|c| !c.is_whitespace()).collect();
    for (os, arch) in DELIVERED {
        let row = GOLDEN.iter().find(|g| g.0 == os && g.1 == arch);
        assert!(row.is_some_and(|r| !r.4.is_empty()), "no export reference row for {os}/{arch}");
        assert!(
            title.contains(&format!("(\"{os}\",\"{arch}\",GOLDEN_TITLE_VIDEO")),
            "no title reference row for {os}/{arch} in title_golden.rs"
        );
    }
}

// REGENERATED 2026-09-07 (the second and last time): the automation axis was
// moved from the SOURCE domain to the EVENT domain by the user's explicit decision
// (item 1 of R.37, test 31) — the clip's sound with automation changed deliberately.
// The former value was 0x469c89aa2adc3b14.
//
// The shift was computed by hand BEFORE the run and matched: clip 4 has a rate curve
// 1.0 → 0.5 on the 0.3/0.7 fractions, so S(1) went from ≈0.7728 (the ODE solution over the
// material) to exactly 0.75 (the time trapezoid: 0.3·1 + 0.4·0.75 + 0.3·0.5)
// — about 2200 source frames, plus the mix curve was re-timed.
//
// The video reference did NOT change and must not: the project's video clips have `fx: vec![]`,
// and the warp branch is enabled only with slowed in the chain.
const GOLDEN_AUDIO: u64 = 0x5a83eef245ab43f0;
const GOLDEN_VIDEO: u64 = 0x95b790352d74f586;

#[test]
#[ignore = "reference generator: cargo test -- --ignored t11_print"]
fn t11_print_golden() {
    let (a, v, af, vf) = run_export_hashes();
    println!("GOLDEN_AUDIO = 0x{a:016x}; GOLDEN_VIDEO = 0x{v:016x}; frames a={af} v={vf}");
}

#[test]
fn t11_preview_equals_export_frame() {
    // Frame No. 17 of the export stream == the preview composite of the same moment
    // at the same Quality (a single render path).
    let p = golden_project();
    let q = Quality { width: 128, height: 72 };
    let target = 17usize;
    let mut captured: Option<Vec<u8>> = None;
    let mut n = 0usize;
    render_raw_video(&p, all_sources(), q, |rgba| {
        if n == target {
            captured = Some(rgba.to_vec());
        }
        n += 1;
        Ok(())
    })
    .unwrap();
    let export_frame = captured.unwrap();

    // “preview”: an independent cache, direct decode of the layers, the same composite
    let mut video = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let t = Tick(target as i64 * 3000);
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        all_sources().into_iter().collect();
    for (media, src_pts, q) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    let (preview_frame, complete) = video.compose_from_cache(&cache, t, q);
    assert!(complete);
    assert_eq!(preview_frame, export_frame, "preview and export must match byte for byte");
}

/// Test 67 (item 2, round 7): preview == export with a VIDEO-FX chain — both a cold
/// compose (the chain is applied) and a warm one (the frame from the post-FX cache) are
/// byte for byte equal to the frame of the export stream. Post-FX memoization does not
/// break invariant No. 7.
#[test]
fn t67_preview_equals_export_with_fx_cold_and_warm() {
    let mut p = golden_project();
    // the chain from the user's complaint about the video clip
    for c in p.clips.iter_mut().filter(|c| c.kind == ClipKind::Video) {
        c.fx = vec![
            FxInstance::Mirror { horizontal: true, vertical: false },
            FxInstance::glow(1.0),
            FxInstance::vhs(2, 0.4),
        ];
    }
    p.validate().unwrap();
    let q = Quality { width: 128, height: 72 };
    let target = 17usize;
    let mut captured: Option<Vec<u8>> = None;
    let mut n = 0usize;
    render_raw_video(&p, all_sources(), q, |rgba| {
        if n == target {
            captured = Some(rgba.to_vec());
        }
        n += 1;
        Ok(())
    })
    .unwrap();
    let export_frame = captured.unwrap();

    let mut video = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let t = Tick(target as i64 * 3000);
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        all_sources().into_iter().collect();
    for (media, src_pts, q) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    let (cold, complete) = video.compose_from_cache(&cache, t, q);
    assert!(complete);
    assert_eq!(cold, export_frame, "cold FX composite == export");
    let (warm, _) = video.compose_from_cache(&cache, t, q);
    assert_eq!(warm, export_frame, "warm (post-FX cache) == export");
}

// ---- R.52: generator in the render ------------------------------------------

type Sources = Vec<(MediaId, Box<dyn MediaSource>)>;

/// Project t479: a solid generator over a synthetic background.
fn generator_project() -> (Project, Sources) {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 128,
        height: 72,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:dc:0.25"));
    // R.59, stage 2: the media path holds the KIND, the values are on the event
    p.media.push(MediaRef::new(MediaId(2), "gen:solid"));
    let clip = |id: u64, lane: u32, start: i64, dur: i64, media: u64| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    // the background — the whole scene; the solid enters at 30_000 (frame No. 10) as the upper layer
    let mut g2 = clip(2, 0, 30_000, 60_000, 2);
    g2.r#gen = Some(kt_model::GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 });
    p.clips = vec![clip(1, 1, 0, 90_000, 1), g2];
    p.normalize(); // tracks fitted to the content (R.62)
    p.validate().unwrap();
    // the generator's source is opened by export ITSELF (R.59, stage 2): its
    // address is derived from the event, and the caller has nowhere to get it
    let sources: Sources =
        vec![(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000), AudioPattern::Silence)))];
    (p, sources)
}

/// Test 479 (R.52, sibling of t11): the generator in the render path. Export
/// is deterministic between runs; a frame in the generator's zone is exactly
/// the given colour; preview == export byte for byte (invariant 7).
/// GOLDEN_AUDIO/GOLDEN_VIDEO are NOT touched by this scene: they have their own
/// project without generators.
#[test]
fn t479_generator_preview_equals_export() {
    let q = Quality { width: 128, height: 72 };
    let target = 17usize; // a frame inside the solid's zone (t >= 30_000)
    let run = || {
        let (p, sources) = generator_project();
        let mut captured: Option<Vec<u8>> = None;
        let mut all = Vec::new();
        let mut n = 0usize;
        render_raw_video(&p, sources, q, |rgba| {
            if n == target {
                captured = Some(rgba.to_vec());
            }
            all.extend_from_slice(rgba);
            n += 1;
            Ok(())
        })
        .unwrap();
        (captured.unwrap(), fnv(&all))
    };
    let (frame_a, hash_a) = run();
    let (_, hash_b) = run();
    assert_eq!(hash_a, hash_b, "export with a generator must be deterministic");

    // the upper layer is solid: every pixel of the frame is exactly the given colour
    assert!(
        frame_a.chunks_exact(4).all(|px| px == [255, 136, 0, 255]),
        "a generator frame must be exactly the set colour"
    );

    // preview by the same path as t11: an independent cache + layers_at
    let (p, sources) = generator_project();
    let mut video = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let t = Tick(target as i64 * 3000);
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources.into_iter().collect();
    // preview raises the generator sources with the same list as export
    for (addr, state) in kt_engine::gen_sources(&p) {
        srcs.insert(addr, kt_media::factory::open_gen(&state));
    }
    for (media, src_pts, q) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    let (preview_frame, complete) = video.compose_from_cache(&cache, t, q);
    assert!(complete, "a composite with a generator must be complete");
    assert_eq!(preview_frame, frame_a, "preview and export must match byte for byte");
}

/// Test 715 (R.62): the golden files do not move (t11/t67 above stay green without
/// regeneration — at matching proportions the leading node is identical), and a
/// vertical source in 16:9 gives a frame with MARGINS: a measurement of the fill's geometry
/// on the finished export frame, as in F-MED-004.
#[test]
fn t715_vertical_source_gets_fields_in_export() {
    let mut p = golden_project();
    // one video clip (id 5) on media 1; no audio and no second layer needed
    p.clips.retain(|c| c.id == ClipId(5));
    p.transitions.clear();
    p.morphers.clear();
    let q = Quality { width: 128, height: 72 };
    let src: Vec<(MediaId, Box<dyn MediaSource>)> = vec![(
        MediaId(1),
        Box::new(
            SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence).with_video_size(36, 64),
        ),
    )];
    let mut captured: Option<Vec<u8>> = None;
    let mut n = 0usize;
    render_raw_video(&p, src, q, |rgba| {
        if n == 3 {
            captured = Some(rgba.to_vec());
        }
        n += 1;
        Ok(())
    })
    .unwrap();
    let f = captured.expect("frame 3");
    let px =
        |x: u32, y: u32| &f[((y * q.width + x) * 4) as usize..((y * q.width + x) * 4 + 3) as usize];
    let dark = |x: u32, y: u32| px(x, y).iter().all(|&v| v < 8);
    // margins left and right — background, the middle — the source's fill
    assert!(dark(0, 36) && dark(127, 36), "fields are background");
    assert!(!dark(64, 36), "centre is the source");
    let lit = (0..q.width).filter(|&x| !dark(x, 36)).count();
    // the “fit” frame of the vertical 36×64 in 128×72: 72·36/64 = 40.5 px
    assert!((39..=42).contains(&lit), "fitted width ≈ 40.5 px, got {lit}");
}

/// Test 728b (R.62): export at a resolution ≠ the project's conforms by the SAME
/// proportions. A vertical source in a 1920×1080 project: an export frame
/// 1280×720 is similar to the 1920×1080 frame — the fill's width fraction and its centre
/// coincide within the resampling tolerance; the rule and the node are not recomputed.
#[test]
fn t728b_export_in_another_resolution_is_similar_to_preview() {
    let mut p = golden_project();
    p.clips.retain(|c| c.id == ClipId(5));
    p.clips[0].duration = Tick(3000 * 6);
    p.transitions.clear();
    p.morphers.clear();
    p.settings.width = 1920;
    p.settings.height = 1080;
    let src = || -> Vec<(MediaId, Box<dyn MediaSource>)> {
        vec![(
            MediaId(1),
            Box::new(
                SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence)
                    .with_video_size(36, 64),
            ),
        )]
    };
    let profile = |q: Quality| -> (f64, f64) {
        let mut captured: Option<Vec<u8>> = None;
        let mut n = 0usize;
        render_raw_video(&p, src(), q, |rgba| {
            if n == 3 {
                captured = Some(rgba.to_vec());
            }
            n += 1;
            Ok(())
        })
        .unwrap();
        let f = captured.expect("frame 3");
        let y = q.height / 2;
        let lit: Vec<u32> = (0..q.width)
            .filter(|&x| {
                let i = ((y * q.width + x) * 4) as usize;
                f[i..i + 3].iter().any(|&v| v >= 8)
            })
            .collect();
        assert!(!lit.is_empty(), "{}×{}: empty frame", q.width, q.height);
        let w = q.width as f64;
        (lit.len() as f64 / w, (lit[0] + lit[lit.len() - 1]) as f64 / 2.0 / w)
    };
    let (frac_a, mid_a) = profile(Quality { width: 1920, height: 1080 });
    let (frac_b, mid_b) = profile(Quality { width: 1280, height: 720 });
    // “fit” 36×64 in 16:9: fill width = 36/64 · 9/16 = 0.3164 of the canvas
    let want = 36.0 / 64.0 * 9.0 / 16.0;
    assert!((frac_a - want).abs() < 2.0 / 1080.0, "1080p: share {frac_a} ≠ {want}");
    assert!((frac_b - want).abs() < 2.0 / 720.0, "720p: share {frac_b} ≠ {want}");
    assert!((frac_a - frac_b).abs() < 2.0 / 720.0, "shares diverged: {frac_a} / {frac_b}");
    assert!((mid_a - 0.5).abs() < 1.0 / 720.0 && (mid_b - 0.5).abs() < 1.0 / 720.0);
}
