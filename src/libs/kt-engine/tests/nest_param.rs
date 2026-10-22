//! Tests 674-679, 686 and 688 (docs/porting-notes.md): overrides of the
//! public parameters reach the PICTURE and the SOUND (R.59, stage 3).
//!
//! The engine knows nothing about publication: it is handed ready clips from
//! `Project::resolve_reel`. Exactly one thing changes — the EDITION of the reel has become a
//! function of the pair (reel, set of instance overrides), and everything that
//! rests on the edition (memo keys, the sub-renderer tree, the decode scale step,
//! the generator's source address) must hold this.

use kt_engine::{AudioRenderer, DirectFetcher, VideoRenderer};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const FPS: u32 = 30;
const FRAME: i64 = 90_000 / FPS as i64;
const PART: Tick = Tick(FRAME * 30);
const Q: Quality = Quality { width: 96, height: 54 };

const SEQ: MediaId = MediaId(100);
const TITLE: ClipId = ClipId(1);
const PLATE: ClipId = ClipId(2);
const INST_A: ClipId = ClipId(10);
const INST_B: ClipId = ClipId(11);
const HEADLINE: PubId = PubId(1);
const ZOOM: PubId = PubId(2);
const LOGO: PubId = PubId(3);
const LEVEL: PubId = PubId(4);

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: FPS,
        width: 96,
        height: 54,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    }
}

fn title(text: &str) -> GeneratorKind {
    let mut k = GeneratorKind::default_text();
    if let GeneratorKind::Text { text: t, size, .. } = &mut k {
        *t = text.into();
        *size = 0.4;
    }
    k
}

fn clip(id: u64, kind: ClipKind, lane: u32, start: Tick, dur: Tick, media: u64) -> ClipEvent {
    ClipEvent {
        id: ClipId(id),
        link_group: None,
        kind,
        lane,
        start,
        duration: dur,
        source_in: Tick::ZERO,
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: Vec::new(),
        opacity: 1.0,
        fx_off: false,
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
    }
}

fn solid(r: u8, g: u8, b: u8) -> GeneratorKind {
    let mut k = GeneratorKind::default_for_tag("solid").expect("the “fill” kind is in the registry");
    k.set_value("color", &ParamValue::Color([r, g, b, 255]));
    k
}

fn base() -> Project {
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), gen_kind_path(&title(""))));
    p.media.push(MediaRef::new(MediaId(2), "synth:2"));
    // “logo”: a fill of its own colour. Synthetic media is no good for the substitution —
    // its frame is a function of the TICK, and two different synthetic media at one
    // moment give one colour: the test would be green on nothing.
    p.media.push(MediaRef::new(MediaId(3), gen_kind_path(&solid(220, 20, 20))));
    p
}

fn file_sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    vec![(
        MediaId(2),
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 440.0 }))
            as Box<dyn MediaSource>,
    )]
}

/// All sources of the DOCUMENT: the files and generators at the addresses that
/// `gen_sources` names. There must be no second enumeration.
fn sources(p: &Project) -> std::collections::HashMap<MediaId, Box<dyn MediaSource>> {
    let mut out: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        file_sources().into_iter().collect();
    for (addr, st) in kt_engine::gen_sources(p) {
        out.insert(addr, kt_media::factory::open_gen(&st));
    }
    out
}

