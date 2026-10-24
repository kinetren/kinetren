//! Tests 671-673 and 690: public parameters
//! of a composition and per-instance overrides (R.59, stage 3).
//!
//! The owner: “someone made a template project for a news broadcast… obviously
//! people will not climb into the composition every time and change the text
//! and logos there.” Hence two halves: the set of controls declared by the author on the REEL
//! and the values of these controls on the INSTANCE.

use kt_model::*;
use kt_time::Tick;

const FPS: u32 = 30;
const FRAME: i64 = 90_000 / FPS as i64;
const LEN: Tick = Tick(FRAME * 30);

const SEQ: MediaId = MediaId(100);
const TITLE: ClipId = ClipId(1);
const PLATE: ClipId = ClipId(2);
const INST_A: ClipId = ClipId(10);
const INST_B: ClipId = ClipId(11);
const HEADLINE: PubId = PubId(1);
const BLUR: PubId = PubId(2);
const LOGO: PubId = PubId(3);

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: FPS,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    }
}

fn clip(id: u64, media: u64, start: Tick, dur: Tick) -> ClipEvent {
    ClipEvent {
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
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

fn title_kind(text: &str) -> GeneratorKind {
    let mut k = GeneratorKind::default_for_tag("text").expect("kind “text” is in the registry");
    k.set_value("text", &ParamValue::Text(text.into()));
    k
}

/// A news-broadcast template: a headline title and a background plate with blur.
fn template() -> Project {
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), gen_kind_path(&title_kind(""))));
    p.media.push(MediaRef::new(MediaId(2), "synth:2"));
    p.media.push(MediaRef::new(MediaId(3), "synth:3"));

    let mut s = Sequence::new(SEQ, "Issue");
    let mut title = clip(TITLE.0, 1, Tick::ZERO, LEN);
    title.r#gen = Some(title_kind("Default heading"));
    s.clips.push(title);
    let mut plate = clip(PLATE.0, 2, Tick::ZERO, LEN);
    plate.lane = 1;
    plate.fx = vec![FxInstance::Blur { radius: 8, radius_y: 0 }];
    s.clips.push(plate);
    s.ensure_tracks(LaneKind::Video, 2);
    s.published = vec![
        PublishedParam {
            id: HEADLINE,
            name: "News heading".into(),
            target: ParamAddr::new(TITLE, ParamOwner::Generator, "text"),
            ty: ParamType::Text { multiline: false },
            range: PubRange::default(),
        },
        PublishedParam {
            id: BLUR,
            name: "Backdrop blur".into(),
            target: ParamAddr::new(PLATE, ParamOwner::FxSlot { slot: 0 }, "radius"),
            ty: ParamType::Num,
            range: PubRange { min: Some(2.0), max: Some(16.0) },
        },
        PublishedParam {
            id: LOGO,
            name: "Logo".into(),
            target: ParamAddr::new(PLATE, ParamOwner::ClipAttr, "media"),
            ty: ParamType::Media,
            range: PubRange::default(),
        },
    ];
    p.sequences.push(s);

    let mut a = clip(INST_A.0, SEQ.0, Tick::ZERO, LEN);
    a.source_in = Tick::ZERO;
    p.clips.push(a);
    let mut b = clip(INST_B.0, SEQ.0, LEN, LEN);
    b.source_in = Tick::ZERO;
    p.clips.push(b);
    p.validate().expect("the template must be legal");
    p
}

fn text_of(p: &Project, inst: &[Override]) -> String {
    let r = p.resolve_reel(Some(SEQ), inst);
    let c = r.clips.iter().find(|c| same_clip(c.id, TITLE)).expect("title in place");
    match c.r#gen.as_ref().and_then(|g| g.get_value("text")) {
        Some(ParamValue::Text(s)) => s,
        other => panic!("a title has no text: {other:?}"),
    }
}

// ---- 671. commands, reading the template and the effective value ---------------------

