//! Tests of the generator vocabulary (R.52, item 402): parsing/building of the `gen:` path,
//! escaping of string values, completeness of the parameter specs.

use kt_model::{
    GeneratorKind, HasParams, escape_gen_value, format_gen_path, is_pseudo_path, parse_gen_path,
    unescape_gen_value,
};

/// t469: `format(parse(s))` canonicalizes, parsing is case-insensitive
/// (regression of the item 211 footnote: `synth:Ramp` was silently counted as a missing
/// file), garbage gives an honest `None`, defaults are not written.
#[test]
fn t469_gen_path_parse_format_canonical() {
    // canonical round trip of all registry kinds with defaults
    for k in GeneratorKind::all() {
        let path = format_gen_path(&k);
        assert!(path.starts_with("gen:"), "{path}");
        assert_eq!(parse_gen_path(&path), Some(k.clone()), "{path}");
        assert_eq!(format_gen_path(&parse_gen_path(&path).unwrap()), path);
    }

    // a title with NON-defaults: a string with escaping, style, colours and the layer
    // order survive the lossless path (the title text is C2)
    let mut title = GeneratorKind::default_text();
    if let GeneratorKind::Text {
        text, bold, size, align, anchor, fill_a, stroke_w, order, ..
    } = &mut title
    {
        *text = "Hello: a=b %25\nsecond line".into();
        *bold = true;
        *size = 0.12;
        *align = kt_model::TextAlign::Left;
        *anchor = kt_model::TextAnchor::BottomCenter;
        *fill_a = 200;
        *stroke_w = 0.05;
        *order = [
            kt_model::TextLayer::Shadow,
            kt_model::TextLayer::Plate,
            kt_model::TextLayer::Fill,
            kt_model::TextLayer::Stroke,
        ];
    }
    let path = format_gen_path(&title);
    assert!(!path.contains('\n'), "the path stays one line: {path:?}");
    assert_eq!(parse_gen_path(&path), Some(title.clone()), "{path}");
    assert_eq!(format_gen_path(&parse_gen_path(&path).unwrap()), path, "canon is stable");

    // defaults are not serialized: a black solid is a bare tag
    assert_eq!(
        format_gen_path(&GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }),
        "gen:solid"
    );

    // A non-default is written as one colour token, in lowercase.
    let orange = GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 };
    assert_eq!(format_gen_path(&orange), "gen:solid:c=ff8800");
    assert_eq!(parse_gen_path("gen:solid:c=ff8800"), Some(orange.clone()));

    // The case of the schema, tag, key and hex value does not matter.
    assert_eq!(parse_gen_path("Gen:SOLID:C=FF8800"), Some(orange));
    // ...and canonicalization returns lowercase.
    assert_eq!(
        format_gen_path(&parse_gen_path("GEN:Solid:c=FF8800").unwrap()),
        "gen:solid:c=ff8800"
    );

    // Garbage — None, not a guess: an unknown tag, an unknown key, a broken value, a token
    // without '=', an empty tag, a foreign schema.
    for bad in [
        "gen:unknown",
        "gen:solid:zz=1",
        "gen:solid:c=xyzxyz",
        "gen:solid:c=ff88",
        "gen:solid:ff8800",
        "gen:",
        "synth:ramp",
        "clip.mp4",
        "",
    ] {
        assert_eq!(parse_gen_path(bad), None, "{bad:?}");
    }

    // A single predicate for pseudo-paths: both schemes, any case, but not files.
    assert!(is_pseudo_path("gen:solid"));
    assert!(is_pseudo_path("GEN:solid"));
    assert!(is_pseudo_path("synth:ramp"));
    assert!(is_pseudo_path("Synth:Ramp"));
    assert!(!is_pseudo_path("media/clip.mp4"));
    assert!(!is_pseudo_path("generator.mp4"));
}

