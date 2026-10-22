//! Tests 247 (docs/porting-notes.md): the VIDEO slows down (R.37 item 2).
//!
//! The user's request: “slowed&reverb for video (a separate checkbox by default — with some simple frame generation)”.
//! Stretching time is not expressed by a per-frame filter —
//! it changes the CHOICE of the source frame, that is, a single function `clip_material_pos`.
//! Prefetch, preview and export are all fed from it,
//! so we check precisely their agreement.

use kt_engine::VideoRenderer;
use kt_model::*;
use kt_time::{Tick, TickRange};

const SEC: i64 = 90_000;
const STEP: i64 = SEC / 30; // 3000, 30 fps
/// Quality of the tests; decode scale step (R.46) without native sizes of the sources — 1.
const Q: kt_media::Quality = kt_media::Quality { width: 64, height: 36 };

fn project(fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>, reversed: bool) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
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
        duration: Tick(4 * SEC),
        source_in: Tick(SEC),
        media: MediaId(1),
        gain_db: 0.0,
        reversed,
        fx,
        fx_bypass: vec![],
        fx_auto: auto,
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

/// the source pts that the render requests on each frame of the event.
fn pts_track(p: &Project) -> Vec<i64> {
    let vr = VideoRenderer::new(p);
    (0..4 * 30)
        .map(|i| {
            let t = Tick(i * STEP);
            vr.layers_at(t, Q).first().map(|(_, s, _)| s.0).expect("layer is present")
        })
        .collect()
}

/// Test 247: half speed — each source frame is held for two output frames;
/// double speed — every second one is skipped. The position is computed from the
/// integral of the speed, not “by eye”.
#[test]
fn t247_video_follows_the_rate() {
    // 0.5×: over 4 s of the event 2 s of material is consumed
    let slow = pts_track(&project(vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)], vec![], false));
    assert_eq!(slow[0], SEC, "the first frame is the entry point");
    assert_eq!(slow[119], SEC + 2 * SEC - STEP, "by the end of the event 2 s of material was consumed");
    // each source frame is held for exactly two output frames
    for i in (0..118).step_by(2) {
        assert_eq!(slow[i], slow[i + 1], "frame {i} must hold for two output frames");
        assert_eq!(slow[i + 2] - slow[i], STEP, "and change exactly by one frame");
    }

    // 2×: we skip every second one
    let fast = pts_track(&project(vec![FxInstance::slowed_reverb(2.0, 0.0, 85.0)], vec![], false));
    for i in 0..119 {
        assert_eq!(fast[i + 1] - fast[i], 2 * STEP, "at 2× the source step is twice as large");
    }

    // rate = 1 — the path is BYTE-FOR-BYTE the old one (GOLDEN_VIDEO rests on this)
    let plain = pts_track(&project(vec![], vec![], false));
    let one = pts_track(&project(vec![FxInstance::slowed_reverb(1.0, 0.0, 85.0)], vec![], false));
    assert_eq!(plain, one, "unit speed changes nothing");
}

/// Test 247b: the speed curve — the same integral as for audio. Accelerating
/// 1.0 → 2.0 over an event consumes exactly 1.5 of its length (a parabola, not a line).
#[test]
fn t247_video_follows_the_rate_curve() {
    let dur = 4 * SEC;
    let curve = vec![vec![ParamCurve {
        param: "rate".into(),

        // keys — in ticks of the MATERIAL from source_in (the domain of R.37: the axis is linear)
        keys: AutoKey::linear(vec![AutoKey::at(Tick(SEC), 1.0), AutoKey::at(Tick(SEC + dur), 2.0)]),
        source: Default::default(),
    }]];
    let p = project(vec![FxInstance::slowed_reverb(1.0, 0.0, 85.0)], curve, false);
    let track = pts_track(&p);
    let prof: Vec<(f64, f64)> = vec![(0.0, 1.0), (1.0, 2.0)];
    for i in [0usize, 30, 60, 90, 119] {
        let tau = (i as i64 * STEP) as f64 / dur as f64;
        let want = SEC + kt_fx::warp_material_ticks(1.0, &prof, dur, tau);
        // the render aligns to the frame grid — we compare with frame precision
        assert!(
            (track[i] - (want / STEP) * STEP).abs() <= STEP,
            "frame {i}: pts {} against the integral {want}",
            track[i]
        );
    }
    assert!(
        track[119] - SEC > (dur as f64 * 1.4) as i64,
        "the acceleration must consume ~1.5 of the event length, consumed {}",
        track[119] - SEC
    );
}