/// Test 671 (R.59, stage 3): six commands and two questions that the model
/// must answer — “what is set in the template” and “what plays here”.
#[test]
fn t671_publishing_and_overriding_go_through_commands() {
    let mut p = template();
    // we start from a clean reel: we publish with a COMMAND, from inside the reel
    p.sequence_mut(SEQ).unwrap().published.clear();
    let mut m = TimelineModel::new(p);
    m.set_editing(Some(SEQ));

    let head = PublishedParam {
        id: HEADLINE,
        name: "News heading".into(),
        target: ParamAddr::new(TITLE, ParamOwner::Generator, "text"),
        ty: ParamType::Text { multiline: false },
        range: PubRange::default(),
    };
    let blur = PublishedParam {
        id: BLUR,
        name: "Backdrop blur".into(),
        target: ParamAddr::new(PLATE, ParamOwner::FxSlot { slot: 0 }, "radius"),
        ty: ParamType::Num,
        range: PubRange { min: Some(2.0), max: Some(16.0) },
    };
    m.apply_batch(vec![
        Command::PublishParam { seq: SEQ, param: Box::new(head.clone()) },
        Command::PublishParam { seq: SEQ, param: Box::new(blur.clone()) },
    ])
    .expect("publishing two live targets is legal");

    // a duplicate number — a rejection, not a silent overwrite
    assert!(
        m.apply_batch(vec![Command::PublishParam { seq: SEQ, param: Box::new(head.clone()) }]).is_err(),
        "two declarations with one number must be a rejection: the number is an ADDRESS"
    );
    // a target that does not exist — also a rejection
    let ghost = PublishedParam {
        target: ParamAddr::new(TITLE, ParamOwner::FxSlot { slot: 7 }, "radius"),
        id: PubId(9),
        ..head.clone()
    };
    assert!(
        m.apply_batch(vec![Command::PublishParam { seq: SEQ, param: Box::new(ghost) }]).is_err(),
        "publishing to a nonexistent slot must be a rejection: a handle that does nothing \
         is a minus that the reconnaissance named directly"
    );

    // order is part of the author’s interface
    m.apply_batch(vec![Command::ReorderPublished { seq: SEQ, order: vec![BLUR, HEADLINE] }])
        .expect("reordering is legal");
    assert_eq!(
        m.project().published.iter().map(|x| x.id).collect::<Vec<_>>(),
        vec![BLUR, HEADLINE],
        "handle order was not applied"
    );
    assert!(
        m.apply_batch(vec![Command::ReorderPublished { seq: SEQ, order: vec![BLUR] }]).is_err(),
        "the order must be a PERMUTATION: an incomplete list would silently lose a handle"
    );

    // renaming and narrowing the limit
    let renamed =
        PublishedParam { name: "Release heading".into(), ..head.clone() };
    m.apply_batch(vec![Command::SetPublished { seq: SEQ, param: Box::new(renamed) }])
        .expect("renaming is legal");
    assert_eq!(
        m.project().published.iter().find(|x| x.id == HEADLINE).map(|x| x.name.as_str()),
        Some("Release heading"),
        "the author name did not arrive"
    );

    // we exit outward — further edits are to INSTANCES
    m.set_editing(None);
    let p = m.project().clone();
    assert_eq!(
        p.published_value(SEQ, HEADLINE),
        Some(ParamValue::Text("Default heading".into())),
        "the value of the TEMPLATE is read from the target live — it must not be stored as a second copy"
    );

    let ov = Override::value(HEADLINE, ParamValue::Text("Second release".into()));
    m.apply_batch(vec![Command::SetOverride { id: INST_B, ov: Box::new(ov) }])
        .expect("overriding a published value is legal");
    let p = m.project().clone();
    let a = p.clips.iter().find(|c| c.id == INST_A).unwrap();
    let b = p.clips.iter().find(|c| c.id == INST_B).unwrap();
    assert_eq!(
        p.instance_value(a, HEADLINE),
        Some(ParamValue::Text("Default heading".into())),
        "an instance without an override must play the TEMPLATE"
    );
    assert_eq!(
        p.instance_value(b, HEADLINE),
        Some(ParamValue::Text("Second release".into())),
        "the override is the ONLY legal divergence of an instance from its reel"
    );
    assert_eq!(text_of(&p, &b.overrides), "Second release", "the value did not reach the contents");
    assert_eq!(text_of(&p, &a.overrides), "Default heading");

    // overriding an UNpublished one is not allowed (the owner’s decision)
    let stray = Override::value(PubId(77), ParamValue::Text("no such handle".into()));
    assert!(
        m.apply_batch(vec![Command::SetOverride { id: INST_B, ov: Box::new(stray) }]).is_err(),
        "only the published can be overridden: otherwise the template loses its interface"
    );
    // and of the wrong type — also not allowed
    let wrong = Override::value(HEADLINE, ParamValue::Num(1.0));
    assert!(
        m.apply_batch(vec![Command::SetOverride { id: INST_B, ov: Box::new(wrong) }]).is_err(),
        "the value type must match the declared one"
    );

    // narrowing the limit is part of the DECLARATION, not a decoration of the window
    let hard = Override::value(BLUR, ParamValue::Num(40.0));
    m.apply_batch(vec![Command::SetOverride { id: INST_A, ov: Box::new(hard) }])
        .expect("a number in the field is legal; clamping it is a matter of resolution");
    let p = m.project().clone();
    let a = p.clips.iter().find(|c| c.id == INST_A).unwrap();
    let got = p.resolve_reel(Some(SEQ), &a.overrides);
    let plate = got.clips.iter().find(|c| same_clip(c.id, PLATE)).unwrap();
    let radius = plate.fx[0].get_param("radius").unwrap();
    assert!(
        (radius - 16.0).abs() < 1e-9,
        "the author’s limit must clamp the instance value: got {radius}"
    );

    // resetting the row
    m.apply_batch(vec![Command::ClearOverride { id: INST_B, pub_id: HEADLINE }])
        .expect("reset is legal");
    let p = m.project().clone();
    let b = p.clips.iter().find(|c| c.id == INST_B).unwrap();
    assert!(b.overrides.is_empty(), "reset did not remove the override");
    assert_eq!(text_of(&p, &b.overrides), "Default heading", "reset did not restore the template");
}

// ---- 672. the fate of the target ---------------------------------------------------------

/// Test 672 (R.59, stage 3): the published address is attached to a `ClipId`.
///
/// It survives a trim and the LEFT half of a split (piece No. 0 keeps the id — the same rule
/// as for morpher cutting). When the target disappears, the publication disappears too, and
/// all its overrides in ALL instances, in ONE history step.
#[test]
fn t672_a_published_target_lives_and_dies_with_its_clip() {
    let mut p = template();
    p.clips.iter_mut().find(|c| c.id == INST_B).unwrap().overrides =
        vec![Override::value(HEADLINE, ParamValue::Text("Second release".into()))];
    p.clips.iter_mut().find(|c| c.id == INST_A).unwrap().overrides =
        vec![Override::value(BLUR, ParamValue::Num(4.0))];
    p.validate().expect("a template with overrides is legal");
    let mut m = TimelineModel::new(p);

    // --- trim of the target: the publication is intact --------------------------------------
    m.set_editing(Some(SEQ));
    m.apply_batch(vec![Command::TrimEnd { id: TITLE, new_end: Tick(LEN.0 - FRAME * 5) }])
        .expect("trim is legal");
    assert!(
        m.project().published.iter().any(|x| x.id == HEADLINE),
        "trimming the target has no right to kill the publication: the address is ClipId, not the window"
    );

    // --- split: piece No. 0 keeps the id, so the control too -----------------------
    m.apply_batch(vec![Command::Split {
        id: TITLE,
        at: Tick(FRAME * 10),
        new_id: ClipId(50),
        new_group: None,
    }])
    .expect("cut is legal");
    assert!(
        m.project().published.iter().any(|x| x.id == HEADLINE),
        "a cut must leave the handle on the LEFT half — it kept the id \
         (the same rule as for a morpher cut)"
    );
    m.set_editing(None);
    let b = m.project().clips.iter().find(|c| c.id == INST_B).unwrap();
    assert_eq!(b.overrides.len(), 1, "the override survived an edit inside the reel");

    // --- the target disappeared: the control and ALL its overrides go in one step --
    m.set_editing(Some(SEQ));
    m.apply_batch(vec![Command::RemoveClip { id: TITLE }]).expect("deletion is legal");
    m.set_editing(None);
    let p = m.project().clone();
    assert!(
        !p.sequence(SEQ).unwrap().published.iter().any(|x| x.id == HEADLINE),
        "the target disappeared — the publication must disappear too: a handle that does nothing \
         is exactly the minus the reconnaissance speaks of"
    );
    assert!(
        p.clips.iter().find(|c| c.id == INST_B).unwrap().overrides.is_empty(),
        "the override of a removed handle stayed on the instance"
    );
    assert_eq!(
        p.clips.iter().find(|c| c.id == INST_A).unwrap().overrides.len(),
        1,
        "a foreign handle (blur) was harmed by association"
    );
    p.validate().expect("after reconciliation the project must be legal");

    // ONE history step: undo restores the target, the control and the override
    m.undo().expect("a history step exists — the rollback must pass");
    // undo moves back to where the edit was (t454) — we exit outward again
    m.set_editing(None);
    let p = m.project().clone();
    assert!(
        p.sequence(SEQ).unwrap().published.iter().any(|x| x.id == HEADLINE),
        "undo did not restore the publication — so the reconciliation went as a SEPARATE step"
    );
    assert_eq!(
        p.clips.iter().find(|c| c.id == INST_B).unwrap().overrides.len(),
        1,
        "undo did not restore the override"
    );

    // ...and EXPANDING THE STRIP does not wipe out the publications. While the strip is open,
    // the reel’s content physically lies at the root, and its publications stay
    // in `sequences`: if we computed the fate of the target at that moment, every one
    // would look gone, and the expansion would silently eat the whole set.
    m.undo().expect("a history step exists — the rollback must pass");
    m.set_editing(None);
    assert!(m.expand_band(INST_A), "the expansion band must open");
    m.apply_batch(vec![Command::SetOpacity { id: INST_B, opacity: 0.5 }])
        .expect("an edit with an open band is legal");
    assert!(
        m.document().sequence(SEQ).unwrap().published.iter().any(|x| x.id == HEADLINE),
        "expanding the band wiped the publication"
    );
    m.collapse_all_bands();

    // removing the control by hand works the same way — across the whole tree at once
    m.set_editing(Some(SEQ));
    m.apply_batch(vec![Command::UnpublishParam { seq: SEQ, id: HEADLINE }]).expect("removal is legal");
    m.set_editing(None);
    assert!(
        m.project().clips.iter().find(|c| c.id == INST_B).unwrap().overrides.is_empty(),
        "a removed handle must take its overrides out of ALL instances"
    );
}

