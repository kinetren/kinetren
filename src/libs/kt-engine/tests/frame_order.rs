//! Test 113 (item 14, round 14): frames in the OFFLINE render (the same path as export)
//! go strictly forward, evenly, and without black holes — including the case where the
//! project fps does not match the source fps.
//!
//! The user: “there is stutter in rendered videos always.” One cause
//! was in the decoder (test 112 on a real file); the second is checked here:
//! the mapping “project tick → source frame” and the frame-by-frame output of the export.
//! The synthetic source is switched to its own frame-grid mode — the colour of each frame
//! encodes the source frame NUMBER, so the finished RGBA shows exactly
//! which frame reached the output.

use kt_engine::export::render_raw_video;
use kt_media::MediaSource;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;

const DUR: Tick = Tick(90_000 * 3); // 3 s

fn project(fps: u32, reversed: bool) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
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
        duration: DUR,
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn sources(src_fps: u32) -> Vec<(MediaId, Box<dyn MediaSource>)> {
    vec![(
        MediaId(1),
        Box::new(SyntheticSource::new(DUR, AudioPattern::Silence).with_video_fps(src_fps))
            as Box<dyn MediaSource>,
    )]
}

/// Source frame numbers that reached the output (colour encodes the number).
fn rendered_indices(project_fps: u32, src_fps: u32, reversed: bool) -> Vec<i64> {
    let p = project(project_fps, reversed);
    let q = kt_media::Quality { width: 64, height: 36 };
    let mut out = Vec::new();
    render_raw_video(&p, sources(src_fps), q, |rgba| {
        let idx = rgba[0] as i64 | (rgba[1] as i64) << 8 | (rgba[2] as i64) << 16;
        out.push(idx);
        Ok(())
    })
    .expect("offline render");
    out
}

#[test]
fn t113_export_frames_are_monotonic_and_uniform() {
    // --- grids do NOT match: project 60, source 25
    let idx = rendered_indices(60, 25, false);
    assert_eq!(idx.len(), 180, "3 s at 60 fps = 180 frames, got {}", idx.len());
    assert!(idx.iter().all(|&i| i >= 0), "black frame (index not read)");
    // 1) NEVER backwards
    for w in idx.windows(2) {
        assert!(w[1] >= w[0], "frame went back: {} → {}", w[0], w[1]);
    }
    // 2) no gaps: neighbouring frames are either the same, or the next one
    for w in idx.windows(2) {
        assert!(w[1] - w[0] <= 1, "source frame skipped: {} → {}", w[0], w[1]);
    }
    // 3) evenness: the lengths of runs of identical frames differ by ≤ 1
    let mut runs: Vec<usize> = Vec::new();
    let mut cur = idx[0];
    let mut len = 0usize;
    for &i in &idx {
        if i == cur {
            len += 1;
        } else {
            runs.push(len);
            cur = i;
            len = 1;
        }
    }
    runs.push(len);
    // the edge runs may be cut by the range boundaries — we do not judge them
    let mid = &runs[1..runs.len() - 1];
    let lo = mid.iter().min().copied().unwrap();
    let hi = mid.iter().max().copied().unwrap();
    assert!(hi - lo <= 1, "uneven duplicates: runs from {lo} to {hi} ({runs:?})");

    // --- grids match: strictly 1:1
    let same = rendered_indices(30, 30, false);
    assert_eq!(same.len(), 90);
    for (n, &i) in same.iter().enumerate() {
        assert_eq!(i, n as i64, "with equal grids frame {n} must be 1:1, got {i}");
    }

    // --- reverse: strictly backwards, also without gaps
    let rev = rendered_indices(30, 30, true);
    assert_eq!(rev.len(), 90);
    for w in rev.windows(2) {
        assert!(w[1] <= w[0], "reverse: frame went forward {} → {}", w[0], w[1]);
        assert!(w[0] - w[1] <= 1, "reverse: frame {} → {} skipped", w[0], w[1]);
    }
}

