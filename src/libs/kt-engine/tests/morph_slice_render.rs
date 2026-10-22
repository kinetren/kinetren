//! Tests 237-238 (docs/porting-notes.md): cutting the target by a morpher layer in the RENDER.
//!
//! A morpher layer cuts the target along the boundaries of its own window (R.35). Two things
//! that must remain untouched in doing so:
//!
//! * **the transition does not disappear** — it is addressed by `ClipId`, and for the cut
//!   clip this id carries only piece No. 0; the end of the transition is re-attached to
//!   the piece whose boundary coincided with the joint (`same_clip` + a check of the
//!   boundary in `VideoRenderer::new`/`AudioRenderer::new`);
//! * **the cut is neither audible nor visible** — a patch that does nothing to this kind
//!   of clip must give a BYTE-FOR-BYTE identical result.

use kt_engine::{AudioRenderer, DirectFetcher, VideoRenderer};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const LEN: Tick = Tick(90_000);

fn direct() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 440.0 })),
    );
    f.add_source(MediaId(2), Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp)));
    f
}

fn clip(id: u64, kind: ClipKind, start: Tick, dur: Tick) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind,
        lane: 0,
        start,
        duration: dur,
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn base(kind: ClipKind) -> Project {
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
    p.media.push(MediaRef::new(MediaId(1), "synth:sine:440"));
    p.media.push(MediaRef::new(MediaId(2), "synth:ramp"));
    p.clips.push(clip(10, kind, Tick(0), LEN));
    p.clips.push(clip(11, kind, LEN, LEN));
    p.ensure_tracks(kt_model::LaneKind::Morph, 1);
    p
}

/// A layer at the middle of the FIRST clip. `patch` is set by the caller.
fn layer(patch: MorphPatch) -> MorpherEvent {
    MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick(30_000),
        duration: Tick(30_000),
        targets: MorphTargets::Clips { ids: vec![ClipId(10)] },
        patch,
        timed: true,
        strength: 1.0,
        fade_in: Tick::ZERO,
        fade_out: Tick::ZERO,
        enabled: true,
    }
}

fn crossfade(p: &mut Project) {
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(10)),
        b: Some(ClipId(11)),
        in_offset: Tick(9_000),
        out_offset: Tick(9_000),
        kind: Default::default(),
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
}

fn render_audio(p: &Project, len: Tick) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let frames = len.to_audio_frames() as usize;
    let mut out = vec![0f32; frames * CH];
    let s = r.render_frames(&mut direct(), 0, &mut out);
    assert_eq!(s.underruns, 0, "underrun with a fully available source");
    out
}

/// The preview frame through the same path as t137: warm the cache with exactly the
/// layers the plan itself named, and composite with the CPU reference.
fn render_frame(p: &Project, t: Tick) -> Vec<u8> {
    use kt_media::MediaSource;
    let mut vr = VideoRenderer::new(p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };
    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence);
    for (media, src_pts, q) in vr.layers_at(t, q) {
        let frame = synth.read_video_frame(src_pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video_cache_idx(src_pts),
            std::sync::Arc::new(frame),
        );
    }
    let (out, complete) = vr.compose_from_cache(&cache, t, q);
    assert!(complete, "frame {t:?}: all layers are warm");
    out
}

/// Test 237: the transition on a CUT clip is alive. The layer covers the TAIL of the
/// first clip together with the crossfade window and substitutes the entry point there —
/// so the joint falls to a piece whose id is already synthetic. The reference
/// is the same edit, cut BY HAND (Split + a transition on the right
/// half). On the old code (`c.id == id`) the end of the transition went to
/// piece No. 0, it stuck out as a handle over the whole window, and the frame diverged.
#[test]
fn t237_transition_survives_slicing() {
    let mut sliced = base(ClipKind::Video);
    crossfade(&mut sliced);
    let mut m = layer(MorphPatch { absolute_source_in: Some(Tick(15_000)), ..Default::default() });
    m.start = Tick(60_000);
    m.duration = Tick(30_000); // to the end of the clip: the transition window is covered entirely
    sliced.morphers.push(m);
    sliced.validate().unwrap();
    let eff = sliced.effective_clips();
    assert_eq!(eff.iter().filter(|c| same_clip(c.id, ClipId(10))).count(), 2, "the clip is split");
    assert_ne!(eff[0].id, eff[1].id);

    // reference: the same thing, made with the model's commands
    let mut manual = base(ClipKind::Video);
    crossfade(&mut manual);
    let mut mm = TimelineModel::new(manual);
    mm.apply(Command::Split {
        id: ClipId(10),
        at: Tick(60_000),
        new_id: ClipId(12),
        new_group: None,
    })
    .unwrap();
    let mut manual = mm.project().clone();
    manual.clips.iter_mut().find(|c| c.id == ClipId(12)).unwrap().source_in = Tick(15_000);
    manual.validate().unwrap();
    assert_eq!(
        manual.transitions[0].a,
        Some(ClipId(12)),
        "Split moved the transition to the right half"
    );

    for t in [Tick(81_000), Tick(84_000), Tick(90_000), Tick(96_000)] {
        assert_eq!(render_frame(&sliced, t), render_frame(&manual, t), "frame {t:?}");
    }
    // a safeguard: without the transition the frame in the middle is DIFFERENT — the comparison above
    // really checks the transition, not the coincidence of the sources
    let mut no_trans = manual.clone();
    no_trans.transitions.clear();
    assert_ne!(
        render_frame(&no_trans, Tick(90_000)),
        render_frame(&manual, Tick(90_000)),
        "the transition is visible in the frame"
    );
}