/// Test 247c: reverse + stretch — the same frames in reverse order.
/// The composition “WARP, then MIRROR”: a mirror before the warp would drive the speed curve
/// backwards and desynchronize A/V.
#[test]
fn t247_reverse_and_stretch_compose() {
    let fx = vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)];
    let fwd = pts_track(&project(fx.clone(), vec![], false));
    let rev = pts_track(&project(fx, vec![], true));
    // the same material, traversed in the opposite direction
    assert_eq!(rev[0], fwd[119], "the reverse starts with the last frame of the forward one");
    assert_eq!(rev[119], fwd[0], "and ends with the first");
    assert!(rev.windows(2).all(|w| w[0] >= w[1]), "the material runs strictly toward us");
    assert!(rev.iter().all(|&s| (SEC..SEC + 2 * SEC).contains(&s)), "the material inside the window");
    // the phase of repeats may differ by half a frame: the composition is
    // “WARP, then MIRROR” (as in audio), not a mirror of the output axis.
    // At rate = 1 there is no divergence at all — this is what holds t24 and GOLDEN.
    let one = pts_track(&project(vec![FxInstance::slowed_reverb(1.0, 0.0, 85.0)], vec![], true));
    let plain = pts_track(&project(vec![], vec![], true));
    assert_eq!(one, plain, "at unit speed the reverse is byte for byte the same as before");
}

/// Test 247d: prefetch warms EXACTLY what the composite reads. A divergence of
/// these two calculations = a permanent cache miss and a black frame — the same
/// invariant as in audio (kt-fx: “a single source of truth”).
#[test]
fn t247_prefetch_matches_what_render_reads() {
    let p = project(vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)], vec![], false);
    let vr = VideoRenderer::new(&p);
    // PAIRS are compared (frame, decode quality): since R.46 the size is part of
    // the request, and diverging on it is as fatal as on pts (item 318)
    let wanted: std::collections::BTreeSet<(i64, kt_media::Quality)> = vr
        .wanted_frames(TickRange::new(Tick(0), Tick(4 * SEC)), Q)
        .into_iter()
        .map(|(_, s, q)| (s.0, q))
        .collect();
    let read: std::collections::BTreeSet<(i64, kt_media::Quality)> = (0..4 * 30)
        .flat_map(|i| vr.layers_at(Tick(i * STEP), Q))
        .map(|(_, s, q)| (s.0, q))
        .collect();
    assert_eq!(read, wanted, "prefetch and render must ask for the same set of frames");
    // keys strictly increase and have no duplicates — a precondition of read_video_span
    let keys: Vec<i64> = wanted.iter().map(|(k, _)| *k).collect();
    assert!(keys.windows(2).all(|w| w[0] < w[1]), "the keys strictly increase");
    // at 0.5× there are half as many frames as output — the repeats collapsed
    assert_eq!(keys.len(), 60, "over 4 s at 0.5× we need 60 source frames, not 120");
}

/// Test 247e: a morpher layer does not cut a STRETCHED clip regardless of the kind —
/// and in the reference modes too (the veto stood below them, a defect before R.37).
#[test]
fn t247_layer_never_slices_a_stretched_clip() {
    for kind in [ClipKind::Video, ClipKind::Audio] {
        let mut p = project(vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)], vec![], false);
        p.clips[0].kind = kind;
        if kind == ClipKind::Audio {
            p.ensure_tracks(kt_model::LaneKind::Audio, 1);
        }
        p.markers.push(Marker {
            t: Tick(SEC),
            duration: Tick(SEC),
            name: "Insert".into(),
            color: None,
        });
        p.ensure_tracks(kt_model::LaneKind::Morph, 1);
        p.morphers.push(MorpherEvent {
            id: MorphId(1),
            lane: 0,
            start: Tick(SEC),
            duration: Tick(SEC),
            targets: MorphTargets::Marks { names: vec!["Insert".into()] },
            patch: MorphPatch {
                video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
                audio_fx: vec![FxInstance::gain(3.0)],
                ..Default::default()
            },
            timed: true,
            strength: 1.0,
            fade_in: Tick::ZERO,
            fade_out: Tick::ZERO,
            enabled: true,
        });
        p.validate().unwrap();
        assert_eq!(
            p.effective_clips().len(),
            1,
            "{kind:?}: a stretched clip is not cut even by a named reference"
        );
    }
}

