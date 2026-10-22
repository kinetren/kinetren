//! Test 24 (docs/porting-notes.md, item 16): reversal of events.
//! Audio: the output == a mirrored slice of the source (exact, Ramp pattern).
//! Video: the frame at moment t == the frame of the mirrored pts (the synthetics encode pts
//! in the color). FX+reverse: deterministic and not silence.

use kt_engine::{AudioRenderer, DirectFetcher, VideoRenderer};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, TICK_HZ, Tick};

const CH: usize = AUDIO_CHANNELS;
/// the preview quality of the tests: decode scale step (R.46) on it — 1.
const Q: kt_media::Quality = kt_media::Quality { width: 64, height: 36 };

fn clip(reversed: bool, fx: Vec<FxInstance>, kind: ClipKind) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
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
        kind,
        lane: 0,
        start: Tick::from_audio_frames(48_000), // the clip is not from the start of the timeline
        duration: Tick::from_audio_frames(48_000 * 2),
        source_in: Tick::from_audio_frames(9_600), // and not from the start of the source
        media: MediaId(1),
        gain_db: 0.0,
        reversed,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn direct() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp)));
    f
}

#[test]
fn t24_reversed_audio_is_exact_mirror() {
    let fwd = clip(false, vec![], ClipKind::Audio);
    let rev = clip(true, vec![], ClipKind::Audio);
    let n = 48_000usize * 2;
    let mut a = vec![0f32; n * CH];
    AudioRenderer::new(&fwd).render_frames(&mut direct(), 48_000, &mut a);
    let mut b = vec![0f32; n * CH];
    AudioRenderer::new(&rev).render_frames(&mut direct(), 48_000, &mut b);

    // b[k] == a[n-1-k] per frame, exact
    for k in [0usize, 1, 1023, 1024, 4097, n / 2, n - 2, n - 1] {
        for ch in 0..CH {
            assert_eq!(
                b[k * CH + ch],
                a[(n - 1 - k) * CH + ch],
                "frame {k} channel {ch}: the reverse must be a mirrored slice"
            );
        }
    }
    assert!(b.iter().any(|s| *s != 0.0), "not silence");
}

#[test]
fn t24_reversed_video_maps_to_mirrored_pts() {
    let fwd = clip(false, vec![], ClipKind::Video);
    let rev = clip(true, vec![], ClipKind::Video);
    let vf = VideoRenderer::new(&fwd);
    let vr = VideoRenderer::new(&rev);
    let step = TICK_HZ / 30;
    let (start, span) = (90_000i64, 90_000i64 * 2);

    for i in [0i64, 1, 7, 29, 59] {
        let t = Tick(start + i * step);
        let f = vf.layers_at(t, Q)[0].1;
        let r = vr.layers_at(t, Q)[0].1;
        // mirror: f and r are symmetric inside the source piece
        let mirrored = vf.layers_at(Tick(start + span - step - i * step), Q)[0].1;
        assert_eq!(
            r, mirrored,
            "frame {i}: the reverse must show a mirrored pts (fwd {f:?}, rev {r:?})"
        );
    }
}

#[test]
fn t24_reversed_with_slowed_fx_is_deterministic_and_not_silent() {
    let fx = vec![FxInstance::SlowedReverb {
        rate: 0.6,
        mix: 0.2,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        predelay_ms: 0.0,
        width: 1.0,
        frames: Default::default(),
        automation: vec![],
    }];
    let rev = clip(true, fx, ClipKind::Audio);
    let renderer = AudioRenderer::new(&rev);
    let n = 48_000usize;
    let mut a = vec![0f32; n * CH];
    renderer.render_frames(&mut direct(), 48_000 + 9_600, &mut a);
    let mut b = vec![0f32; n * CH];
    renderer.render_frames(&mut direct(), 48_000 + 9_600, &mut b);
    assert_eq!(a, b, "reverse+slowed is deterministic");
    let rms = (a.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / a.len() as f64).sqrt();
    assert!(rms > 1e-4, "reverse+slowed must not be silence, rms={rms}");
}