/// Test 672b: what validation must explicitly reject.
#[test]
fn t672b_validation_refuses_dangling_and_mistyped_overrides() {
    let mut p = template();
    p.clips.iter_mut().find(|c| c.id == INST_A).unwrap().overrides =
        vec![Override::value(PubId(77), ParamValue::Num(1.0))];
    assert!(
        matches!(p.validate(), Err(ProjectError::OverrideDangling(INST_A, PubId(77)))),
        "an override of an unpublished value must fail loading: otherwise \
         the instance silently plays the template and there is nothing to explain it with"
    );

    let mut p = template();
    p.clips.iter_mut().find(|c| c.id == INST_A).unwrap().overrides =
        vec![Override::value(HEADLINE, ParamValue::Num(1.0))];
    assert!(matches!(p.validate(), Err(ProjectError::OverrideTypeMismatch(INST_A, HEADLINE))));

    // an override on an ORDINARY event
    let mut p = template();
    let mut stray = clip(60, 3, Tick(LEN.0 * 3), LEN);
    stray.overrides = vec![Override::value(HEADLINE, ParamValue::Text("no".into()))];
    p.clips.push(stray);
    assert!(matches!(p.validate(), Err(ProjectError::OverrideOnPlainClip(ClipId(60)))));

    // a curve on a STEPPED type
    let mut p = template();
    p.clips.iter_mut().find(|c| c.id == INST_A).unwrap().overrides = vec![Override {
        pub_id: HEADLINE,
        value: ParamValue::Text("once".into()),
        curves: vec![ParamCurve {
            param: "text".into(),
            keys: vec![AutoKey::at(Tick::ZERO, 0.0)],
            source: CurveSource::Keys,
        }],
    }];
    assert!(
        matches!(p.validate(), Err(ProjectError::OverrideCurveNotAllowed(INST_A, HEADLINE))),
        "between “Heading” and another heading there is no half — a crooked one has nowhere to come from"
    );

    // a declaration on a target of ANOTHER type
    let mut p = template();
    p.sequence_mut(SEQ).unwrap().published[0].ty = ParamType::Num;
    assert!(matches!(p.validate(), Err(ProjectError::PublishedTypeMismatch(Some(SEQ), HEADLINE))));

    // a declaration without a name
    let mut p = template();
    p.sequence_mut(SEQ).unwrap().published[0].name = "  ".into();
    assert!(matches!(p.validate(), Err(ProjectError::PublishedUnnamed(Some(SEQ), HEADLINE))));
}

// ---- 673. empty set — the former path -----------------------------------------

/// Test 673 (R.59, stage 3): `resolve_reel(seq, &[])` — the FORMER path.
///
/// The zero-cost requirement (“if the project is simple, with no fancy tricks, it must not lag
/// at all”) is expressed literally here: without overrides
/// the reel’s content must come out of the same code, with no clone of the reel and no
/// clone of its transitions and cameras.
#[test]
fn t673_an_empty_override_set_takes_the_old_path() {
    let p = template();

    // 1) byte equality with the former clip-output door
    assert_eq!(
        p.resolve_reel(Some(SEQ), &[]).clips,
        p.effective_reel(Some(SEQ)),
        "an empty set must give EXACTLY the same as the old door"
    );

    // 2) not a single clone: transitions and cameras are BORROWED
    let r = p.resolve_reel(Some(SEQ), &[]);
    assert!(
        matches!(r.transitions, std::borrow::Cow::Borrowed(_))
            && matches!(r.cameras, std::borrow::Cow::Borrowed(_)),
        "an empty set clones the reel contents — and that is the price that must not exist"
    );

    // 3) and a set CONSISTING OF FOREIGN controls also goes the former path: it
    //    changes nothing, so there is nothing to clone
    let alien = [Override::value(PubId(999), ParamValue::Num(1.0))];
    let r = p.resolve_reel(Some(SEQ), &alien);
    assert!(
        matches!(r.transitions, std::borrow::Cow::Borrowed(_)),
        "a set without a single live handle must take the previous path"
    );

    // 4) the publication ITSELF does not move the content: a reel with a declared set
    //    and a reel without one give the same clips
    let mut bare = template();
    bare.sequence_mut(SEQ).unwrap().published.clear();
    assert_eq!(
        bare.effective_reel(Some(SEQ)),
        p.effective_reel(Some(SEQ)),
        "declaring a handle changed the contents of the reel — so publishing is not free"
    );
}