/// Test 250 (R.37 C5): the frame mode “blending”. The pre-FX frame of the layer is a
/// linear mix of two adjacent source frames, made by the SAME `dissolve_into` as the crossfade.
/// Blending lives in the COMMON code of the plan,
/// so both executors receive the same frame (invariant 7).
#[test]
fn t250_blend_mode_mixes_neighbouring_frames() {
    use kt_media::MediaSource;
    use kt_media::synthetic::{AudioPattern, SyntheticSource};

    let blend_fx = || {
        vec![FxInstance::SlowedReverb {
            rate: 0.5,
            mix: 0.0,
            reverb_delay_ms: 85.0,
            damp: 0.32,
            decay: 0.58,
            wet_gain: 0.95,
            predelay_ms: 0.0,
            frames: FrameMode::Blend,
            width: 1.0,
            automation: vec![],
        }]
    };
    let p = project(blend_fx(), vec![], false);
    let mut vr = VideoRenderer::new(&p);
    let q = kt_media::Quality { width: 64, height: 36 };
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence);
    // we warm exactly what the plan asks for
    for (media, pts, _q) in vr.wanted_frames(TickRange::new(Tick(0), Tick(4 * SEC)), Q) {
        let f = synth.read_video_frame(pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video_cache_idx(pts),
            std::sync::Arc::new(f),
        );
    }

    // at 0.5× the odd output frames sit exactly midway between the source frames:
    // weight 2048/4096
    let t = Tick(STEP); // the second frame of the event
    let (out, complete) = vr.compose_from_cache(&cache, t, q);
    assert!(complete, "both frames of the pair are warm");
    let a = synth.read_video_frame(Tick(SEC), Some(q)).unwrap().rgba.into_vec();
    let b = synth.read_video_frame(Tick(SEC + STEP), Some(q)).unwrap().rgba.into_vec();
    let want: Vec<u8> = a
        .iter()
        .zip(&b)
        .map(|(&x, &y)| ((x as u32 * 2048 + y as u32 * 2048 + 2048) >> 12) as u8)
        .collect();
    assert_eq!(out, want, "the mix is computed with the same integer kernel as the crossfade");

    // even frames sit ON the grid — weight 0, no pair, the old path
    let (on_grid, _) = vr.compose_from_cache(&cache, Tick(0), q);
    assert_eq!(on_grid, a, "on the grid there is nothing to mix");

    // “nearest” on the same data gives a CLEAN frame, not a mix
    let mut near = VideoRenderer::new(&project(
        vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)],
        vec![],
        false,
    ));
    let (n, _) = near.compose_from_cache(&cache, t, q);
    assert_eq!(n, a, "the “nearest” mode holds the frame");
    assert_ne!(n, out, "and it differs from mixing");
}