/// Test 237b: the same for audio — the crossfade window is counted in frames, and the joint is
/// found by the same check.
#[test]
fn t237_audio_transition_survives_slicing() {
    let mut plain = base(ClipKind::Audio);
    crossfade(&mut plain);
    plain.validate().unwrap();
    let mut sliced = plain.clone();
    sliced.morphers.push(layer(MorphPatch { media: Some(MediaId(1)), ..Default::default() }));
    sliced.validate().unwrap();

    let a = render_audio(&plain, Tick(180_000));
    let b = render_audio(&sliced, Tick(180_000));
    assert_eq!(a, b, "a crossfade on a split clip sounds byte for byte the same");

    let mut no_trans = plain.clone();
    no_trans.transitions.clear();
    assert_ne!(render_audio(&no_trans, Tick(180_000)), a, "the crossfade is audible");
}

/// Test 238: cutting is not audible. A layer with VIDEO effects only cuts the video,
/// but does not touch the audio under it at all; a layer that nevertheless cuts an audio clip
/// gives the same sound, if the patch itself does not change the sound.
#[test]
fn t238_slicing_is_inaudible() {
    // a video patch does not cut the audio at all
    let mut p = base(ClipKind::Audio);
    let mut m = layer(MorphPatch {
        video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
        ..Default::default()
    });
    m.targets = MorphTargets::Clips { ids: vec![ClipId(10), ClipId(11)] };
    p.morphers.push(m);
    p.validate().unwrap();
    assert_eq!(p.effective_clips(), p.clips, "the video layer does not touch the audio under it");

    // a patch that cuts: substitution of the source with the SAME one — the sound is the same as before
    let plain = base(ClipKind::Audio);
    let mut sliced = plain.clone();
    sliced.morphers.push(layer(MorphPatch { media: Some(MediaId(1)), ..Default::default() }));
    sliced.validate().unwrap();
    assert_eq!(sliced.effective_clips().len(), 4, "the clip is split");
    assert_eq!(
        render_audio(&sliced, Tick(180_000)),
        render_audio(&plain, Tick(180_000)),
        "the cut seam is not audible — byte for byte"
    );
}

/// Test 238b: a clip with its OWN time warp (slowed&reverb) is not cut at all —
/// the linearly computed entry point of the piece would point to the wrong place
/// (finding 2 of R.35). The patch acts on it as a whole.
#[test]
fn t238_warped_clip_is_never_sliced() {
    let mut p = base(ClipKind::Audio);
    p.clips[0].fx = vec![FxInstance::slowed_reverb(0.8, 0.3, 60.0)];
    p.morphers.push(layer(MorphPatch { media: Some(MediaId(2)), ..Default::default() }));
    p.validate().unwrap();
    let eff = p.effective_clips();
    assert_eq!(eff.iter().filter(|c| same_clip(c.id, ClipId(10))).count(), 1, "a warp is not cut");
    assert_eq!(eff[0].media, MediaId(2), "the patch applies to the whole clip");

    // a warp added by the layer ITSELF — the same case
    let mut q = base(ClipKind::Audio);
    q.morphers.push(layer(MorphPatch {
        audio_fx: vec![FxInstance::slowed_reverb(0.8, 0.3, 60.0)],
        ..Default::default()
    }));
    q.validate().unwrap();
    assert_eq!(
        q.effective_clips().iter().filter(|c| same_clip(c.id, ClipId(10))).count(),
        1,
        "a layer that adds a warp does not cut either"
    );
}