// ---- 690. the strip shows the template, expansion bakes ----------------------

/// Test 690 (R.59, stage 3): the expansion strip is a REFERENCE, expanding is a
/// COPY.
///
/// The strip shows the TEMPLATE and must reassemble byte-for-byte together with
/// the overrides (they sit on the INSTANCE, which does not move into the strip).
/// `expand_instance` is an irreversible action: the reel under the pieces no longer exists,
/// the control has nowhere to live, so the values are BAKED IN and the count is named aloud.
#[test]
fn t690_a_band_shows_the_template_and_expanding_bakes_the_overrides() {
    let mut p = template();
    // one reel — one instance: the expansion strip is counted from it
    p.clips.retain(|c| c.id != INST_B);
    p.clips.iter_mut().find(|c| c.id == INST_A).unwrap().overrides =
        vec![Override::value(HEADLINE, ParamValue::Text("Second release".into()))];
    p.ensure_tracks(LaneKind::Video, 1);
    p.validate().expect("the scene is legal");

    // --- strip: expand and collect — BYTE-FOR-BYTE identity ---------------
    let before = p.clone();
    let b = Band::of(&before, INST_A).expect("band for an instance");
    let mut q = before.clone();
    q.merge_reel(&b);
    let inner = q.clips.iter().find(|c| c.id == TITLE).expect("reel contents in the band");
    assert_eq!(
        inner.r#gen.as_ref().and_then(|g| g.get_value("text")),
        Some(ParamValue::Text("Default heading".into())),
        "the expansion band must show the TEMPLATE: it is a reference, not a copy, and \
         an edit in it edits the REEL — that is, all instances at once"
    );
    q.unmerge_reel(&b).expect("reassembly must pass");
    assert_eq!(q, before, "expanding and reassembling stopped being an identity");

    // --- expanding: the values are BAKED IN ------------------------------
    let mut m = TimelineModel::new(p);
    let mut n = ops::NestCounters::of(m.project());
    let (cmds, rep) = ops::expand_instance(m.project(), INST_A, n.ids());
    assert_eq!(rep.overrides_baked, 1, "expanding must say how many it baked");
    m.apply_batch(cmds).expect("the expansion is legal");
    let p = m.project();
    let titles: Vec<&ClipEvent> = p.clips.iter().filter(|c| c.r#gen.is_some()).collect();
    assert_eq!(titles.len(), 1, "the title did not expand");
    assert_eq!(
        titles[0].r#gen.as_ref().and_then(|g| g.get_value("text")),
        Some(ParamValue::Text("Second release".into())),
        "“what you saw is what you got”: the expanded title must carry the value of the INSTANCE — \
         it no longer has a reference to the reel, and the handle has nowhere to live"
    );
    assert!(
        p.sequence(SEQ).unwrap().clips.iter().any(|c| c
            .r#gen
            .as_ref()
            .and_then(|g| { g.get_value("text") })
            == Some(ParamValue::Text("Default heading".into()))),
        "baking has no right to touch the reel ITSELF"
    );
}

// ---- 692, 694 (R.60). the target’s spec sheet is the only source of limits --------

/// Test 694 (R.60): a property’s spec sheet BY ADDRESS exists for every owner.
///
/// Before R.60 the model answered “what type is this target” (`addr_type`) and “what
/// value it has” (`addr_value`), but not “what limits does it have” — and the author’s
/// window drew an empty narrowing as zero. The spec sheet must come from the same place
/// as the type, and match it by type: if they diverge, a color would get the spec sheet of a
/// component, while its type is a group.
#[test]
fn t694_addr_spec_answers_for_every_owner() {
    let mut p = template();

    // effect slot: limits and default are the effect’s own spec sheet
    let fx = ParamAddr::new(PLATE, ParamOwner::FxSlot { slot: 0 }, "radius");
    let s = p.addr_spec(Some(SEQ), &fx).expect("slot passport");
    assert_eq!(Some(s.ty.clone()), p.addr_type(Some(SEQ), &fx), "type diverged from addr_type");
    let native = FxInstance::Blur { radius: 8, radius_y: 0 }
        .param_spec("radius")
        .expect("Blur has a radius")
        .clone();
    assert_eq!((s.min, s.max, s.default), (native.min, native.max, native.default));
    assert!(s.max > 0.0, "slot passport came back empty: {s:?}");

    // a generator on an event
    let gen_addr = ParamAddr::new(TITLE, ParamOwner::Generator, "text");
    let s = p.addr_spec(Some(SEQ), &gen_addr).expect("generator passport");
    assert_eq!(Some(s.ty), p.addr_type(Some(SEQ), &gen_addr), "generator type diverged");

    // an event attribute: the limits are the same ones the model clamps it with
    let att = ParamAddr::new(PLATE, ParamOwner::ClipAttr, "opacity");
    let s = p.addr_spec(Some(SEQ), &att).expect("attribute passport");
    assert_eq!(Some(s.ty.clone()), p.addr_type(Some(SEQ), &att));
    assert_eq!((s.min, s.max, s.default), (0.0, 1.0, 1.0), "level limits are wrong");
    let media_att = ParamAddr::new(PLATE, ParamOwner::ClipAttr, "media");
    assert_eq!(
        p.addr_spec(Some(SEQ), &media_att).map(|s| s.ty),
        p.addr_type(Some(SEQ), &media_att),
        "the media reference’s passport and type diverged"
    );

    // transition and camera have their own numbers, `clip` does not take part in the address
    {
        let s = p.sequences.iter_mut().find(|s| s.id == SEQ).expect("reel");
        s.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(TITLE),
            b: Some(PLATE),
            in_offset: Tick(FRAME),
            out_offset: Tick(FRAME),
            kind: TransitionKind::default(),
            audio: Default::default(),
            auto: Vec::new(),
            invert: false,
        });
    }
    let tr_name = {
        let s = p.sequences.iter().find(|s| s.id == SEQ).expect("reel");
        s.transitions[0].kind.param_specs().first().map(|x| x.name.to_string())
    };
    if let Some(name) = tr_name {
        let addr = ParamAddr::new(ClipId(0), ParamOwner::Transition { id: TransitionId(1) }, &name);
        let s = p.addr_spec(Some(SEQ), &addr).expect("transition passport");
        assert_eq!(Some(s.ty), p.addr_type(Some(SEQ), &addr), "transition type diverged");
    }

    // target gone — no spec sheet, just as there is no type
    let gone = ParamAddr::new(ClipId(999), ParamOwner::FxSlot { slot: 0 }, "radius");
    assert!(p.addr_spec(Some(SEQ), &gone).is_none(), "passport of a dead target");
    assert!(p.addr_type(Some(SEQ), &gone).is_none());
    let no_such = ParamAddr::new(PLATE, ParamOwner::FxSlot { slot: 0 }, "no_such");
    assert!(p.addr_spec(Some(SEQ), &no_such).is_none(), "passport of a nonexistent name");

    // a nested publication: the TARGET’s spec sheet goes outward, narrowed by the inner
    // control — the consumer of the outer reel gets no more than the
    // author of the inner one allowed.
    let mut outer = Sequence::new(MediaId(200), "Wrapper");
    let mut inst = clip(50, SEQ.0, Tick::ZERO, LEN);
    inst.media = SEQ;
    outer.clips.push(inst);
    p.media.push(MediaRef::new(MediaId(200), "seq:200"));
    p.sequences.push(outer);
    let nested = ParamAddr::new(ClipId(50), ParamOwner::Nested { pub_id: BLUR }, "radius");
    let s = p.addr_spec(Some(MediaId(200)), &nested).expect("passport of a nested handle");
    assert_eq!(
        (s.min, s.max),
        (2.0, 16.0),
        "narrowing of the inner handle did not apply to the target passport: {s:?}"
    );
    assert!(
        s.default >= s.min && s.default <= s.max,
        "default moved outside the narrowed limits: {s:?}"
    );
}