/// Test 250b: prefetch warms BOTH frames of the pair. If it does not warm them, the second
/// is never requested, and the layer starves forever.
#[test]
fn t250_blend_prefetches_both_sides() {
    let p = project(
        vec![FxInstance::SlowedReverb {
            rate: 0.5,
            mix: 0.0,
            reverb_delay_ms: 85.0,
            damp: 0.32,
            decay: 0.58,
            wet_gain: 0.95,
            predelay_ms: 0.0,
            frames: FrameMode::Blend,
            width: 1.0,
            automation: vec![],
        }],
        vec![],
        false,
    );
    let vr = VideoRenderer::new(&p);
    let wanted: std::collections::BTreeSet<i64> = vr
        .wanted_frames(TickRange::new(Tick(0), Tick(4 * SEC)), Q)
        .into_iter()
        .map(|(_, s, _)| s.0)
        .collect();
    // the neighbors added a frame on the right: 61 instead of 60
    assert_eq!(wanted.len(), 61, "both sides of the mix warm up, not one");
    let keys: Vec<i64> = wanted.iter().copied().collect();
    assert!(keys.windows(2).all(|w| w[0] < w[1]), "the keys still strictly increase");

    // a blending layer holds TWO frames — the export must account for this,
    // otherwise the chunk would evict the neighbor and the bytes would depend on the cache
    assert_eq!(vr.max_concurrent_layers(), 2, "a mixing layer weighs two frames");
}

// ---------------------------------------------------------------------------
// Tests 272 (R.39): slicing and the automation domain on a stretched clip.
// ---------------------------------------------------------------------------

/// Test 272: a sliced project renders the SAME frames as an unsliced one.
///
/// This is exactly the “jump at the joint” that was discussed: before R.39
/// `clip_slice` computed `source_in` linearly, and the sequence of source
/// pts diverged on EVERY frame after the cut.
#[test]
fn t272_split_renders_the_same_frames() {
    for (name, fx, auto, reversed) in [
        ("0.5x", vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)], vec![], false),
        ("2x", vec![FxInstance::slowed_reverb(2.0, 0.0, 85.0)], vec![], false),
        ("0.5x rev", vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0)], vec![], true),
        (
            "curve",
            vec![FxInstance::slowed_reverb(1.0, 0.0, 85.0)],
            vec![vec![ParamCurve {
                param: "rate".into(),

                keys: AutoKey::linear(vec![
                    AutoKey::at(Tick(SEC), 1.0),
                    AutoKey::at(Tick(SEC + 4 * SEC), 2.0),
                ]),
                source: Default::default(),
            }]],
            false,
        ),
    ] {
        let whole = project(fx.clone(), auto.clone(), reversed);
        let before = pts_track(&whole);
        for at in [2 * SEC, SEC * 17 / 10] {
            let mut m = TimelineModel::new(whole.clone());
            m.apply(Command::Split {
                id: ClipId(1),
                at: Tick(at),
                new_id: ClipId(2),
                new_group: None,
            })
            .expect("the cut goes through");
            let after = pts_track(m.project());
            let bad = before
                .iter()
                .zip(&after)
                .enumerate()
                .filter(|(_, (a, b))| (*a - *b).abs() > STEP)
                .count();
            assert_eq!(
                bad,
                0,
                "{name}, cut at {at}: {bad} frames out of {} diverged — the cut landed on the wrong \
                 material",
                before.len()
            );
        }
    }
}

/// Test 272b: the VIDEO-FX curve is sampled in the same domain as the sound and UI.
///
/// Before R.39 `layer_src` took the warp-dependent frame position, that is, on a
/// slowed clip the picture was animated on its own schedule. The axis is
/// linear by the decision of R.37 (“WITHOUT accounting for slowing-speeding”).
#[test]
fn t272b_video_curves_use_the_linear_axis() {
    // Blur with a radius curve + half speed. The key is exactly at the middle of the
    // event: the value under it must match the value that the UI will show
    // (the same `clip_material_at`).
    let mut p = project(
        vec![FxInstance::slowed_reverb(0.5, 0.0, 85.0), FxInstance::blur(1)],
        vec![
            vec![],
            vec![ParamCurve {
                param: "radius".into(),

                keys: AutoKey::linear(vec![
                    AutoKey::at(Tick(SEC), 1.0),
                    AutoKey::at(Tick(SEC + 2 * SEC), 48.0),
                ]),
                source: Default::default(),
            }],
        ],
        false,
    );
    p.clips[0].fx_bypass = vec![false, false];
    p.validate().unwrap();

    let mid = Tick(2 * SEC); // the middle of the event
    let want_at = kt_model::clip_material_at(
        p.clips[0].source_in,
        p.clips[0].start,
        p.clips[0].duration,
        false,
        STEP,
        mid,
    );
    // the keys cover [SEC, 3·SEC]; the linear axis puts the middle of the event at
    // material 3·SEC — that is, the end of the curve
    assert_eq!(want_at, Tick(SEC + 2 * SEC), "linear axis: the middle of the event = the middle of the material");

    let vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let plan = vr.frame_plan(&cache, mid, kt_media::Quality { width: 64, height: 36 });
    let radius = plan.layers.iter().find_map(|l| match l {
        kt_engine::PlanLayer::One { src, .. } => src.fx.iter().find_map(|f| match f {
            FxInstance::Blur { radius, .. } => Some(*radius),
            _ => None,
        }),
        _ => None,
    });
    assert_eq!(
        radius,
        Some(48),
        "the radius must be taken from the LINEAR axis (the end of the curve), not from the warp position"
    );
}