/// Test 267 (R.38 item 12, verbatim: “the morpher has no way to apply
/// fade and opacity — the plan is not kept”).
///
/// The layer's strength is a crossfade of the processed picture with the unprocessed one (the verdict
/// of the R.36 reconnaissance: “layer opacity works for free as a strength slider”). Three things that
/// must hold EXACTLY:
///
/// 1. strength 1.0 — the former frame BYTE FOR BYTE (there is no mix slot at all);
/// 2. strength 0.0 — the frame as WITHOUT THE LAYER, but with the event's own effects:
///    the strength rolls back the layer's contribution, not the whole chain;
/// 3. an intermediate strength — exactly the linear mix of these two.
#[test]
fn t267_layer_strength_blends_only_the_layer() {
    let t = Tick(45_000); // the middle of the layer's window
    let own = FxInstance::Mirror { horizontal: true, vertical: false };
    let layer_fx = FxInstance::edges(24);

    // the event with ITS OWN effect; the layer adds its own on top
    let with_own = |strength: f64, layered: bool| {
        let mut p = base(ClipKind::Video);
        p.clips[0].fx = vec![own.clone()];
        p.clips[0].fx_bypass = vec![false];
        p.clips[0].fx_auto = vec![vec![]];
        if layered {
            let mut m =
                layer(MorphPatch { video_fx: vec![layer_fx.clone()], ..Default::default() });
            m.strength = strength;
            p.morphers.push(m);
        }
        p.validate().unwrap();
        p
    };

    let full = render_frame(&with_own(1.0, true), t);
    let none = render_frame(&with_own(0.0, true), t);
    let no_layer = render_frame(&with_own(1.0, false), t);

    assert_ne!(full, no_layer, "at strength 1.0 the layer must change the picture");
    assert_eq!(
        none, no_layer,
        "at strength 0.0 the frame must match the frame WITHOUT THE LAYER: strength rolls back the layer’s \
         contribution, not the event’s own effects (this is what the `from` of the mix slot is for)"
    );

    // the half — an exact linear mix of the same two frames
    let half = render_frame(&with_own(0.5, true), t);
    let k = (0.5f64 * 4096.0).round() as u32;
    let ik = 4096 - k;
    let want: Vec<u8> = none
        .iter()
        .zip(full.iter())
        .map(|(a, b)| (((*a as u32) * ik + (*b as u32) * k + 2048) >> 12) as u8)
        .collect();
    assert_eq!(half, want, "strength 0.5 — exactly the crossfade of “without the layer” and “with the layer”");
}

/// Test 267b: the fades at the block's edges are a STRENGTH CURVE, not a separate
/// mechanism. At the very edge the layer does not act, in the middle it acts
/// fully.
#[test]
fn t267b_layer_fades_are_the_strength_curve() {
    let scene = || {
        let mut p = base(ClipKind::Video);
        let mut m =
            layer(MorphPatch { video_fx: vec![FxInstance::edges(24)], ..Default::default() });
        // block 30_000..60_000; fades of a third on each side
        m.fade_in = Tick(10_000);
        m.fade_out = Tick(10_000);
        p.morphers.push(m);
        p.validate().unwrap();
        p
    };
    let plain = {
        let p = base(ClipKind::Video);
        p.validate().unwrap();
        p
    };
    let s = scene();
    // at the very start of the block the layer does not act yet
    let edge = render_frame(&s, Tick(30_000));
    assert_eq!(edge, render_frame(&plain, Tick(30_000)), "at the edge of the block the strength is zero");
    // in the middle of the plateau — it acts fully
    let mid = render_frame(&s, Tick(45_000));
    assert_ne!(mid, render_frame(&plain, Tick(45_000)), "in the middle the layer acts");
    // and within the fade — somewhere in between them
    let inside = render_frame(&s, Tick(35_000));
    assert_ne!(inside, edge, "inside the fade the layer already acts");
    assert_ne!(inside, mid, "but not yet at full strength");
}

/// Test 268 (R.38 item 12, the second half of the decision “video and audio at once”).
/// The layer's strength attenuates the layer's contribution in the sound AND likewise does not touch
/// the event's own effects.
#[test]
fn t268_layer_strength_blends_audio_too() {
    let scene = |strength: f64, layered: bool| {
        let mut p = base(ClipKind::Audio);
        // the event's OWN level: the layer's strength must preserve it
        p.clips[0].fx = vec![FxInstance::gain(-6.0)];
        p.clips[0].fx_bypass = vec![false];
        p.clips[0].fx_auto = vec![vec![]];
        if layered {
            let mut m =
                layer(MorphPatch { audio_fx: vec![FxInstance::gain(12.0)], ..Default::default() });
            m.strength = strength;
            p.morphers.push(m);
        }
        p.validate().unwrap();
        p
    };
    let len = Tick(60_000);
    let full = render_audio(&scene(1.0, true), len);
    let none = render_audio(&scene(0.0, true), len);
    let no_layer = render_audio(&scene(1.0, false), len);

    let close = |a: &[f32], b: &[f32]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    assert!(!close(&full, &no_layer), "at strength 1.0 the layer must change the sound");
    assert!(
        close(&none, &no_layer),
        "at strength 0.0 the sound must match the sound WITHOUT THE LAYER: strength rolls back the layer’s \
         contribution, while the event’s own level (−6 dB) remains"
    );

    // the half — a linear mix of the same two (within the layer's window, away from the chunk edges)
    let half = render_audio(&scene(0.5, true), len);
    let probe = 35_000i64 as usize; // within the layer's window
    let i = (Tick(probe as i64).to_audio_frames() as usize) * CH;
    let want = none[i] * 0.5 + full[i] * 0.5;
    assert!(
        (half[i] - want).abs() < 1e-4,
        "strength 0.5 — a linear mix: {} against {}",
        half[i],
        want
    );
}