/// Test 115 (item 14, round 14): during SLOW playback (0.25×) the key
/// of the shown frame does not roll back — an integration test, through a live
/// `Player` with a fake audio callback (no sound hardware is needed).
#[test]
fn t115_preview_keys_are_monotonic_at_quarter_rate() {
    use kt_engine::Player;
    use kt_engine::player::preview_key_step;

    let p = project(30, false);
    let (player, mut ring) = Player::new(
        p,
        vec![(
            MediaId(1),
            Box::new(SyntheticSource::new(Tick(90_000 * 30), AudioPattern::Ramp)) as _,
        )],
    );
    player.seek(Tick(0));
    player.set_playback_rate(0.25);
    player.play();

    let grid = player.video_grid();
    let step = grid.frame_step();
    let mut prev: Option<i64> = None;
    let mut rollbacks = 0u64;
    let start = std::time::Instant::now();
    let mut consumed = 0usize;
    // ~1.5 s of real time: the callback at 1024 frames each, between them
    // several “UI frames” reading the display position
    while start.elapsed().as_millis() < 1500 {
        for _ in 0..1024 {
            if ring.pop().is_err() {
                break;
            }
        }
        consumed += 1;
        player.shared.consumed_frames.fetch_add(256, std::sync::atomic::Ordering::AcqRel);
        for _ in 0..4 {
            let t = player.video_playhead();
            let key = grid.align_floor(t).0;
            let (key, _, rolled) = preview_key_step(prev, key, step, 1, false);
            if rolled {
                rollbacks += 1;
            }
            if let Some(pv) = prev {
                assert!(key >= pv, "display key moved back: {pv} → {key}");
            }
            prev = Some(key);
            std::thread::sleep(std::time::Duration::from_millis(4));
        }
    }
    assert!(consumed > 10, "callback did not turn: {consumed}");
    // the ratchet is a safeguard, not a working mode: with healthy clocks it
    // fires a few times, with jittering clocks — constantly
    assert!(rollbacks < 20, "display clock jitters at 0.25×: {rollbacks} rollbacks");
}

/// Test 116 (item 1, round 15): the render does not output BLACK frames at a
/// large resolution.
///
/// Regression: export decoded in batches of 120 frames into a 64 MB cache.
/// At 640×360 a batch weighs 111 MB, at 1080p — 995 MB: the LRU evicted
/// frames BEFORE the composite, and the render came out black with a few surviving
/// frames. At 320×180 the batch fit — which is why last round's live check
/// did not see the defect.
#[test]
fn t116_export_has_no_black_frames_at_large_quality() {
    use kt_engine::export::{EXPORT_VIDEO_CACHE_BYTES, export_chunk_frames};

    // a pure function: a batch must not exceed half the budget —
    // taking into account the NUMBER OF LAYERS (R.31 item 4: a frame per layer of the moment)
    for layers in [1usize, 2, 3, 5] {
        for (w, h) in [(320u32, 180u32), (640, 360), (1280, 720), (1920, 1080)] {
            let n = export_chunk_frames(EXPORT_VIDEO_CACHE_BYTES, w, h, layers);
            assert!(n >= 1, "{w}×{h}×{layers}: the batch degenerated into 0 frames");
            let bytes = n as usize * w as usize * h as usize * 4 * layers;
            assert!(
                bytes * 2 <= EXPORT_VIDEO_CACHE_BYTES || n == 1,
                "{w}×{h}×{layers}: a batch of {n} frames = {} MB does not fit the cache budget",
                bytes / 1_000_000
            );
        }
    }

    // a live render at a resolution where the old chunk certainly did not fit
    // into the cache (640×360 × 120 frames = 111 MB). The sign of an evicted frame is
    // not “blackness” (index 0 is black by itself), but a MISSING LAYER and
    // a gap in the sequence of indices.
    let p = project(30, false);
    let q = kt_media::Quality { width: 640, height: 360 };
    let mut idx: Vec<i64> = Vec::new();
    let stats = kt_engine::export::render_raw_video(&p, sources(25), q, |rgba| {
        idx.push(rgba[0] as i64 | (rgba[1] as i64) << 8 | (rgba[2] as i64) << 16);
        Ok(())
    })
    .expect("render");
    assert_eq!(idx.len(), 90, "3 s at 30 fps");
    assert_eq!(
        stats.missing_video_layers, 0,
        "frames were evicted from the cache BEFORE the composite: missing layers {}",
        stats.missing_video_layers
    );
    for w in idx.windows(2) {
        assert!(w[1] >= w[0], "frame index went back: {} → {}", w[0], w[1]);
        assert!(w[1] - w[0] <= 1, "source frame skipped: {} → {}", w[0], w[1]);
    }
    assert!(*idx.last().unwrap() >= 70, "render did not reach the end: {:?}", idx.last());
}