// ---- R.57: signed speed -------------------------------------------------

/// Test 604a (R.57): the video runs over the material BACKWARD when the speed
/// is negative, and turns around where the curve changes sign.
///
/// The owner's claim, verbatim: “reverse playback in a slowed effect
/// (negative rate)”.
#[test]
fn t604a_video_follows_a_signed_curve() {
    let dur = 4 * SEC;
    // 1 → −1: the position rises to the middle of the event and returns
    let curve = vec![vec![ParamCurve {
        param: "rate".into(),

        keys: AutoKey::linear(vec![
            AutoKey::at(Tick(SEC), 1.0),
            AutoKey::at(Tick(SEC + dur), -1.0),
        ]),
        source: Default::default(),
    }]];
    let track = pts_track(&project(vec![FxInstance::slowed_reverb(1.0, 0.0, 85.0)], curve, false));
    let top = track.iter().enumerate().max_by_key(|(_, v)| **v).unwrap().0;
    assert!(
        (55..65).contains(&top),
        "the apex is not in the middle of the event: frame {top}, track {:?}",
        &track[..5]
    );
    assert!(track[top] > track[0] + SEC / 2, "the material did not advance forward at all");
    // and RETURNED to where it started: ∫rate over the whole event is zero
    assert!(
        (track[119] - track[0]).abs() <= 2 * STEP,
        "the curve did not return to the start: {} against {}",
        track[119],
        track[0]
    );
    // the second half strictly decreases — this is the backward motion
    for i in (top + 2)..118 {
        assert!(track[i] >= track[i + 1], "frame {i}: the material does not go backward");
    }
}

/// Test 604b (R.57): zero speed is a FREEZE FRAME, not “very slow”.
#[test]
fn t604b_zero_rate_is_a_freeze() {
    let track = pts_track(&project(vec![FxInstance::slowed_reverb(0.0, 0.0, 85.0)], vec![], false));
    assert!(track.iter().all(|&t| t == track[0]), "the still frame moved: {:?}", &track[..6]);
    assert_eq!(track[0], SEC, "the hold keeps the frame of the entry point");
    // and the order for such a clip is ONE keyframe for the whole event
    let p = project(vec![FxInstance::slowed_reverb(0.0, 0.0, 85.0)], vec![], false);
    let vr = VideoRenderer::new(&p);
    let keys = vr.wanted_frames(TickRange::new(Tick(0), Tick(4 * SEC)), Q);
    assert_eq!(keys.len(), 1, "the still frame ordered more than one frame: {keys:?}");
}

/// Test 604c (R.57): at the start of the material the frame HOLDS, rather than going negative.
#[test]
fn t604c_edge_holds_at_the_material_start() {
    // source_in = 1 s, speed −1: in a second we would hit the file’s zero point
    let track =
        pts_track(&project(vec![FxInstance::slowed_reverb(-1.0, 0.0, 85.0)], vec![], false));
    assert!(track.iter().all(|&t| t >= 0), "the frame went past the start of the file: {:?}", &track[28..34]);
    assert_eq!(track[0], SEC);
    assert_eq!(track[119], 0, "by the end of the event we are at the start of the file and hold");
    assert!(track[35] == 0 && track[60] == 0, "the edge does not hold");
}