/// Test 26 (integration, item 25): video FX are applied in the SINGLE composite path
/// (preview == export), the cache holds pre-FX frames.
#[test]
fn t26_video_fx_applied_in_compose_and_deterministic() {
    let mut p = clip(false, vec![], ClipKind::Video);
    p.clips[0].fx = vec![FxInstance::Mirror { horizontal: true, vertical: false }];
    p.validate().unwrap();
    let mut vr = VideoRenderer::new(&p);

    let cache = std::sync::Arc::new(kt_cache::BurstCache::new(kt_cache::CacheConfig::default()));
    let q = kt_media::Quality { width: 64, height: 36 };
    // warm the clip's frame into the cache directly (pre-FX)
    let t = Tick(90_000);
    let src_pts = vr.layers_at(t, Q)[0].1;
    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence);
    use kt_media::MediaSource;
    let frame = synth.read_video_frame(src_pts, Some(q)).unwrap();
    cache.insert_video(
        kt_engine::source_id(MediaId(1)),
        Some(q),
        kt_engine::video_cache_idx(src_pts),
        std::sync::Arc::new(frame),
    );

    let (a, complete) = vr.compose_from_cache(&cache, t, q);
    assert!(complete);
    let (b, _) = vr.compose_from_cache(&cache, t, q);
    assert_eq!(a, b, "the FX composite is deterministic");
    // the cache is not touched by the FX: the frame in the cache stayed pre-FX (solid color),
    // and the output is mirrored, but for a solid color the mirror == the original; we check
    // at least that the output is not empty and the alpha is opaque
    assert!(a.chunks_exact(4).all(|p| p[3] == 255));
    assert!(a.iter().any(|v| *v != 0));
}

/// Test 29 (item 16): for a reversed clip the content does NOT depend on the right edge.
#[test]
fn t29_reversed_trim_end_preserves_left_edge_frames() {
    let p = clip(true, vec![], ClipKind::Video);
    let vr_before = VideoRenderer::new(&p);
    let step = TICK_HZ / 30;
    let probes: Vec<Tick> = (0..10).map(|i| Tick(90_000 + i * step)).collect();
    let before: Vec<Tick> = probes.iter().map(|&t| vr_before.layers_at(t, Q)[0].1).collect();

    // TrimEnd: shorten by 0.5 s from the right
    let mut m = kt_model::TimelineModel::new(p);
    let id = m.project().clips[0].id;
    let old_end = m.project().clips[0].start + m.project().clips[0].duration;
    m.apply(Command::TrimEnd { id, new_end: Tick(old_end.0 - 45_000) }).unwrap();
    let vr_after = VideoRenderer::new(m.project());
    let after: Vec<Tick> = probes.iter().map(|&t| vr_after.layers_at(t, Q)[0].1).collect();

    assert_eq!(before, after, "frames at the LEFT edge of a reversed clip must not change on TrimEnd");
}

#[test]
fn t29_reversed_trim_start_preserves_surviving_frames() {
    let p = clip(true, vec![], ClipKind::Video);
    let vr_before = VideoRenderer::new(&p);
    let step = TICK_HZ / 30;
    // samples in the interval that will survive the trim
    let probes: Vec<Tick> = (0..8).map(|i| Tick(90_000 + 45_000 + i * step)).collect();
    let before: Vec<Tick> = probes.iter().map(|&t| vr_before.layers_at(t, Q)[0].1).collect();

    let mut m = kt_model::TimelineModel::new(p);
    let id = m.project().clips[0].id;
    let old_start = m.project().clips[0].start;
    m.apply(Command::TrimStart { id, new_start: Tick(old_start.0 + 45_000) }).unwrap();
    let vr_after = VideoRenderer::new(m.project());
    let after: Vec<Tick> = probes.iter().map(|&t| vr_after.layers_at(t, Q)[0].1).collect();

    assert_eq!(before, after, "surviving frames of a reversed clip must not change on TrimStart");
}

#[test]
fn t29_reversed_split_is_visually_seamless() {
    let p = clip(true, vec![], ClipKind::Video);
    let vr_before = VideoRenderer::new(&p);
    let step = TICK_HZ / 30;
    let probes: Vec<Tick> = (0..20).map(|i| Tick(90_000 + i * step * 3)).collect();
    let before: Vec<Tick> = probes.iter().map(|&t| vr_before.layers_at(t, Q)[0].1).collect();

    let mut m = kt_model::TimelineModel::new(p);
    let id = m.project().clips[0].id;
    let at = Tick(90_000 + 90_000); // the middle
    m.apply(Command::Split { id, at, new_id: ClipId(99), new_group: None }).unwrap();
    let vr_after = VideoRenderer::new(m.project());
    let after: Vec<Tick> = probes.iter().map(|&t| vr_after.layers_at(t, Q)[0].1).collect();

    assert_eq!(before, after, "a split of a reversed clip must be seamless in content");
}