/// Test 692 (R.60): the narrowing is stored ONLY as a narrowing.
///
/// “The min and max values are always set to 0 by default” — an empty
/// `Option` was drawn as zero. The reverse half of the rule: a limit equal to
/// the target’s limit must be written to the file as absence, otherwise the very first
/// publication would write a copy of the spec sheet there, and it would diverge from the target when
/// the effect’s version changes (the same argument as against a stored template
/// value, R.59).
#[test]
fn t692_a_published_range_stores_narrowing_only() {
    let p = template();
    let addr = ParamAddr::new(PLATE, ParamOwner::FxSlot { slot: 0 }, "radius");
    let spec = p.addr_spec(Some(SEQ), &addr).expect("target passport");

    // a freshly published narrowing control carries nothing
    let fresh = PubRange::default();
    assert!(fresh.is_empty(), "a fresh publication already narrowed something");
    assert_eq!(
        (fresh.min.unwrap_or(spec.min), fresh.max.unwrap_or(spec.max)),
        (spec.min, spec.max),
        "the author’s window would show not the target’s limits"
    );

    // the target’s limits set by hand are the ABSENCE of a narrowing
    let same = PubRange::narrowed(&spec, spec.min, spec.max);
    assert!(same.is_empty(), "the target’s limits were recorded as a copy: {same:?}");

    // narrowed only from above — below, still nothing
    let hi = PubRange::narrowed(&spec, spec.min, spec.max / 2.0);
    assert_eq!(hi.min, None, "a narrowing appeared from nowhere at the bottom");
    assert_eq!(hi.max, Some(spec.max / 2.0));

    // an attempt to WIDEN the target’s limits with a narrowing does not take hold
    let wide = PubRange::narrowed(&spec, spec.min - 100.0, spec.max + 100.0);
    assert!(wide.is_empty(), "a widening was recorded as a narrowing: {wide:?}");

    // returning to the target’s limit removes the narrowing
    let back = PubRange::narrowed(&spec, spec.min, spec.max);
    assert!(back.is_empty(), "returning to the target’s limit did not remove the narrowing");

    // clamp in both directions — on top of the target’s limits
    let narrow = PubRange { min: Some(4.0), max: Some(6.0) };
    assert_eq!(narrow.clamp(1.0), 4.0);
    assert_eq!(narrow.clamp(99.0), 6.0);

    // and serialization: an empty narrowing does not create fields
    let json = serde_json::to_string(&PublishedParam {
        id: BLUR,
        name: "Blur".into(),
        target: addr,
        ty: ParamType::Num,
        range: PubRange::default(),
    })
    .expect("serialization");
    assert!(!json.contains("min"), "an empty narrowing leaked into the file: {json}");
    assert!(!json.contains("max"), "an empty narrowing leaked into the file: {json}");
}

// ---- 752. leading geometry — the same carrier of a control -------------------------

const ZOOM: PubId = PubId(4);