/// Test 604d (R.57): under negative speed the “blend” pair is taken
/// from the SAME side as for positive speed — the fractional part is measured forward.
#[test]
fn t604d_blend_pair_under_negative_rate() {
    let mut fx = FxInstance::slowed_reverb(-0.5, 0.0, 85.0);
    if let FxInstance::SlowedReverb { frames, .. } = &mut fx {
        *frames = FrameMode::Blend;
    }
    let p = project(vec![fx], vec![], false);
    let vr = VideoRenderer::new(&p);
    // at mid-frame the pair must be (a, a+STEP) and the weight exactly one half
    for i in [1i64, 3, 5, 7] {
        let keys = vr.layers_at(Tick(i * STEP), Q);
        assert_eq!(keys.len(), 2, "frame {i}: there is no pair — {keys:?}");
        let (a, b) = (keys[0].1.0, keys[1].1.0);
        assert_eq!(b - a, STEP, "frame {i}: the neighbour was not taken forward: {a} and {b}");
    }
}

/// Test 604e (R.57): order == read under a sign flip (class t247d).
///
/// A window over the two ends would miss the excursion: the curve goes forward
/// and comes back, and the maximum lies INSIDE the window, not at its edge.
#[test]
fn t604e_prefetch_matches_reads_under_a_sign_change() {
    let dur = 4 * SEC;
    let curve = vec![vec![ParamCurve {
        param: "rate".into(),

        keys: AutoKey::linear(vec![
            AutoKey::at(Tick(SEC), 2.0),
            AutoKey::at(Tick(SEC + dur), -2.0),
        ]),
        source: Default::default(),
    }]];
    let p = project(vec![FxInstance::slowed_reverb(1.0, 0.0, 85.0)], curve, false);
    let vr = VideoRenderer::new(&p);
    let wanted: std::collections::BTreeSet<i64> = vr
        .wanted_frames(TickRange::new(Tick(0), Tick(dur)), Q)
        .into_iter()
        .map(|(_, s, _)| s.0)
        .collect();
    let read: std::collections::BTreeSet<i64> = (0..4 * 30)
        .flat_map(|i| vr.layers_at(Tick(i * STEP), Q).into_iter().map(|(_, s, _)| s.0))
        .collect();
    assert!(!read.is_empty());
    assert!(
        read.is_subset(&wanted),
        "something was read that was not ordered: {:?}",
        read.difference(&wanted).collect::<Vec<_>>()
    );
    // SCENE GUARD: the excursion must exist — otherwise the test catches nothing
    let first = vr.layers_at(Tick(0), Q)[0].1.0;
    let last = vr.layers_at(Tick(119 * STEP), Q)[0].1.0;
    let top = *read.iter().max().unwrap();
    assert!(top > first.max(last) + STEP, "the scene degenerated: there is no excursion");
}

/// Test 604g (R.57): “Reverse” on top of negative speed gives motion
/// FORWARD — the mirror is taken around the span, not around the consumed part.
#[test]
fn t604g_reverse_over_negative_rate_goes_forward() {
    let rev = pts_track(&project(vec![FxInstance::slowed_reverb(-0.5, 0.0, 85.0)], vec![], true));
    // without the mirror −0.5 goes backward; with the mirror — forward, from the lower edge
    // of the span to the upper edge
    assert!(rev[0] < rev[119], "the mirror did not reverse the motion: {} → {}", rev[0], rev[119]);
    for i in 0..118 {
        assert!(rev[i] <= rev[i + 1] + STEP, "frame {i}: the motion is not monotonic");
    }
    // span −0.5 over 4 s is 2 s backward: the mirror puts us at −1 s from the
    // entry point and leads back to it
    assert_eq!(rev[0], 0, "the mirror starts from the lower edge of the span");
    // tolerance of two frames: the mirror subtracts the grid step (the first output
    // frame of the mirrored clip is the LAST real one), and the last frame
    // of the event lies one more step earlier than the end
    assert!((rev[119] - SEC).abs() <= 2 * STEP, "the mirror does not reach the entry point: {}", rev[119]);
}