/// Test 117 (item 2, round 15): on the SECOND loop cycle the video does not freeze.
///
/// Regression: the anti-rollback ratchet of the display key allowed jumping back
/// only on a generation change, and loop-wrap deliberately does not bump it
/// (sound and FX tails continue) — the picture froze at the end of the region
/// while the sound kept playing.
#[test]
fn t117_loop_wrap_unfreezes_video() {
    use kt_engine::player::preview_key_step;
    use kt_engine::transport::Transport;

    // 1) the transport counts the jumps, but does NOT touch the generation
    let tr = Transport::new();
    let (s, e) = (48_000i64, 96_000i64);
    tr.seek_frames(s);
    let gen0 = tr.generation();
    assert_eq!(tr.wraps(), 0);
    tr.advance_frames_wrapped(40_000, Some((s, e)));
    assert_eq!(tr.wraps(), 0, "there is no jump inside the region");
    tr.advance_frames_wrapped(40_000, Some((s, e)));
    assert_eq!(tr.wraps(), 1, "a jump across the end of the region was not counted");
    assert_eq!(tr.generation(), gen0, "wrap must not bump the generation");
    assert!(tr.playhead_frames() >= s && tr.playhead_frames() < e);
    // a reverse across the start of the region is also a jump
    tr.advance_frames_wrapped(-40_000, Some((s, e)));
    assert_eq!(tr.wraps(), 2, "a jump backward was not counted");

    // 2) ratchet: without re-anchoring the key freezes (that was the regression),
    //    with re-anchoring on wrap — it moves to the start of the region
    let step = 3000;
    let (frozen, _, rolled) = preview_key_step(Some(90_000), 48_000, step, 1, false);
    assert_eq!((frozen, rolled), (90_000, true), "without re-anchoring the key must freeze");
    let (jumped, _, rolled) = preview_key_step(Some(90_000), 48_000, step, 1, true);
    assert_eq!((jumped, rolled), (48_000, false), "on wrap the key must jump into the region");

    // 3) LIVE player: two loop cycles — the display key must return to
    //    the start of the region, not freeze at its end (the sound does keep going)
    use kt_engine::Player;
    let proj = project(30, false);
    let (player, mut ring) = Player::new(
        proj,
        vec![(
            MediaId(1),
            Box::new(SyntheticSource::new(DUR, AudioPattern::Ramp).with_video_fps(30)) as _,
        )],
    );
    let r0 = Tick(90_000 / 2); // region 0.5 s … 1.0 s
    let r1 = Tick(90_000);
    player.set_loop_region(Some(kt_time::TickRange::new(r0, r1)));
    player.seek(r0);
    player.play();

    let grid = player.video_grid();
    let step = grid.frame_step();
    let mut prev: Option<i64> = None;
    let mut anchor = (player.transport.generation(), player.transport.wraps());
    let mut keys: Vec<i64> = Vec::new();
    let start = std::time::Instant::now();
    while start.elapsed().as_millis() < 2500 && player.transport.wraps() < 2 {
        for _ in 0..1024 {
            if ring.pop().is_err() {
                break;
            }
        }
        player.shared.consumed_frames.fetch_add(1024, std::sync::atomic::Ordering::AcqRel);
        let now = (player.transport.generation(), player.transport.wraps());
        let reanchor = now != anchor;
        anchor = now;
        let key = grid.align_floor(player.video_playhead()).0;
        let (key, _, _) = preview_key_step(prev, key, step, 1, reanchor);
        prev = Some(key);
        keys.push(key);
        std::thread::sleep(std::time::Duration::from_millis(4));
    }
    assert!(player.transport.wraps() >= 1, "loop did not jump once: {keys:?}");
    // after the jump the key must be AGAIN at the start of the region
    let wrap_at = keys
        .windows(2)
        .position(|w| w[1] < w[0])
        .expect("the display key never returned to the start of the region — the video froze");
    assert!(
        keys[wrap_at + 1] < r0.0 + 6 * step,
        "after the jump the key is not at the start of the region: {}",
        keys[wrap_at + 1]
    );
    // and then it moves forward again. Compare with the MAXIMUM, not the last one:
    // within the 2.5 s window a second wrap-around can also happen, and if it falls on the last sample,
    // `last` is legitimately smaller than `first`
    // (this was caught as a rare flake under load).
    let after: Vec<i64> = keys[wrap_at + 1..].to_vec();
    assert!(
        after.iter().max().unwrap() > after.first().unwrap(),
        "after the jump the video did not move on: {after:?}"
    );
}