/// Test 752 (R.63 item 9): the LEADING GEOMETRY is published like any slot.
///
/// The owner: “for what reason does the first geometry have no way to
/// publish parameters while the second one does? this first geometry runs
/// as an fx, doesn’t it? or do we have duplicated code?” Not duplicates — a HOLE IN ADDRESSING:
/// the leading node lives in its own event field, and `ParamOwner` did not know about it.
/// Its spec sheet, `set_value` and curves are exactly the same as for a slot, so the
/// test asserts the same thing that t671/t672 assert about a slot: the template
/// value is read live, the override acts with a number and a curve,
/// the address survives a trim and the left half of a split, the target’s death takes the control with it,
/// and an audio event has no such address.
#[test]
fn t752_the_lead_geometry_is_a_publishable_owner() {
    let mut p = template();
    {
        let s = p.sequence_mut(SEQ).expect("reel");
        s.published.push(PublishedParam {
            id: ZOOM,
            name: "Backdrop scale".into(),
            target: ParamAddr::new(PLATE, ParamOwner::Geometry, "zoom"),
            ty: ParamType::Num,
            range: PubRange::default(),
        });
    }
    p.validate().expect("a handle on the leading geometry is legal");

    // --- the template VALUE is read live, as for a slot --------------------
    assert_eq!(
        p.published_value(SEQ, ZOOM),
        Some(ParamValue::Num(1.0)),
        "the template value must be read FROM THE TARGET, not stored in the declaration"
    );
    let spec = p
        .addr_spec(Some(SEQ), &ParamAddr::new(PLATE, ParamOwner::Geometry, "zoom"))
        .expect("target passport");
    assert!(spec.min > 0.0 && spec.max >= 32.0, "the passport did not come from a geometry node: {spec:?}");

    // --- an override with a NUMBER changes the instance’s frame --------------------
    let zoom_of = |p: &Project, ov: &[Override]| -> f64 {
        let r = p.resolve_reel(Some(SEQ), ov);
        r.clips.iter().find(|c| same_clip(c.id, PLATE)).expect("backdrop").geometry.zoom
    };
    assert_eq!(zoom_of(&p, &[]), 1.0, "without an override — the template value");
    assert_eq!(
        zoom_of(&p, &[Override::value(ZOOM, ParamValue::Num(2.5))]),
        2.5,
        "the zoom override did not reach the leading node"
    );
    assert_eq!(
        p.sequence(SEQ).unwrap().clips.iter().find(|c| c.id == PLATE).unwrap().geometry.zoom,
        1.0,
        "the instance override rewrote the reel ITSELF"
    );

    // --- a curve lands in `geometry_auto` and is translated into the target’s material ---
    let curve = ParamCurve {
        param: "zoom".into(),
        keys: vec![AutoKey::at(Tick::ZERO, 1.0), AutoKey::at(Tick(FRAME * 10), 3.0)],
        source: CurveSource::Keys,
    };
    let ov = vec![Override { pub_id: ZOOM, value: ParamValue::Num(2.5), curves: vec![curve] }];
    let r = p.resolve_reel(Some(SEQ), &ov);
    let plate = r.clips.iter().find(|c| same_clip(c.id, PLATE)).expect("backdrop");
    assert_eq!(plate.geometry_auto.len(), 1, "the curve did not arrive at the home of the leading node");
    assert_eq!(plate.geometry_auto[0].param, "zoom");
    assert_eq!(
        plate.geometry_auto[0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
        vec![0, FRAME * 10],
        "keyframes were not converted to the target material by the same code as the slot"
    );
    // and it is LEGITIMATE: the curve’s carrier is declared
    let mut with = p.clone();
    with.clips.iter_mut().find(|c| c.id == INST_A).unwrap().overrides = ov.clone();
    with.validate().expect("a curve on the leading geometry is a legal carrier");

    // --- the conform RULE is the same kind of property (choice) -------------------
    {
        let s = p.sequence_mut(SEQ).expect("reel");
        s.published.push(PublishedParam {
            id: PubId(5),
            name: "Frame rule".into(),
            target: ParamAddr::new(PLATE, ParamOwner::Geometry, "rule"),
            ty: ParamType::Choice { variants: std::borrow::Cow::Borrowed(CONFORM_KEYS) },
            range: PubRange::default(),
        });
    }
    p.validate().expect("a handle on the rule is legal");
    let r = p.resolve_reel(Some(SEQ), &[Override::value(
        PubId(5),
        ParamValue::Choice("fill".into()),
    )]);
    assert_eq!(
        r.clips.iter().find(|c| same_clip(c.id, PLATE)).unwrap().geometry.rule,
        Conform::Fill,
        "the conform rule did not override"
    );

    // --- an AUDIO EVENT has NO such address ----------------------
    let mut bad = template();
    {
        let s = bad.sequence_mut(SEQ).expect("reel");
        let mut a = clip(70, 3, Tick::ZERO, LEN);
        a.kind = ClipKind::Audio;
        a.lane = 0;
        s.clips.push(a);
        s.ensure_tracks(LaneKind::Audio, 1);
        s.published.push(PublishedParam {
            id: ZOOM,
            name: "Zoom on audio".into(),
            target: ParamAddr::new(ClipId(70), ParamOwner::Geometry, "zoom"),
            ty: ParamType::Num,
            range: PubRange::default(),
        });
    }
    assert_eq!(
        bad.addr_type(Some(SEQ), &ParamAddr::new(ClipId(70), ParamOwner::Geometry, "zoom")),
        None,
        "an audio event has no leading node — the address must be NONEXISTENT"
    );
    assert!(
        matches!(bad.validate(), Err(ProjectError::PublishedTargetMissing(Some(SEQ), ZOOM))),
        "geometry publication on audio passed validation silently"
    );

    // --- the TARGET’S FATE: the same as for a slot ----------------------------------
    let mut m = TimelineModel::new(p.clone());
    m.set_editing(Some(SEQ));
    m.apply_batch(vec![Command::TrimEnd { id: PLATE, new_end: Tick(LEN.0 - FRAME * 5) }])
        .expect("trim is legal");
    assert!(
        m.project().published.iter().any(|x| x.id == ZOOM),
        "trimming the target killed the geometry handle: the address is ClipId, not the window"
    );
    m.apply_batch(vec![Command::Split {
        id: PLATE,
        at: Tick(FRAME * 10),
        new_id: ClipId(51),
        new_group: None,
    }])
    .expect("cut is legal");
    assert!(
        m.project().published.iter().any(|x| x.id == ZOOM),
        "a cut must leave the handle on the LEFT half"
    );
    m.apply_batch(vec![Command::RemoveClip { id: PLATE }]).expect("deletion is legal");
    m.set_editing(None);
    assert!(
        !m.project().sequence(SEQ).unwrap().published.iter().any(|x| x.id == ZOOM),
        "the target disappeared, but the geometry handle was left hanging"
    );
    m.project().validate().expect("after reconciliation the project is legal");
}

// ---- 753. the command names the reel, not the context --------------------------------

const SEQ2: MediaId = MediaId(200);
const FROM_BAND: PubId = PubId(10);
const FROM_INSIDE: PubId = PubId(11);
const FROM_OTHER: PubId = PubId(12);
const ON_TRANS: PubId = PubId(13);

/// Test 753 (R.63 item 11): publication addresses the REEL, not the place where
/// the person stands.
///
/// The owner: “if I publish a parameter without fully entering the composition —
/// why do we automatically enter the composition in full?” The entry was
/// not a whim of the interface but a consequence of the command: it governed “the reel
/// you are standing in”, and from an expanded strip the control would land at the ROOT. Now
/// the reel is named in the command itself — and the three entry points give one result,
/// `editing()` does not move, and a non-existent reel gets a rejection.
#[test]
fn t753_publishing_names_the_reel_not_the_context() {
    let mut p = template();
    // a neighbouring reel — to issue the command FROM ANOTHER reel
    let mut s2 = Sequence::new(SEQ2, "Neighbor");
    s2.clips.push(clip(30, 3, Tick::ZERO, LEN));
    s2.ensure_tracks(LaneKind::Video, 1);
    p.sequences.push(s2);
    let mut inst2 = clip(20, SEQ2.0, Tick(LEN.0 * 2), LEN);
    inst2.lane = 1;
    p.clips.push(inst2);
    p.ensure_tracks(LaneKind::Video, 2);
    // and a transition INSIDE the first reel: its control is addressed by the same `seq`
    {
        let s = p.sequence_mut(SEQ).expect("reel");
        let mut b = clip(3, 2, LEN, LEN);
        b.lane = 1;
        s.clips.push(b);
        s.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(PLATE),
            b: Some(ClipId(3)),
            in_offset: Tick(FRAME * 2),
            out_offset: Tick(FRAME * 2),
            // a crossfade has no numbers of its own at all — there is nothing to publish;
            // we take the variant that has parameters
            kind: TransitionKind::Wipe { angle: 0.0, softness: 0.15, dir: Default::default() },
            audio: Default::default(),
            auto: Vec::new(),
            invert: false,
        });
    }
    p.validate().expect("the scene is legal");
    let mut m = TimelineModel::new(p);

    let decl = |id: PubId, name: &str, target: ParamAddr| PublishedParam {
        id,
        name: name.to_string(),
        target,
        ty: ParamType::Num,
        range: PubRange::default(),
    };
    let zoom = |c: ClipId| ParamAddr::new(c, ParamOwner::Geometry, "zoom");

    // --- 1) FROM THE ROOT, reel expanded by the strip ------------------------------
    assert!(m.expand_band(INST_A), "the expansion band must open");
    assert_eq!(m.editing(), None, "scene: the person stands at the ROOT");
    m.apply_batch(vec![Command::PublishParam {
        seq: SEQ,
        param: Box::new(decl(FROM_BAND, "From band", zoom(PLATE))),
    }])
    .expect("publishing from the band is legal");
    assert_eq!(
        m.editing(),
        None,
        "the publish confirmation WENT INTO the reel — exactly what the owner called \
         “why do we automatically go fully into the composition”"
    );
    assert!(
        m.document().sequence(SEQ).unwrap().published.iter().any(|x| x.id == FROM_BAND),
        "the handle did not land in the named reel"
    );
    // the reel’s transition is addressed by the SAME `seq` from the same strip (this closes
    // the “Deferred” item of R.59: before, it could not be reached)
    m.apply_batch(vec![Command::PublishParam {
        seq: SEQ,
        param: Box::new(PublishedParam {
            id: ON_TRANS,
            name: "Seam softness".into(),
            target: ParamAddr::new(
                ClipId(0),
                ParamOwner::Transition { id: TransitionId(1) },
                "softness",
            ),
            ty: m
                .document()
                .addr_type(
                    Some(SEQ),
                    &ParamAddr::new(
                        ClipId(0),
                        ParamOwner::Transition { id: TransitionId(1) },
                        "softness",
                    ),
                )
                .expect("a crossfade has softness"),
            range: PubRange::default(),
        }),
    }])
    .expect("publishing a transition parameter from the band is legal");
    assert!(
        m.document().sequence(SEQ).unwrap().published.iter().any(|x| x.id == ON_TRANS),
        "the transition handle did not land in the named reel"
    );
    m.collapse_all_bands();

    // --- 2) FROM INSIDE the reel itself ------------------------------------------
    m.set_editing(Some(SEQ));
    m.apply_batch(vec![Command::PublishParam {
        seq: SEQ,
        param: Box::new(decl(FROM_INSIDE, "From inside", zoom(TITLE))),
    }])
    .expect("publishing from inside is legal");
    assert!(
        m.project().published.iter().any(|x| x.id == FROM_INSIDE),
        "standing INSIDE the reel, the handle did not land in that same reel"
    );
    m.set_editing(None);

    // --- 3) FROM ANOTHER reel ------------------------------------------------
    m.set_editing(Some(SEQ2));
    m.apply_batch(vec![Command::PublishParam {
        seq: SEQ,
        param: Box::new(decl(FROM_OTHER, "From neighbor", zoom(ClipId(3)))),
    }])
    .expect("publishing into the NAMED reel from another one is legal");
    assert_eq!(m.editing(), Some(SEQ2), "the command pulled the person out of their reel");
    m.set_editing(None);

    // all three controls are in ONE reel, and it is the named one
    let set = m.document().sequence(SEQ).unwrap().published.clone();
    for id in [FROM_BAND, FROM_INSIDE, FROM_OTHER, ON_TRANS] {
        assert!(set.iter().any(|x| x.id == id), "handle {id:?} did not reach the reel: {set:?}");
    }
    assert!(
        m.document().sequence(SEQ2).unwrap().published.is_empty(),
        "the handle moved to the neighboring reel — exactly where it would have died in the same batch"
    );
    assert!(m.document().published.is_empty(), "the handle landed at the ROOT: it is no one’s instance");

    // --- 4) NON-EXISTENT reel — rejection with the address ------------------------
    let ghost = MediaId(777);
    assert_eq!(
        m.apply_batch(vec![Command::PublishParam {
            seq: ghost,
            param: Box::new(decl(PubId(14), "Nowhere", zoom(TITLE))),
        }]),
        Err(CommandError::SequenceNotFound(ghost)),
        "publishing into a nonexistent reel must be a REJECTION with the address"
    );
    assert_eq!(
        m.apply_batch(vec![Command::UnpublishParam { seq: ghost, id: FROM_BAND }]),
        Err(CommandError::SequenceNotFound(ghost))
    );

    // --- 5) removing and editing a declaration are addressed the same way -------------------
    m.apply_batch(vec![Command::SetPublished {
        seq: SEQ,
        param: Box::new(decl(FROM_BAND, "Renamed", zoom(PLATE))),
    }])
    .expect("editing a declaration from the root is legal");
    assert_eq!(
        m.document()
            .sequence(SEQ)
            .unwrap()
            .published
            .iter()
            .find(|x| x.id == FROM_BAND)
            .map(|x| x.name.clone()),
        Some("Renamed".to_string()),
        "the declaration edit did not reach the named reel"
    );
    m.apply_batch(vec![Command::UnpublishParam { seq: SEQ, id: FROM_BAND }])
        .expect("removal from the root is legal");
    assert!(
        !m.document().sequence(SEQ).unwrap().published.iter().any(|x| x.id == FROM_BAND),
        "the removal did not reach the named reel"
    );
    m.document().validate().expect("the document must remain legal");
}