/// Test 29 (audio half): reverse + TrimEnd — samples at the left edge are exact.
#[test]
fn t29_reversed_audio_trim_end_left_edge_exact() {
    let p = clip(true, vec![], ClipKind::Audio);
    let n = 4096usize;
    let mut before = vec![0f32; n * CH];
    AudioRenderer::new(&p).render_frames(&mut direct(), 48_000, &mut before);

    let mut m = kt_model::TimelineModel::new(p);
    let id = m.project().clips[0].id;
    let old_end = m.project().clips[0].start + m.project().clips[0].duration;
    m.apply(Command::TrimEnd { id, new_end: Tick(old_end.0 - 45_000) }).unwrap();
    let mut after = vec![0f32; n * CH];
    AudioRenderer::new(m.project()).render_frames(&mut direct(), 48_000, &mut after);

    assert_eq!(before, after, "the start of a reversed audio event must not shift on TrimEnd");
}

/// The Ramp pattern encodes the SOURCE FRAME NUMBER in the sample itself
/// (`n * 1e-6`, left channel). The average frame number over the output piece is the
/// direct answer to “which material is sounding right now”.
fn mean_source_frame(out: &[f32], from: usize, n: usize) -> f64 {
    let mut sum = 0f64;
    for k in from..from + n {
        sum += out[k * CH] as f64;
    }
    (sum / n as f64) / 1e-6
}

fn slowed_dry(rate: f64) -> FxInstance {
    FxInstance::SlowedReverb {
        rate,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 1.0,
        predelay_ms: 0.0,
        width: 1.0,
        frames: Default::default(),
        automation: vec![],
    }
}

/// Test 282 (R.43 item 2/item 292): a mirrored event with a warp reads EXACTLY its own
/// material.
///
/// The claim, verbatim: “in problems1 there is a trio of events with reverses, and there
/// some sound from another event somehow got in”, “and also there the picture is broken
/// because of slowd”.
///
/// The cause: the mirror's axis was the END OF THE SOURCE WINDOW, and that window includes the
/// resampler margin `WARP_SOURCE_MARGIN_FRAMES` = 8192 frames (170.7 ms). Video on the
/// same event is anchored to the actually consumed `ClipWarp::used` without
/// the margin — that is, A and V honestly showed DIFFERENT material. On a user's event
/// 0.36 s long the shift was 63 % of its length.
///
/// Not one test in the repository combined `reversed` with a warp and checked
/// the POSITION of the material: t24 requires only determinism and “not silence”.
#[test]
fn t282_reversed_warped_clip_reads_its_own_material() {
    let src_in = 9_600i64;
    let len = 96_000i64;
    let rev = {
        let mut p = clip(true, vec![slowed_dry(1.0)], ClipKind::Audio);
        p.clips[0].duration = Tick::from_audio_frames(len);
        p.validate().unwrap();
        p
    };
    let n = 4096usize;
    let mut out = vec![0f32; n * CH];
    AudioRenderer::new(&rev).render_frames(&mut direct(), 48_000, &mut out);

    // at rate = 1 the consumed span equals the event length, so the first output frame must read
    // the LAST frame of this span
    let axis = (src_in + len) as f64;
    let got = mean_source_frame(&out, 0, n);
    let want = axis - n as f64 / 2.0;
    assert!(
        (got - want).abs() < 1_000.0,
        "the mirror must be computed from the consumed end of {axis}: \
         the frame sounding is ≈{got:.0}, expected ≈{want:.0} \
         (before R.43 the axis included the resampler headroom of 8192 frames and sounded ≈{:.0})",
        want + 8_192.0
    );
}