/// The composite of the moment `t`, warmed EXACTLY by what `layers_at` ordered.
/// Returns (bytes, completeness): completeness false means that the order and the read
/// diverged — and that is exactly the permanent cache miss.
fn compose(p: &Project, t: Tick) -> (Vec<u8>, bool) {
    let mut video = VideoRenderer::new(p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs = sources(p);
    for (addr, pts, dq) in video.layers_at(t, Q) {
        let f = srcs
            .get_mut(&addr)
            .unwrap_or_else(|| panic!("address {addr:?} was requested, but there is no source under it"))
            .read_video_frame(pts, Some(dq))
            .unwrap();
        cache.insert_video(
            kt_engine::source_id(addr),
            Some(dq),
            kt_engine::video::video_cache_idx(pts),
            f.into(),
        );
    }
    video.compose_from_cache(&cache, t, Q)
}

fn render_audio(p: &Project, frames: usize) -> Vec<f32> {
    let mut f = DirectFetcher::new(None);
    for (id, s) in file_sources() {
        f.add_source(id, s);
    }
    for (addr, st) in kt_engine::gen_sources(p) {
        f.add_source(addr, kt_media::factory::open_gen(&st));
    }
    let r = AudioRenderer::new(p);
    let mut out = vec![0f32; frames * CH];
    r.render_frames(&mut f, 0, &mut out);
    out
}

/// Template: a title over a backdrop, two placements end-to-end.
fn template() -> Project {
    let mut p = base();
    let mut s = Sequence::new(SEQ, "Issue");
    let mut t = clip(TITLE.0, ClipKind::Video, 0, Tick::ZERO, PART, 1);
    t.r#gen = Some(title("FIRST"));
    s.clips.push(t);
    s.clips.push(clip(PLATE.0, ClipKind::Video, 1, Tick::ZERO, PART, 2));
    s.ensure_tracks(kt_model::LaneKind::Video, 2);
    s.published = vec![
        PublishedParam {
            id: HEADLINE,
            name: "Heading".into(),
            target: ParamAddr::new(TITLE, ParamOwner::Generator, "text"),
            ty: ParamType::Text { multiline: false },
            range: PubRange::default(),
        },
        PublishedParam {
            id: LOGO,
            name: "Backing".into(),
            target: ParamAddr::new(PLATE, ParamOwner::ClipAttr, "media"),
            ty: ParamType::Media,
            range: PubRange::default(),
        },
    ];
    p.sequences.push(s);
    let mut a = clip(INST_A.0, ClipKind::Video, 0, Tick::ZERO, PART, SEQ.0);
    a.source_in = Tick::ZERO;
    p.clips.push(a);
    let mut b = clip(INST_B.0, ClipKind::Video, 0, PART, PART, SEQ.0);
    b.source_in = Tick::ZERO;
    p.clips.push(b);
    p.validate().expect("a template is legal");
    p
}

fn set_override(p: &mut Project, id: ClipId, ov: Override) {
    let c = p.clips.iter_mut().find(|c| c.id == id).expect("the instance is in place");
    c.overrides.retain(|x| x.pub_id != ov.pub_id);
    c.overrides.push(ov);
    p.validate().expect("a project with an override is legal");
}

// ---- 675. two instances, different frames ----------------------------------------

/// Test 675 (R.59, stage 3): two instances of one reel with a different
/// title give DIFFERENT frames.
///
/// This is exactly the owner's setup as a whole: “ten episodes of one
/// intro”. Before R.59 the text was part of the media address, i.e. shared across all
/// places; with R.59 it is on the event, and “an event inside a reel” stopped
/// uniquely determining the values — they are set by the INSTANCE.
#[test]
fn t675_two_instances_with_different_headlines_render_differently() {
    let mut p = template();
    set_override(&mut p, INST_B, Override::value(HEADLINE, ParamValue::Text("SECOND".into())));

    let mid = Tick(FRAME * 10);
    let (a, ca) = compose(&p, mid);
    let (b, cb) = compose(&p, Tick(PART.0 + mid.0));
    assert!(ca && cb, "the composite is incomplete — the order and the read diverged");
    assert_ne!(
        a, b,
        "two instances played ONE heading: the instance’s values did not reach \
         the frame. This is exactly what the owner called “people will not climb into the composition \
         every time”"
    );
    assert!(a.iter().any(|&x| x != 0), "the frame is black — the test was verified on nothing");

    // ...and this is not “any two frames differ”: without an override they are equal
    let plain = template();
    let (a0, _) = compose(&plain, mid);
    let (b0, _) = compose(&plain, Tick(PART.0 + mid.0));
    assert_eq!(a0, b0, "without overrides the instances must be indistinguishable");
    assert_eq!(a, a0, "the override of the second instance changed the FIRST");
}

// ---- 676. zero cost -----------------------------------------------------------

/// Test 676 (R.59, stage 3): without overrides — ONE sub-renderer and
/// the former bytes.
///
/// The owner's requirement: “if the project is simple with no tricks — it must not lag
/// at all.” The tree key has become a pair (reel, edition), and this is
/// a guard that the pair with an empty set is the very same entry.
#[test]
fn t676_without_overrides_there_is_one_subrenderer_and_the_old_bytes() {
    let plain = template();
    let v = VideoRenderer::new(&plain);
    assert_eq!(
        v.nest_count(),
        1,
        "two placements of one reel without overrides created TWO sub-renderers — \
         exactly the gain for which the tree was built (t442/t443)"
    );

    // a reel without a single publication — the reference “as it was before the round”
    let mut bare = template();
    bare.sequence_mut(SEQ).unwrap().published.clear();
    assert_eq!(
        kt_engine::content_hash(&bare, SEQ),
        kt_engine::instance_content(&bare, SEQ, &[]),
        "an empty set must give EXACTLY content_hash: otherwise all memo keys and the sidecar previews \
         would shift for projects that do not use publishing"
    );
    for i in 0..20 {
        let t = Tick(i * FRAME * 3);
        assert_eq!(
            compose(&bare, t).0,
            compose(&plain, t).0,
            "declaring a handle changed the frame at {i}: publishing must be free"
        );
    }

    // ...and as soon as a set appears there are two entries
    let mut two = template();
    set_override(&mut two, INST_B, Override::value(HEADLINE, ParamValue::Text("SECOND".into())));
    assert_eq!(
        VideoRenderer::new(&two).nest_count(),
        2,
        "an instance with an override must get ITS OWN sub-renderer: otherwise \
         two releases share one sub-composite and one heading"
    );
}

// ---- 674. the override curve ---------------------------------------------

/// Test 674 (R.59, stage 3): the override curve lives in the material time of the
/// INSTANCE (which is also the reel's time) and lands on the same frame of the target's
/// material — under the instance's trim and reverse.
#[test]
fn t674_an_override_curve_lands_on_the_same_material_frame() {
    let mut p = base();
    let mut s = Sequence::new(SEQ, "Reel");
    // the target is SHIFTED both along the reel's timeline and in its material: at zeros the
    // domain mapping would degenerate into the identity and check nothing
    let mut plate = clip(PLATE.0, ClipKind::Video, 0, Tick(FRAME * 5), PART, 2);
    plate.source_in = Tick(FRAME * 40);
    plate.fx = vec![FxInstance::geometry_zoom(1.0)];
    s.clips.push(plate);
    s.published = vec![PublishedParam {
        id: ZOOM,
        name: "Push-in".into(),
        target: ParamAddr::new(PLATE, ParamOwner::FxSlot { slot: 0 }, "zoom"),
        ty: ParamType::Num,
        range: PubRange::default(),
    }];
    p.sequences.push(s);

    let mut a = clip(INST_A.0, ClipKind::Video, 0, Tick::ZERO, PART, SEQ.0);
    a.source_in = Tick(FRAME * 5);
    p.clips.push(a);
    let mut b = clip(INST_B.0, ClipKind::Video, 0, PART, PART, SEQ.0);
    b.source_in = Tick(FRAME * 5);
    b.reversed = true;
    p.clips.push(b);

    // keys — in REEL ticks
    let key = |t: i64, v: f64| AutoKey::at(Tick(t), v);
    let ov = Override {
        pub_id: ZOOM,
        value: ParamValue::Num(1.0),
        curves: vec![ParamCurve {
            param: "zoom".into(),

            keys: AutoKey::linear(vec![key(FRAME * 5, 1.0), key(FRAME * 25, 3.0)]),
            source: CurveSource::Keys,
        }],
    };
    for id in [INST_A, INST_B] {
        p.clips.iter_mut().find(|c| c.id == id).unwrap().overrides = vec![ov.clone()];
    }
    p.validate().expect("a project with an override curve is legal");

    // domain mapping: reel tick τ → target material source_in + (τ − start)
    let want: Vec<i64> = vec![FRAME * 40, FRAME * 60];
    for id in [INST_A, INST_B] {
        let inst = p.clips.iter().find(|c| c.id == id).unwrap();
        let r = p.resolve_reel(Some(SEQ), &inst.overrides);
        let target = r.clips.iter().find(|c| same_clip(c.id, PLATE)).unwrap();
        let got: Vec<i64> = target.fx_auto[0]
            .iter()
            .find(|c| c.param == "zoom")
            .expect("the override curve did not reach the target")
            .keys
            .iter()
            .map(|k| k.t.0)
            .collect();
        assert_eq!(
            got, want,
            "the override keys of instance {id:?} landed on the wrong frame of the material: \
             the curve lives in REEL TIME, and the instance’s trim and reverse \
             do not move it"
        );
    }

    // and this is visible in the frame: the same reel tick of a direct and of a mirrored
    // instance gives ONE frame
    let reel_tick = FRAME * 20;
    let at_a = Tick(reel_tick - FRAME * 5);
    // the mirrored one plays the reel against the direction: the last frame shows source_in
    let at_b = Tick(PART.0 + (PART.0 - FRAME - (reel_tick - FRAME * 5)));
    let (fa, ca) = compose(&p, at_a);
    let (fb, cb) = compose(&p, at_b);
    assert!(ca && cb, "the composite is incomplete");
    assert_eq!(
        fa, fb,
        "one reel tick gave DIFFERENT frames for the direct and the mirrored instance — \
         so the override curve is attached not to the reel but to the screen"
    );
    // and the curve really works: the start and the end differ
    let (f0, _) = compose(&p, Tick(0));
    assert_ne!(f0, fa, "the override curve does not move the bytes — the test is about nothing");
}

// ---- 677. logo substitution ----------------------------------------------------

/// Test 677 (R.59, stage 3): substituting the MEDIA of an instance — a “logo”.
/// The order goes deeper along the NEW address, and the read goes along the same one.
#[test]
fn t677_a_media_override_swaps_the_logo_and_order_equals_read() {
    let mut p = template();
    set_override(&mut p, INST_B, Override::value(LOGO, ParamValue::Media(MediaId(3))));

    // FIRST — the substitution itself in the content. Without this line the test was
    // green with `c.media = *m` switched off: the generator values fitted to the new media
    // (`fit_gen_to_media`) themselves steered the event to the generator's address, the order
    // changed, the frame changed — and “the override works”
    // turned out to be a coincidence. A red test must be red for the SAME reason
    // for which the green one is green.
    {
        let b = p.clips.iter().find(|c| c.id == INST_B).unwrap();
        let r = p.resolve_reel(Some(SEQ), &b.overrides);
        let plate = r.clips.iter().find(|c| same_clip(c.id, PLATE)).expect("the backing is in place");
        assert_eq!(
            plate.media,
            MediaId(3),
            "the override did not rewrite the `media` of the target — the logo stayed the same"
        );
        let a = p.clips.iter().find(|c| c.id == INST_A).unwrap();
        let r0 = p.resolve_reel(Some(SEQ), &a.overrides);
        let plate0 = r0.clips.iter().find(|c| same_clip(c.id, PLATE)).unwrap();
        assert_eq!(plate0.media, MediaId(2), "the substitution in the second instance touched the first");
    }

    let v = VideoRenderer::new(&p);
    let mid = Tick(FRAME * 10);
    let ord_a: Vec<MediaId> = v.layers_at(mid, Q).into_iter().map(|(m, _, _)| m).collect();
    let ord_b: Vec<MediaId> =
        v.layers_at(Tick(PART.0 + mid.0), Q).into_iter().map(|(m, _, _)| m).collect();
    assert!(ord_a.contains(&MediaId(2)), "the first instance must order the reel’s BACKING");
    assert!(
        !ord_b.contains(&MediaId(2)),
        "the media substitution did not reach the ORDER: prefetch would warm something other than what the composite reads"
    );
    assert_ne!(ord_a, ord_b, "the orders of two instances matched — there was no substitution");

    let (a, ca) = compose(&p, mid);
    let (b, cb) = compose(&p, Tick(PART.0 + mid.0));
    assert!(ca && cb, "the composite is incomplete — the order and the read diverged");
    assert_ne!(a, b, "the logo substitution did not change the frame");
}

// ---- 678. sound ----------------------------------------------------------------

/// Test 678 (R.59, stage 3): the override is audible.
#[test]
fn t678_an_override_reaches_the_sound() {
    let mut p = base();
    let mut s = Sequence::new(SEQ, "Voice line");
    s.clips.push(clip(PLATE.0, ClipKind::Audio, 0, Tick::ZERO, PART, 2));
    s.published = vec![PublishedParam {
        id: LEVEL,
        name: "Voice line volume".into(),
        target: ParamAddr::new(PLATE, ParamOwner::ClipAttr, "gain_db"),
        ty: ParamType::Num,
        range: PubRange::default(),
    }];
    p.sequences.push(s);
    let mut a = clip(INST_A.0, ClipKind::Audio, 0, Tick::ZERO, PART, SEQ.0);
    a.source_in = Tick::ZERO;
    p.clips.push(a);
    p.validate().expect("an audio template is legal");

    let frames = PART.to_audio_frames() as usize;
    let before = render_audio(&p, frames);
    assert!(before.iter().any(|x| x.abs() > 1e-6), "the fixture is silent");
    assert_eq!(AudioRenderer::new(&p).nest_count(), 1, "one instance — one sub-mix");

    let mut q = p.clone();
    set_override(&mut q, INST_A, Override::value(LEVEL, ParamValue::Num(-12.0)));
    let after = render_audio(&q, frames);
    assert_ne!(before, after, "the volume override inside a reel is not audible");
    let peak = |v: &[f32]| v.iter().fold(0f32, |m, x| m.max(x.abs()));
    assert!(
        peak(&after) < peak(&before) * 0.5,
        "−12 dB must be audible as a drop in level: was {}, became {}",
        peak(&before),
        peak(&after)
    );
}

// ---- 679. published zoom and the reel's step ---------------------------------

/// Test 679 (R.59, stage 3): the reel's scale is computed OVER THE TREE OF
/// INSTANCES and remains ONE per reel (t450).
///
/// Before the round, `nest_scales` walked the reels flatly, with TEMPLATE values:
/// an instance whose published zoom was turned outward would get a soft picture —
/// the chain input would be taken at Quality, although it zooms eightfold.
/// The scale must nevertheless remain common: different scales would give the
/// inner sheet two cache keys, two coverage bars,
/// and two orders (invariant 4).
#[test]
fn t679_a_published_zoom_raises_the_reel_stage_and_keeps_it_single() {
    // the inner reel — the one whose scale we measure
    let mut p = base();
    let mut inner = Sequence::new(MediaId(101), "Frame");
    inner.clips.push(clip(PLATE.0, ClipKind::Video, 0, Tick::ZERO, PART, 2));
    p.sequences.push(inner);

    // the outer: an instance of the inner one with a ZOOM chain, the zoom declared outward
    let mut outer = Sequence::new(SEQ, "Issue");
    let mut mid = clip(20, ClipKind::Video, 0, Tick::ZERO, PART, 101);
    mid.source_in = Tick::ZERO;
    mid.fx = vec![FxInstance::geometry_zoom(1.0)];
    outer.clips.push(mid);
    outer.published = vec![PublishedParam {
        id: ZOOM,
        name: "Push-in".into(),
        target: ParamAddr::new(ClipId(20), ParamOwner::FxSlot { slot: 0 }, "zoom"),
        ty: ParamType::Num,
        range: PubRange::default(),
    }];
    p.sequences.push(outer);

    let mut a = clip(INST_A.0, ClipKind::Video, 0, Tick::ZERO, PART, SEQ.0);
    a.source_in = Tick::ZERO;
    p.clips.push(a);
    let mut b = clip(INST_B.0, ClipKind::Video, 0, PART, PART, SEQ.0);
    b.source_in = Tick::ZERO;
    p.clips.push(b);
    p.validate().expect("a project with a published zoom is legal");

    // the quality at which the DEEPEST sheet is ordered
    let leaf_q = |p: &Project, t: Tick| -> Quality {
        let ord = VideoRenderer::new(p).layers_at(t, Q);
        assert_eq!(ord.len(), 1, "inside the reel there is exactly one leaf");
        ord[0].2
    };
    let mid_t = Tick(FRAME * 10);
    let flat = leaf_q(&p, mid_t);

    let mut zoomed = p.clone();
    set_override(&mut zoomed, INST_B, Override::value(ZOOM, ParamValue::Num(8.0)));
    let qa = leaf_q(&zoomed, mid_t);
    let qb = leaf_q(&zoomed, Tick(PART.0 + mid_t.0));
    assert!(
        qb.width > flat.width,
        "the published push-in did not raise the reel’s step: {flat:?} → {qb:?}. \
         `nest_scales` must walk the TREE of instances with their \
         overrides, not iterate over reels flatly with template \
         values"
    );
    assert_eq!(
        qa, qb,
        "the step diverged between instances: it is a property of the REEL (t450), otherwise the inner \
         leaf would have two cache keys, two coverage bars and two orders"
    );
    // order == read on both
    assert!(
        compose(&zoomed, mid_t).1 && compose(&zoomed, Tick(PART.0 + mid_t.0)).1,
        "the composite is incomplete — the order and the read diverged"
    );
}

// ---- 686. a template inside a template ----------------------------------------

/// Test 686 (R.59, stage 3): `Nested` gives a template inside a template for free.
///
/// The outer reel declares outward the handle of the INNER instance; its override
/// patches that instance's set, and from there it is resolved by itself — by the same
/// code one floor down.
#[test]
fn t686_a_template_inside_a_template_resolves_recursively() {
    let mut p = base();
    // the inner reel with a title
    let mut inner = Sequence::new(MediaId(101), "Title");
    let mut t = clip(TITLE.0, ClipKind::Video, 0, Tick::ZERO, PART, 1);
    t.r#gen = Some(title("FIRST"));
    inner.clips.push(t);
    inner.published = vec![PublishedParam {
        id: HEADLINE,
        name: "Heading".into(),
        target: ParamAddr::new(TITLE, ParamOwner::Generator, "text"),
        ty: ParamType::Text { multiline: false },
        range: PubRange::default(),
    }];
    p.sequences.push(inner);

    // the outer: the inner instance plus a backdrop, a FOREIGN handle sticks out
    let mut outer = Sequence::new(SEQ, "Issue");
    let mut mid = clip(20, ClipKind::Video, 0, Tick::ZERO, PART, 101);
    mid.source_in = Tick::ZERO;
    outer.clips.push(mid);
    outer.clips.push(clip(PLATE.0, ClipKind::Video, 1, Tick::ZERO, PART, 2));
    outer.ensure_tracks(kt_model::LaneKind::Video, 2);
    outer.published = vec![PublishedParam {
        id: PubId(50),
        name: "Release heading".into(),
        target: ParamAddr::new(ClipId(20), ParamOwner::Nested { pub_id: HEADLINE }, "text"),
        ty: ParamType::Text { multiline: false },
        range: PubRange::default(),
    }];
    p.sequences.push(outer);

    let mut a = clip(INST_A.0, ClipKind::Video, 0, Tick::ZERO, PART, SEQ.0);
    a.source_in = Tick::ZERO;
    p.clips.push(a);
    let mut b = clip(INST_B.0, ClipKind::Video, 0, PART, PART, SEQ.0);
    b.source_in = Tick::ZERO;
    p.clips.push(b);
    p.validate().expect("a template inside a template is legal");

    // the template value of the outer handle is read THROUGH two floors
    assert_eq!(
        p.published_value(SEQ, PubId(50)),
        Some(ParamValue::Text("FIRST".into())),
        "a template value through `Nested` was not read"
    );

    let mut q = p.clone();
    set_override(&mut q, INST_B, Override::value(PubId(50), ParamValue::Text("SECOND".into())));
    // the outer handle reached the SET of the inner instance
    let r = q.resolve_reel(Some(SEQ), &q.clips.iter().find(|c| c.id == INST_B).unwrap().overrides);
    let inner_inst = r.clips.iter().find(|c| same_clip(c.id, ClipId(20))).unwrap();
    assert_eq!(
        q.instance_value(inner_inst, HEADLINE),
        Some(ParamValue::Text("SECOND".into())),
        "an override of the outer handle did not reach the set of the inner instance"
    );

    let mid_t = Tick(FRAME * 10);
    let (fa, ca) = compose(&q, mid_t);
    let (fb, cb) = compose(&q, Tick(PART.0 + mid_t.0));
    assert!(ca && cb, "the composite is incomplete");
    assert_ne!(fa, fb, "a template inside a template gave no different frames");
}

// ---- 688. law E with baked-in overrides ------------------------------------

/// Test 688 (R.59, stage 3): LAW E is alive.
///
/// A nesting with an empty chain equals its expanded content — and this must remain
/// true UNDER OVERRIDES: the expansion
/// (`effective_clips`, the law's oracle, and what `kt-cli probe` prints)
/// shows the values of the INSTANCE, not of the template.
#[test]
fn t688_law_e_holds_with_overrides_baked_in() {
    let mut p = template();
    // one setting: law E is about the identity of the nesting with its content
    p.clips.retain(|c| c.id != INST_B);
    set_override(&mut p, INST_A, Override::value(HEADLINE, ParamValue::Text("SECOND".into())));

    // ORACLE: the expansion must carry the values of the instance
    let flat = p.effective_clips();
    let baked = flat
        .iter()
        .find_map(|c| c.r#gen.as_ref().and_then(|g| g.get_value("text")))
        .expect("there is no title in the expansion");
    assert_eq!(
        baked,
        ParamValue::Text("SECOND".into()),
        "the expansion showed the TEMPLATE: the oracle of LAW E would compare the subtree with the wrong one, \
         and `kt-cli probe` would answer the complaint “here is a different heading” with someone else’s text"
    );

    // ...and the expanded project draws the same as the subtree
    let mut n = ops::NestCounters::of(&p);
    let (cmds, rep) = ops::expand_instance(&p, INST_A, n.ids());
    assert_eq!(rep.overrides_baked, 1, "the expansion did not bake in the override");
    let mut m = TimelineModel::new(p.clone());
    m.apply_batch(cmds).expect("the expansion is legal");
    let spread = m.project().clone();

    for i in 0..20 {
        let t = Tick(i * FRAME);
        let (a, ca) = compose(&p, t);
        let (b, cb) = compose(&spread, t);
        assert_eq!(ca, cb, "completeness of the composite diverged on frame {i}");
        assert_eq!(
            a, b,
            "LAW E under overrides: frame {i} of the nested item and of the expanded content must match \
             BYTE FOR BYTE"
        );
    }
}

/// Test 675b (R.59, stage 3): an instance with an override of a generator has its
/// OWN source — and it gets into the `gen_sources` list.
///
/// Two different mechanisms depend on this, and both are silent when broken:
/// “order == read” (the address the plan ordered must exist) and
/// law P — the reconfiguration of values travels by the `retune` band BY ADDRESS, and
/// a retune of one address would leave the second edition with the old size until the
/// first rebuild of the plan (the same stale state that t663 guards against).
#[test]
fn t675b_an_overridden_instance_gets_its_own_generator_source() {
    let plain = template();
    let addrs = |p: &Project| -> Vec<MediaId> {
        let mut v: Vec<MediaId> = kt_engine::gen_sources(p).into_iter().map(|(a, _)| a).collect();
        v.sort();
        v.dedup();
        v
    };
    let before = addrs(&plain);

    let mut p = template();
    set_override(&mut p, INST_B, Override::value(HEADLINE, ParamValue::Text("SECOND".into())));
    let after = addrs(&p);
    assert_eq!(
        after.len(),
        before.len() + 1,
        "an instance with an override must have ITS OWN source address: \
         otherwise two releases read one frame, and the “second” silently shows the first"
    );
    // ...and the states at these addresses are REALLY different
    let texts: std::collections::BTreeSet<String> = kt_engine::gen_sources(&p)
        .into_iter()
        .filter_map(|(_, st)| match st.kind.get_value("text") {
            Some(ParamValue::Text(s)) => Some(s),
            _ => None,
        })
        .collect();
    assert!(
        texts.contains("FIRST") && texts.contains("SECOND"),
        "the list of sources does not carry both headings: {texts:?}"
    );

    // without overrides the list of addresses does NOT grow by a single one: the salt is
    // zero, and a project without publications pays nothing
    let mut bare = template();
    bare.sequence_mut(SEQ).unwrap().published.clear();
    assert_eq!(addrs(&bare), before, "publishing by itself created extra sources");
}

// ---- 752a. the leading geometry reaches the frame ----------------------------------

const GEO: PubId = PubId(5);

/// Test 752a (R.63 item 9): the published zoom of the LEADING GEOMETRY changes the frame
/// of the instance — the same way as a published title text.
///
/// The owner: “for what reason does the first geometry have no way to
/// publish parameters, while the second has one?”. The leading node lives with its own
/// event field, not an `fx` slot — and this is exactly the difference that
/// MUST NOT be visible from outside: the passport, `set_value` and the curves are the same
/// for it. The assertion here is observable: the frames of the two instances differ,
/// the order matched the read, and the reel's edition differs between them.
#[test]
fn t752a_a_published_lead_geometry_changes_the_instance_frame() {
    let mut p = template();
    {
        let s = p.sequences.iter_mut().find(|s| s.id == SEQ).expect("reel");
        // The TARGET is the title, not the backdrop: the backdrop is synthetic and uniform in the frame,
        // and the zoom of a uniform spot changes nothing — the test would be
        // green on nothing
        let title = s.clips.iter_mut().find(|c| c.id == TITLE).expect("title");
        title.geometry.zoom = 1.0;
        s.published.push(PublishedParam {
            id: GEO,
            name: "Heading size".into(),
            target: ParamAddr::new(TITLE, ParamOwner::Geometry, "zoom"),
            ty: ParamType::Num,
            range: PubRange::default(),
        });
    }
    p.validate().expect("a project with a handle on the leading geometry is legal");

    // the declaration ITSELF is free of cost: the frames of the instances coincide
    let mid = Tick(FRAME * 10);
    let (a0, ca) = compose(&p, mid);
    let (b0, cb) = compose(&p, Tick(PART.0 + mid.0));
    assert!(ca && cb, "the composite is incomplete — the order and the read diverged");
    assert_eq!(a0, b0, "declaring the handle changed the frame: publishing must be free");

    // ...and an override on the SECOND instance changes only it
    let mut z = p.clone();
    set_override(&mut z, INST_B, Override::value(GEO, ParamValue::Num(3.0)));
    let (a, ca) = compose(&z, mid);
    let (b, cb) = compose(&z, Tick(PART.0 + mid.0));
    assert!(ca && cb, "the composite is incomplete under a geometry override");
    assert_ne!(
        a, b,
        "the zoom of the leading geometry did not reach the frame of the instance: the node lives as an \
         event field, but outside it must be the same carrier as the slot"
    );
    assert_eq!(a, a0, "the override of the second instance changed the FIRST");
    assert!(b.iter().any(|&x| x != 0), "the frame is black — the test was verified on nothing");

    // the reel's editions differ — so the memo keys differ too
    let ov = &z.clips.iter().find(|c| c.id == INST_B).unwrap().overrides;
    assert_ne!(
        kt_engine::instance_content(&z, SEQ, ov),
        kt_engine::instance_content(&z, SEQ, &[]),
        "a revision with a geometry override equals the template one: the sub-composite would be shared \
         by both instances"
    );
}

// ---- 755. the EFFECT SLOT handle acts on the instance ------------------------

const AMOUNT: PubId = PubId(6);

/// Test 755 (R.63 item 10): a published handle of an EFFECT SLOT changes the frame
/// of the instance — checked on its own parameter, not on the zoom.
///
/// The owner: “I took MCR, put it into the composition, added Radial blur,
/// published the amount parameter, changed it in the composition — and it does not
/// change”. Until now the slot path was proven ONLY by the zoom (t679, where the
/// handle changes the decode scale) and by the title text — that is, by what touches the
/// source address. A numeric parameter of an ordinary pixel filter takes a
/// different route (value → `fx_chain_hash` → post-FX memo), and it had
/// no guard of its own.
#[test]
fn t755_a_published_effect_knob_changes_the_instance_frame() {
    let mut p = template();
    {
        let s = p.sequences.iter_mut().find(|s| s.id == SEQ).expect("reel");
        // a blur on the TITLE: the backdrop is synthetic and uniform, there is nothing to blur
        // in it — the test would be green on nothing
        let title = s.clips.iter_mut().find(|c| c.id == TITLE).expect("title");
        title.fx = vec![FxInstance::radial_blur()];
        title.fx_bypass = vec![false];
        s.published.push(PublishedParam {
            id: AMOUNT,
            name: "Blur".into(),
            target: ParamAddr::new(TITLE, ParamOwner::FxSlot { slot: 0 }, "amount"),
            ty: ParamType::Num,
            range: PubRange::default(),
        });
    }
    p.validate().expect("a project with a published blur handle is legal");

    let mid = Tick(FRAME * 10);
    let at_b = Tick(PART.0 + mid.0);

    // two instances: the first without blur, the second at full strength
    let mut z = p.clone();
    set_override(&mut z, INST_A, Override::value(AMOUNT, ParamValue::Num(0.0)));
    set_override(&mut z, INST_B, Override::value(AMOUNT, ParamValue::Num(0.9)));
    let (a, ca) = compose(&z, mid);
    let (b, cb) = compose(&z, at_b);
    assert!(ca && cb, "the composite is incomplete — the order and the read diverged");
    assert!(a.iter().any(|&x| x != 0), "the frame is black — the test was verified on nothing");
    assert_ne!(
        a, b,
        "the `amount` handle of the radial blur did not reach the frame of the instance — exactly what the \
         owner called “changed it in the composition, and it does not change”"
    );

    // ...and this is not “any two frames differ”: without overrides they are equal
    let (a0, _) = compose(&p, mid);
    let (b0, _) = compose(&p, at_b);
    assert_eq!(a0, b0, "the scene is degenerate: instances differ even without overrides");

    // the editions differ — so the memo keys and the sub-renderers differ
    let ov_a = &z.clips.iter().find(|c| c.id == INST_A).unwrap().overrides;
    let ov_b = &z.clips.iter().find(|c| c.id == INST_B).unwrap().overrides;
    assert_ne!(
        kt_engine::instance_content(&z, SEQ, ov_a),
        kt_engine::instance_content(&z, SEQ, ov_b),
        "two revisions with different blur gave one key — the sub-composite would be shared"
    );
    assert_eq!(
        VideoRenderer::new(&z).nest_count(),
        2,
        "instances with different handles must get different sub-renderers"
    );

    // an edit of the SECOND does not touch the first
    let mut z2 = z.clone();
    set_override(&mut z2, INST_B, Override::value(AMOUNT, ParamValue::Num(0.5)));
    assert_eq!(compose(&z2, mid).0, a, "an edit of the second instance changed the FIRST");
    assert_ne!(compose(&z2, at_b).0, b, "an edit of the second instance did not change itself");
}

// ---- 792b. the override formula changes the frame of the instance --------------------

/// Test 792b (R.67): the formula of an instance's override reaches the FRAME —
/// and sounds along the instance's axis: the frame of an instance with the formula
/// `1 + t / 10` at moment 3 s is byte for byte equal to the frame of an instance with a
/// constant override 1.3, and at moment 0 s — to the frame with 1.0.
#[test]
fn t792b_override_formula_reaches_the_frame_in_instance_time() {
    let build = |curve: Option<ParamCurve>, value: f64| -> Project {
        let mut p = base();
        let mut s = Sequence::new(SEQ, "Reel");
        let mut plate = clip(PLATE.0, ClipKind::Video, 0, Tick::ZERO, Tick(FRAME * 150), 2);
        plate.fx = vec![FxInstance::geometry_zoom(1.0)];
        s.clips.push(plate);
        s.published = vec![PublishedParam {
            id: ZOOM,
            name: "Push-in".into(),
            target: ParamAddr::new(PLATE, ParamOwner::FxSlot { slot: 0 }, "zoom"),
            ty: ParamType::Num,
            range: PubRange::default(),
        }];
        p.sequences.push(s);
        let mut a = clip(INST_A.0, ClipKind::Video, 0, Tick::ZERO, Tick(FRAME * 150), SEQ.0);
        a.overrides = vec![Override {
            pub_id: ZOOM,
            value: ParamValue::Num(value),
            curves: curve.into_iter().collect(),
        }];
        p.clips.push(a);
        p.validate().expect("the project is legal");
        p
    };
    let formula = ParamCurve {
        param: "zoom".into(),
        keys: vec![],
        source: CurveSource::Formula { expr: "1 + t / 10".into() },
    };
    let with_formula = build(Some(formula), 1.0);
    let const_13 = build(None, 1.3);
    let const_10 = build(None, 1.0);
    let t3 = Tick(3 * 90_000);
    let (f3, ok1) = compose(&with_formula, t3);
    let (c3, ok2) = compose(&const_13, t3);
    assert!(ok1 && ok2, "the composite is not fully populated");
    assert_eq!(f3, c3, "the formula 1 + t/10 over 3 s must give the same frame as the constant 1.3");
    let (f0, _) = compose(&with_formula, Tick::ZERO);
    let (c0, _) = compose(&const_10, Tick::ZERO);
    assert_eq!(f0, c0, "at zero the formula equals 1.0");
    assert_ne!(f0, f3, "the frame of the instance changes by the formula");
}