// ---- 792. override curves: forms, layer, formula — in the reel domain -------

/// Test 792 (R.67): an instance override accepts the same forms and sources
/// as a slot curve, and resolves them IN THE REEL DOMAIN (the instance’s axis)
/// before translating into the target’s time. The structure is the owner’s project:
/// two instances of one reel stacked on top of each other, with the curve on the upper one. On
/// the previous code the formula was rejected by `CurveSourceUnsupported`, and the window silently
/// erased the field.
#[test]
fn t792_override_curves_take_shapes_and_sources_in_the_reel_domain() {
    let mut p = template();
    // the second instance is ABOVE the first, at the same time (reel V4 above V1)
    let b = p.clips.iter_mut().find(|c| c.id == INST_B).unwrap();
    b.start = Tick::ZERO;
    b.lane = 3;
    p.ensure_tracks(LaneKind::Video, 4);
    p.validate().expect("two instances one above the other are legal");
    let radius_at = |p: &Project, ov: &[Override], t: i64| -> f64 {
        let r = p.resolve_reel(Some(SEQ), ov);
        let plate = r.clips.iter().find(|c| same_clip(c.id, PLATE)).unwrap();
        match plate.fx_auto.first().and_then(|s| s.iter().find(|c| c.param == "radius")) {
            Some(c) => curve_value(&c.keys, Tick(t)),
            // without a curve — the slot’s value (template or a constant override)
            None => plate.fx[0].get_param("radius").expect("blur has a radius"),
        }
    };
    let sec = 90_000;
    // --- a hump with keys 0→16→0 (handle limits 2..16) ------------------------
    let hill = |shape: SegShape| Override {
        pub_id: BLUR,
        value: ParamValue::Num(8.0),
        curves: vec![ParamCurve {
            param: "radius".into(),
            keys: vec![
                AutoKey::shaped(Tick(0), 2.0, shape),
                AutoKey::shaped(Tick(sec / 2), 16.0, shape),
                AutoKey::shaped(Tick(sec), 2.0, shape),
            ],
            source: CurveSource::Keys,
        }],
    };
    let mut m = TimelineModel::new(p.clone());
    m.apply_batch(vec![Command::SetOverride { id: INST_B, ov: Box::new(hill(SegShape::Linear)) }])
        .expect("a hill made of keyframes is legal");
    let p1 = m.project().clone();
    let ov = &p1.clips.iter().find(|c| c.id == INST_B).unwrap().overrides;
    assert!((radius_at(&p1, ov, sec / 2) - 16.0).abs() < 1e-9, "top of the hill");
    assert!((radius_at(&p1, ov, 0) - 2.0).abs() < 1e-9 && (radius_at(&p1, ov, sec) - 2.0).abs() < 1e-9, "edges");
    // the lower instance (without a set) plays the template
    let a = p1.clips.iter().find(|c| c.id == INST_A).unwrap();
    assert!((radius_at(&p1, &a.overrides, sec / 2) - 8.0).abs() < 1e-9, "the bottom one is the template");

    // --- zigzag on the segment ------------------------------------------------
    let zig = SegShape::Zigzag { periods: 4.0, amplitude: 2.0, profile: ZigProfile::Triangle };
    m.apply_batch(vec![Command::SetOverride { id: INST_B, ov: Box::new(hill(zig)) }])
        .expect("a segment shape on an override is legal");
    let p2 = m.project().clone();
    let ov = &p2.clips.iter().find(|c| c.id == INST_B).unwrap().overrides;
    let plain = 2.0 + (16.0 - 2.0) * 0.25;
    assert!((radius_at(&p2, ov, sec / 8) - plain).abs() > 0.1, "zigzag is silent at the target");

    // --- layer k * 0.5 over the hump ------------------------------------------
    let mut layered = hill(SegShape::Linear);
    layered.curves[0].source = CurveSource::Formula { expr: "k * 0.5".into() };
    m.apply_batch(vec![Command::SetOverride { id: INST_B, ov: Box::new(layered) }])
        .expect("a layer over keyframes is legal (on the previous code — CurveSourceUnsupported)");
    let p3 = m.project().clone();
    let ov = &p3.clips.iter().find(|c| c.id == INST_B).unwrap().overrides;
    assert!((radius_at(&p3, ov, sec / 2) - 8.0).abs() < 1e-6, "k*0.5 at the peak: {}", radius_at(&p3, ov, sec / 2));
    // the hump’s keys in the MODEL are intact
    assert_eq!(ov[0].curves[0].keys.len(), 3, "the layer did not erase the keyframes");

    // --- formula source t/10 on the instance’s axis ---------------------------
    let formula = Override {
        pub_id: BLUR,
        value: ParamValue::Num(8.0),
        curves: vec![ParamCurve { param: "radius".into(), keys: vec![], source: CurveSource::Formula { expr: "2 + t * 4".into() } }],
    };
    m.apply_batch(vec![Command::SetOverride { id: INST_B, ov: Box::new(formula) }])
        .expect("a formula on an override is legal");
    let p4 = m.project().clone();
    let ov = &p4.clips.iter().find(|c| c.id == INST_B).unwrap().overrides;
    // 0.5 s from the start of the instance → 2 + 0.5·4 = 4
    assert!((radius_at(&p4, ov, sec / 2) - 4.0).abs() < 1e-6, "t is computed along the instance axis: {}", radius_at(&p4, ov, sec / 2));

    // --- link outward from the reel — rejection with a reason ------------------------------
    let outside = Override {
        pub_id: BLUR,
        value: ParamValue::Num(8.0),
        curves: vec![ParamCurve {
            param: "radius".into(),
            keys: vec![],
            source: CurveSource::Link { path: format!("{}/fx0/radius", INST_A.0), expr: None, stops: vec![] },
        }],
    };
    let e = m.apply_batch(vec![Command::SetOverride { id: INST_B, ov: Box::new(outside) }]);
    assert!(e.is_err(), "a link across a reel boundary must be a rejection");
    assert!(format!("{e:?}").contains("outside the reel"), "reason not named: {e:?}");
}