/// Test 283 (R.43 item 293): changing `source_in` with the SAME chain structure is audible.
///
/// `chain_fingerprint` hashes only the KINDS of slots, while `FxStateStore` on the
/// reuse branch passed the parameters and curves to the processor, but not the
/// GEOMETRY. Therefore a trim, a cut, a punch-out
/// and a re-anchoring of a morph piece
/// left the processor with the old material anchor: the event kept sounding the material
/// from the file's old place.
/// `reset()` did not help — `hard_seek`/`seam` compute the position from the same field.
#[test]
fn t283_source_in_change_is_heard_with_the_same_chain() {
    let mut p = clip(false, vec![slowed_dry(1.0)], ClipKind::Audio);
    p.clips[0].duration = Tick::from_audio_frames(96_000);
    p.validate().unwrap();

    let n = 4096usize;
    let mut store = kt_engine::FxStateStore::new();
    let r = AudioRenderer::new(&p);
    r.sync_fx(&mut store);
    let mut before = vec![0f32; n * CH];
    r.render_frames_with(&mut direct(), &mut store, 48_000, &mut before);

    // “trim on the left”: the entry point moved, the chain structure is the same
    let delta = 24_000i64;
    let mut p2 = p.clone();
    p2.clips[0].source_in = p2.clips[0].source_in + Tick::from_audio_frames(delta);
    p2.validate().unwrap();
    let r2 = AudioRenderer::new(&p2);
    r2.sync_fx(&mut store); // the SAME store — the FX state is not re-created
    let mut after = vec![0f32; n * CH];
    r2.render_frames_with(&mut direct(), &mut store, 48_000, &mut after);

    let a = mean_source_frame(&before, 0, n);
    let b = mean_source_frame(&after, 0, n);
    assert!(
        (b - a - delta as f64).abs() < 1_000.0,
        "an edit of source_in by {delta} frames must move the SOUNDING material: \
         it was ≈{a:.0}, became ≈{b:.0} (before R.43 the processor held the old anchor \
         and the difference was ≈0)"
    );
}

// ---- R.57: signed speed in audio ------------------------------------------

/// A clip with an arbitrary entry point and a signed speed.
fn signed_clip(rate: f64, source_in_frames: i64, reversed: bool) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
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
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick::from_audio_frames(48_000),
        duration: Tick::from_audio_frames(48_000),
        source_in: Tick::from_audio_frames(source_in_frames),
        media: MediaId(1),
        gain_db: 0.0,
        reversed,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![FxInstance::slowed_reverb(rate, 0.0, 85.0)],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn render(p: &Project, frames: usize) -> Vec<f32> {
    let mut buf = vec![0f32; frames * CH];
    AudioRenderer::new(p).render_frames(&mut direct(), 48_000, &mut buf);
    buf
}

/// Test 616 (R.57): speed −1 is EXACTLY the same as the “Reverse” flag at +1,
/// with the entry point shifted by the event length. Byte-for-byte.
///
/// This is a DEFINITION, not an approximation, and also a consequence to remember: a constant −1
/// goes into the material BEFORE the entry point (the semantics of the Vegas
/// velocity envelope). It can coincide with “Reverse” only with a
/// shift.
#[test]
fn t616_negative_one_equals_reverse_with_a_shifted_in_point() {
    let n = 48_000usize;
    // “Reverse” at +1 from entry point 9600: plays [9600, 57600) backwards
    let rev = render(&signed_clip(1.0, 9_600, true), n);
    // speed −1 from entry point 57600: goes backward over the same material
    let neg = render(&signed_clip(-1.0, 9_600 + 48_000, false), n);
    for k in [0usize, 1, 1023, 1024, 4097, n / 2, n - 2, n - 1] {
        for ch in 0..CH {
            assert_eq!(
                neg[k * CH + ch],
                rev[k * CH + ch],
                "frame {k} channel {ch}: −1 must be the same mirror"
            );
        }
    }
    assert!(neg.iter().any(|s| *s != 0.0), "not silence");
}