/// t469b: escaping of the string value of a lossless token for any string —
/// the title text will stand on it (C2). Service characters of a path and control
/// characters go to %XX, the path stays one string.
#[test]
fn t469b_gen_value_escaping_roundtrip() {
    let cases = [
        "",
        "Hello, world!",
        "a:b=c%d",
        "first\nsecond\r\nthird",
        "tabs\tand zeros\u{1}",
        "百分号 %25 already escaped",
        "العربية and 中文 🎬",
    ];
    for s in cases {
        let esc = escape_gen_value(s);
        assert!(!esc.contains(':') && !esc.contains('=') && !esc.contains('\n'), "{esc:?}");
        assert_eq!(unescape_gen_value(&esc).as_deref(), Some(s), "{esc:?}");
    }
    // Broken sequences — None.
    for bad in ["%", "%f", "%zz", "truncated %2"] {
        assert_eq!(unescape_gen_value(bad), None, "{bad:?}");
    }
}

/// t470: the specs are complete (a relative of t223 for transitions): every kind of the registry
/// has specs, the defaults lie within [min, max] and match the values of the registry
/// instance itself, get/set walk every name.
#[test]
fn t470_generator_param_specs_are_complete() {
    // Kinds WITHOUT numbers are declared here by name (R.53, items 420/422): “specs are empty”
    // is a legitimate state for exactly them, and a new kind that lost its specs through
    // oversight still fails this test.
    const NO_PARAMS: &[&str] = &["transparent", "table"];
    for k in GeneratorKind::all() {
        let specs = k.param_specs();
        assert_eq!(
            specs.is_empty(),
            NO_PARAMS.contains(&k.tag()),
            "{}: specs and the declaration of “kind without numbers” diverged",
            k.tag()
        );
        let mut probe = k.clone();
        for s in specs {
            assert!(
                s.min <= s.default && s.default <= s.max,
                "{}.{}: default out of range",
                k.tag(),
                s.name
            );
            let v = k
                .get_param(&s.name)
                .unwrap_or_else(|| panic!("{}.{}: get_param does not know its own name", k.tag(), s.name));
            assert_eq!(v, s.default, "{}.{}: registry not at default", k.tag(), s.name);
            assert!(probe.set_param(&s.name, s.max), "{}.{}: set_param", k.tag(), s.name);
            assert_eq!(probe.get_param(&s.name), Some(s.max));
        }
        assert_eq!(k.get_param("no_such_param"), None);
        assert!(!probe.set_param("no_such_param", 1.0));
        let _ = &probe;
    }
}