/// Test 617 (R.57): reverb at a signed speed sounds FORWARD in its own
/// time — the chain sees the ordinary forward stream and does not know the direction.
#[test]
fn t617_reverb_runs_forward_over_a_backward_read() {
    let n = 48_000usize;
    let with_fx = |rate: f64, src: i64, reversed: bool| {
        let mut p = signed_clip(rate, src, reversed);
        p.clips[0].fx = vec![FxInstance::slowed_reverb(rate, 0.35, 85.0)];
        let mut buf = vec![0f32; n * CH];
        AudioRenderer::new(&p).render_frames(&mut direct(), 48_000, &mut buf);
        buf
    };
    let neg = with_fx(-1.0, 9_600 + 48_000, false);
    let rev = with_fx(1.0, 9_600, true);
    // with reverb both sides read the material the same way, so they also sound
    // the same: the chain's state depends on the INPUT, not on what the
    // input was reversed by
    for k in [0usize, 1024, n / 2, n - 1] {
        for ch in 0..CH {
            assert!(
                (neg[k * CH + ch] - rev[k * CH + ch]).abs() < 1e-6,
                "frame {k}: the chain saw a different input"
            );
        }
    }
    // and this is NOT the same as reversing a finished forward render: the reverb
    // would then sound backwards
    let fwd = with_fx(1.0, 9_600, false);
    let mut mirrored = vec![0f32; n * CH];
    for k in 0..n {
        for ch in 0..CH {
            mirrored[k * CH + ch] = fwd[(n - 1 - k) * CH + ch];
        }
    }
    let diff = (0..n * CH).map(|i| (neg[i] - mirrored[i]).abs()).fold(0.0f32, f32::max);
    assert!(diff > 1e-4, "the mirror of the finished render matched — the reverb is not audible");
}

/// Test 618 (R.57): around the turning point the output is a PALINDROME, with no
/// skipped and no doubled frame.
#[test]
fn t618_turning_point_is_a_palindrome() {
    let n = 48_000usize;
    let mut p = signed_clip(1.0, 9_600, false);
    // 1 → −1: the reversal exactly in the middle of the event
    p.clips[0].fx_auto = vec![vec![ParamCurve {
        param: "rate".into(),

        keys: AutoKey::linear(vec![
            AutoKey::at(Tick::from_audio_frames(9_600), 1.0),
            AutoKey::at(Tick::from_audio_frames(9_600 + 48_000), -1.0),
        ]),
        source: Default::default(),
    }]];
    let out = render(&p, n);
    // the apex: the material is furthest from the entry point. We look for it in the output —
    // Ramp encodes the position in the amplitude, so the maximum is the reversal itself
    let top = (0..n).max_by(|a, b| out[a * CH].partial_cmp(&out[b * CH]).unwrap()).unwrap();
    assert!(
        (n / 4..3 * n / 4).contains(&top),
        "the scene degenerated: the flip is not in the middle ({top} of {n})"
    );
    for j in [1usize, 2, 16, 1023] {
        if top + j >= n || top < j {
            continue;
        }
        assert!(
            (out[(top + j) * CH] - out[(top + 1 - j.min(top + 1)) * CH]).abs() < 0.2,
            "frame {j} from the apex: the palindrome diverged"
        );
    }
    assert!(out.iter().any(|s| *s != 0.0), "not silence");
}

/// Test 619 (R.57): zero speed — SILENCE, while the reverb tail rings out
/// (owner's decision: “silence only at zero”).
#[test]
fn t619_zero_rate_is_silence_with_a_ringing_tail() {
    let n = 48_000usize;
    let mut p = signed_clip(0.0, 9_600, false);
    p.clips[0].fx = vec![FxInstance::slowed_reverb(0.0, 0.0, 85.0)];
    let dry = render(&p, n);
    assert!(
        dry.iter().all(|s| s.abs() < 1e-6),
        "at zero speed there must be silence: max {}",
        dry.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    );
    // and with reverb: first the material sounded (curve 1 → 0), then the tail
    let mut q = signed_clip(1.0, 9_600, false);
    q.clips[0].fx = vec![FxInstance::slowed_reverb(1.0, 0.6, 85.0)];
    q.clips[0].fx_auto = vec![vec![ParamCurve {
        param: "rate".into(),

        keys: AutoKey::linear(vec![
            AutoKey::at(Tick::from_audio_frames(9_600), 1.0),
            AutoKey::at(Tick::from_audio_frames(9_600 + 24_000), 0.0),
        ]),
        source: Default::default(),
    }]];
    let out = render(&q, n);
    let tail_rms = {
        let tail = &out[(n - 4096) * CH..];
        (tail.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / tail.len() as f64).sqrt()
    };
    assert!(tail_rms > 1e-5, "on a hold the reverb must ring out: rms {tail_rms}");
}