/// Test 475 (R.52): the command to change a media path — an edit of generator parameters.
/// check (the media exists), the mutation is visible to all clips of this media,
/// undo/redo work, a live drag of the window collapses into ONE step.
#[test]
fn t475_set_media_path_command() {
    use kt_model::{
        ClipEvent, ClipId, ClipKind, Command, CommandError, MediaId, Project, ProjectSettings,
        TimelineModel,
    };
    use kt_time::Tick;

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
    p.media.push(kt_model::MediaRef::new(MediaId(1), "gen:solid"));
    p.clips.push(ClipEvent {
        r#gen: Some(kt_model::GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }),
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
        duration: Tick(90_000),
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
    let mut m = TimelineModel::new(p);

    // a nonexistent media — an honest failure
    assert!(matches!(
        m.apply(Command::SetMediaPath { id: MediaId(9), path: "gen:solid".into() }),
        Err(CommandError::MediaNotFound(MediaId(9)))
    ));

    // R.59, stage 2: the media path carries ONLY THE KIND, so a full path is reduced to its
    // canonical form, and the values go to the event. The previous expectation (“the path
    // stores the parameters”)
    // was removed by the owner’s decision “generator parameters — on the event”.
    m.apply(Command::SetMediaPath { id: MediaId(1), path: "gen:solid:c=ff8800".into() }).unwrap();
    assert_eq!(m.project().media[0].path, "gen:solid", "the kind stays in the path");
    assert!(m.project().is_gen(&m.project().clips[0]));

    // changing the KIND re-dresses the events of this media: otherwise the first edit would
    // hit “the value kind is not the one in the path”
    m.apply(Command::SetMediaPath { id: MediaId(1), path: "gen:gradient".into() }).unwrap();
    assert_eq!(m.project().media[0].path, "gen:gradient");
    assert!(
        matches!(m.project().clips[0].r#gen, Some(kt_model::GeneratorKind::Gradient { .. })),
        "an event must be updated together with the media"
    );

    // undo restores the previous kind TOGETHER WITH the values of the event
    m.undo().unwrap();
    assert_eq!(m.project().media[0].path, "gen:solid");
    assert!(matches!(m.project().clips[0].r#gen, Some(kt_model::GeneratorKind::Solid { .. })));
    m.redo().unwrap();
    assert_eq!(m.project().media[0].path, "gen:gradient");

    // window session: three edits in one gesture = ONE undo step
    let before_depth = m.undo_depth();
    for c in ["gen:checker", "gen:corners", "gen:transparent"] {
        m.apply_coalesced(Command::SetMediaPath { id: MediaId(1), path: c.into() }, 42).unwrap();
    }
    assert_eq!(m.project().media[0].path, "gen:transparent");
    assert_eq!(m.undo_depth(), before_depth + 1, "the gesture collapsed into one step");
    m.undo().unwrap();
    assert_eq!(m.project().media[0].path, "gen:gradient", "undo of the gesture — back to before the session");
}

/// Test 510 (R.53, item 414): kind flags — a full passport. Every bool field of a kind MUST
/// be in `flag_specs`: otherwise it exists in the format, and there is nothing to switch it
/// on with (this is how the 1 kHz tone of SMPTE and the coloured noise mode lived under R.52).
/// Completeness is checked by the serde representation — it sees ALL fields, not those
/// the author remembered.
#[test]
fn t510_generator_flags_are_complete_and_roundtrip() {
    use kt_model::GeneratorKind;
    for k in GeneratorKind::all() {
        let v = serde_json::to_value(&k).expect("kind is serialized");
        let obj = v.as_object().expect("kind is an object");
        let bools: Vec<&str> =
            obj.iter().filter(|(_, v)| v.is_boolean()).map(|(k, _)| k.as_str()).collect();
        let declared: Vec<&str> = k.flag_specs().iter().map(|s| s.name).collect();
        for b in &bools {
            assert!(
                declared.contains(b),
                "{}: field {b} is in the format but not declared by a flag — there is no way to enable it",
                k.tag()
            );
        }
        for d in &declared {
            assert!(bools.contains(d), "{}: flag {d} does not exist in the format", k.tag());
        }
        // reading and writing walk every name, the path survives the round trip
        let mut probe = k.clone();
        for spec in k.flag_specs() {
            let was = probe.get_flag(spec.name).expect("flag reads");
            assert!(probe.set_flag(spec.name, !was), "flag {} is not written", spec.name);
            assert_eq!(probe.get_flag(spec.name), Some(!was));
        }
        let path = kt_model::format_gen_path(&probe);
        assert_eq!(kt_model::parse_gen_path(&path), Some(probe), "{path}: flags did not survive the path");
    }

    // and the main consequence: “tone on” and “coloured noise” are expressible
    let tone = GeneratorKind::SmpteBars { tone: true, tone_db: -20.0 };
    assert_eq!(tone.get_flag("tone"), Some(true));
    let colour = GeneratorKind::NoiseField { amount: 1.0, mono: false };
    assert_eq!(colour.get_flag("mono"), Some(false));
}

/// Test 524 (R.53, items 419-422): the new kinds travel the path there and back,
/// and the canonical form stays canonical.
///
/// One parsing rule for all kinds: `format(parse(s)) == format(s)`, defaults are not written,
/// a broken value is `None`, not a silent clamp.
#[test]
fn t524_new_kinds_round_trip_through_the_path() {
    use kt_model::{TestTable, format_gen_path, parse_gen_path};
    // every registry kind: defaults give a BARE path without tokens
    for k in GeneratorKind::all() {
        let p = format_gen_path(&k);
        assert_eq!(parse_gen_path(&p).as_ref(), Some(&k), "{p}: the path did not return its kind");
        assert_eq!(format_gen_path(&parse_gen_path(&p).unwrap()), p, "{p}: canon is not canon");
    }
    // the table variety — ONE tag for five kinds
    for t in TestTable::ALL {
        let k = GeneratorKind::TestPattern { table: t };
        let p = format_gen_path(&k);
        assert!(p.starts_with("gen:table"), "{p}");
        assert_eq!(parse_gen_path(&p), Some(k), "{p}");
    }
    assert_eq!(format_gen_path(&GeneratorKind::Transparent), "gen:transparent");
    assert_eq!(parse_gen_path("gen:transparent"), Some(GeneratorKind::Transparent));
    assert_eq!(parse_gen_path("gen:transparent:c=ff0000"), None, "a backdrop has no parameters");

    // values outside the range — an error, not a silent clamp
    assert_eq!(parse_gen_path("gen:rect:cx=1.5"), None);
    assert_eq!(parse_gen_path("gen:rect:w=3"), None, "size is more than two shares of the axis");
    assert!(parse_gen_path("gen:rect:w=1.8").is_some(), "a shape may extend beyond the frame");
    assert_eq!(parse_gen_path("gen:line:th=nan"), None);
    assert_eq!(parse_gen_path("gen:table:tb=nope"), None);
    assert_eq!(parse_gen_path("gen:leader:n=0"), None);

    // a shape colour — WITH ALPHA (eight hex digits)
    let path = "gen:rect:c=ff8800c0";
    let Some(GeneratorKind::Rect { color_r, color_a, .. }) = parse_gen_path(path) else {
        panic!("{path} did not parse");
    };
    assert_eq!((color_r, color_a), (0xff, 0xc0));
    assert_eq!(parse_gen_path("gen:rect:c=ff8800"), None, "six digits — this is the path without alpha");
}

/// Test 525 (R.53, item 419): “a kind = an enum variant when it has ITS OWN numbers”.
/// A guard against merging shapes into one kind with dead controls (UI rule 9) and against
/// five copies of the same code for the tables.
#[test]
fn t525_kinds_differ_by_their_numbers() {
    use std::collections::BTreeSet;
    let names = |k: &GeneratorKind| -> BTreeSet<String> {
        k.param_specs().iter().map(|s| s.name.to_string()).collect()
    };
    let rect = GeneratorKind::default_for_tag("rect").unwrap();
    let ell = GeneratorKind::default_for_tag("ellipse").unwrap();
    let line = GeneratorKind::default_for_tag("line").unwrap();
    assert_ne!(names(&rect), names(&ell), "rectangle and ellipse have the same numbers");
    assert_ne!(names(&ell), names(&line));
    assert!(names(&rect).contains("radius"), "rounding exists only on the rectangle");
    assert!(!names(&ell).contains("radius"), "on an ellipse rounding would be a dead control");

    // the tables have no numbers at all — therefore the kind is ONE, and the variety is a field
    for t in kt_model::TestTable::ALL {
        assert!(GeneratorKind::TestPattern { table: t }.param_specs().is_empty());
    }
}

// ---- R.59, stage 2: generator parameters on the EVENT ----------------------

/// Test 656: the model carries the generator values on the event, and the media path carries
/// ONLY THE KIND; the bytes of files without generators do not move meanwhile.
///
/// Three assertions, and each holds its own piece of the round:
/// “the path keeps the kind”
/// (otherwise two titles with different text would again become two media), “the variant sort
/// is not taken from the path” (the kind name is a function of the value — rule item 419),
/// “empty fields are not serialized” (otherwise old projects would stop being byte-stable).
#[test]
fn t656_generator_values_live_on_the_event() {
    use kt_model::{ClipEvent, ClipId, ClipKind, GeneratorKind, MediaId, TestTable, gen_kind_path};
    use kt_time::Tick;

    // the path carries the kind; the values are wiped from it
    let mut orange = GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 };
    assert_eq!(gen_kind_path(&orange), "gen:solid");
    orange = GeneratorKind::default_text();
    if let GeneratorKind::Text { text, size, .. } = &mut orange {
        *text = "Heading".into();
        *size = 0.3;
    }
    assert_eq!(gen_kind_path(&orange), "gen:text", "title text does not live in the media path");

    // The SORT of the variant is part of the address: five tables are five menu rows, and
    // the kind name for them is a function of the value
    assert_eq!(gen_kind_path(&GeneratorKind::TestPattern { table: TestTable::Ebu75 }), "gen:table");
    assert_eq!(
        gen_kind_path(&GeneratorKind::TestPattern { table: TestTable::Grayscale }),
        "gen:table:tb=grey"
    );
    // and the kind path always parses back into the SAME kind
    for k in GeneratorKind::all() {
        let back = kt_model::parse_gen_path(&gen_kind_path(&k)).expect("a kind path parses");
        assert_eq!(back.tag(), k.tag(), "the kind did not survive a round trip through the path");
        assert_eq!(gen_kind_path(&back), gen_kind_path(&k), "the kind path is not canonical");
    }

    // empty fields of an event are not serialized: a file without a generator is byte-for-byte
    let plain = ClipEvent {
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
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    let json = serde_json::to_string(&plain).unwrap();
    assert!(!json.contains("\"gen\""), "field gen ended up in a file without a generator: {json}");
    assert!(!json.contains("gen_auto"), "field gen_auto ended up in a file without a generator");

    // and with a generator — it survives the round trip
    let mut g = plain.clone();
    g.r#gen = Some(GeneratorKind::Solid { color_r: 1, color_g: 2, color_b: 3 });
    g.gen_auto = kt_model::text_anim_curves(kt_model::TextAnim::Typewriter, 1.0, 2.0);
    let back: ClipEvent = serde_json::from_str(&serde_json::to_string(&g).unwrap()).unwrap();
    assert_eq!(back, g, "generator values and curves did not survive a round trip through serde");

    // address space: a piece of a morph layer also fits under GEN_TAG
    let slice = kt_model::slice_id(ClipId(kt_model::MAX_REAL_CLIP_ID - 1), u16::MAX as u32);
    assert!(slice.0 < kt_model::GEN_TAG, "the piece address intruded into the generator range");
    let addr = kt_model::GEN_TAG + slice.0;
    assert_eq!(addr, kt_model::GEN_TAG | slice.0, "the generator tag must not overlap with ids");
    assert!(addr < kt_model::NEST_TAG, "the generator address intruded into the reel range");
}

/// Test 657: validation of generator values — WITHOUT LEGACY.
///
/// The strictness here is free: values appear and disappear exactly together
/// with the media
/// (`AddClip`/`SetClipMedia`/`SetMediaPath` keep this themselves), so any mismatch is a damaged
/// file, not a state someone could honestly have ended up in.
#[test]
fn t657_generator_values_are_validated_against_the_media() {
    use kt_model::{
        AutoKey, ClipEvent, ClipId, ClipKind, GeneratorKind, MediaId, MediaRef, ParamCurve,
        Project, ProjectError, ProjectSettings, TestTable,
    };
    use kt_time::Tick;

    let mk = || {
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
        p.media.push(MediaRef::new(MediaId(1), "gen:solid"));
        p.media.push(MediaRef::new(MediaId(2), "movie.mp4"));
        p.clips.push(ClipEvent {
            r#gen: Some(GeneratorKind::Solid { color_r: 0, color_g: 0, color_b: 0 }),
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
            duration: Tick(90_000),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
        p
    };
    mk().validate().expect("a consistent state is legal");

    // on a generator, but without values
    let mut p = mk();
    p.clips[0].r#gen = None;
    assert!(matches!(p.validate(), Err(ProjectError::GenMissing(ClipId(1)))));

    // values on the FILE
    let mut p = mk();
    p.clips[0].media = MediaId(2);
    assert!(matches!(p.validate(), Err(ProjectError::GenOnFile(ClipId(1)))));

    // the value kind is not the one in the media path
    let mut p = mk();
    p.clips[0].r#gen = Some(GeneratorKind::TestPattern { table: TestTable::Ebu75 });
    assert!(matches!(p.validate(), Err(ProjectError::GenKindMismatch(ClipId(1)))));
    // and the SORT is also part of the kind
    let mut p = mk();
    p.media[0].path = "gen:table".into();
    p.clips[0].r#gen = Some(GeneratorKind::TestPattern { table: TestTable::Grayscale });
    assert!(matches!(p.validate(), Err(ProjectError::GenKindMismatch(ClipId(1)))));

    // an orphan curve is REJECTED: an event has exactly one kind, an orphan has nowhere to come from,
    // and a silently inactive curve looks like “the animation disappeared” (for `fx_auto` the rule
    // is different — there a slot could change its kind)
    let curve = |name: &str| ParamCurve {
        param: name.into(),

        keys: AutoKey::linear(vec![AutoKey::at(Tick(0), 0.0), AutoKey::at(Tick(90_000), 255.0)]),
        source: Default::default(),
    };
    let mut p = mk();
    p.clips[0].gen_auto = vec![curve("color_r")];
    p.validate().expect("a curve on an existing parameter is legal");
    let mut p = mk();
    p.clips[0].gen_auto = vec![curve("nope")];
    assert!(matches!(p.validate(), Err(ProjectError::GenAutoOrphan(ClipId(1), _))));

    // structural check: keys in ascending order, values finite
    let mut p = mk();
    let mut bad = curve("color_r");
    bad.keys.reverse();
    p.clips[0].gen_auto = vec![bad];
    assert!(matches!(p.validate(), Err(ProjectError::BadFxAuto(ClipId(1)))));

    // the formula is COMPUTED under R.61; under R.67 it is a LAYER over the keys (owner’s decision:
    // “if I introduce a formula — the points will disappear… I may have changed my mind”): the keys
    // under it live on and are read as `k`, there is no failure
    let mut p = mk();
    let mut formula = curve("color_r");
    formula.source = kt_model::CurveSource::Formula { expr: "t * 2".into() };
    p.clips[0].gen_auto = vec![formula.clone()];
    assert!(p.validate().is_ok(), "a formula over keyframes is a layer, not a rejection (R.67): {:?}", p.validate());
    // but a foreign name in the text is still a failure
    formula.source = kt_model::CurveSource::Formula { expr: "t * zz".into() };
    p.clips[0].gen_auto = vec![formula.clone()];
    assert!(
        matches!(p.validate(), Err(ProjectError::CurveSourceBroken(ClipId(1), _))),
        "an unknown name must be a rejection"
    );
    formula.source = kt_model::CurveSource::Formula { expr: "t * 2".into() };
    formula.keys.clear();
    p.clips[0].gen_auto = vec![formula.clone()];
    p.validate().expect("a formula without keyframes is legal");

    // an UNPARSEABLE text — a loud failure, not a silent render by the keys
    formula.source = kt_model::CurveSource::Formula { expr: "(((".into() };
    p.clips[0].gen_auto = vec![formula];
    assert!(matches!(p.validate(), Err(ProjectError::CurveSourceBroken(ClipId(1), _))));
}
