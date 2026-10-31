//! Test 1048 (plugin-api-stage1, stage 6; capability `plugins`): the generator kinds keep their
//! frames, sound, paths and file text when they move into plugins.
//!
//! Before any kind moves, every row of `GeneratorKind::all()` is printed as a digest — the frame
//! bytes at three sizes and four material ticks, the first sound bursts, the canonical `gen:` path,
//! the media address (`gen_kind_path`), the parse of its own path and the serde text — at the
//! defaults and "tuned" (every number row at 70 % of its range, every flag flipped, so the tone and
//! the beep sound), and frozen here. The move must leave every row unchanged (D13).

use kt_media::generator::GeneratorSource;
use kt_media::{MediaSource, Quality};
use kt_model::{GeneratorKind, HasParams, ParamType};
use kt_time::Tick;

/// The kind with every NUMBER row at 70 % of its range and every flag flipped.
fn tuned(kind: &GeneratorKind) -> GeneratorKind {
    let mut k = kind.clone();
    let specs: Vec<_> = k.param_specs().to_vec();
    for s in specs.iter().filter(|s| s.ty == ParamType::Num) {
        k.set_param(&s.name, s.min + 0.7 * (s.max - s.min));
    }
    let flags: Vec<_> = k.flag_specs().iter().map(|f| f.name.to_string()).collect();
    for f in flags {
        let v = k.get_flag(&f).unwrap_or(false);
        k.set_flag(&f, !v);
    }
    k
}

/// A row's name in the table: the tag, and the table's sort (five rows share one tag).
fn row_name(kind: &GeneratorKind) -> String {
    kt_model::gen_kind_path(kind).trim_start_matches("gen:").replace(":tb=", ".")
}

fn digest(kind: &GeneratorKind) -> u64 {
    let mut text = String::new();
    for (case, k) in [("default", kind.clone()), ("tuned", tuned(kind))] {
        let path = kt_model::format_gen_path(&k);
        let back = kt_model::parse_gen_path(&path).map(|p| kt_model::format_gen_path(&p));
        text.push_str(&format!(
            "{case} path={path} addr={} back={back:?} serde={}\n",
            kt_model::gen_kind_path(&k),
            serde_json::to_string(&k).expect("a kind serializes")
        ));
        let mut src = GeneratorSource::new(k.clone());
        let info = src.info().clone();
        text.push_str(&format!("  info audio={} {}x{}\n", info.has_audio, info.width, info.height));
        for (w, h) in [(64u32, 36u32), (33, 17), (16, 16)] {
            for t in [0i64, 45_000, 181_234, 90_000 * 8 - 1_500] {
                let f = src.read_video_frame(Tick(t), Some(Quality { width: w, height: h })).expect("a frame");
                text.push_str(&format!(
                    "  frame {w}x{h}@{t} {}x{} alpha={} {:016x}\n",
                    f.width,
                    f.height,
                    f.has_alpha,
                    xxhash_rust::xxh3::xxh3_64(&f.rgba)
                ));
            }
        }
        for idx in [0i64, 46, 47, 375] {
            let b = src.read_audio_burst(idx).expect("a burst");
            let bits: Vec<u8> = b.data.iter().flat_map(|s| s.to_bits().to_le_bytes()).collect();
            text.push_str(&format!("  burst {idx} {:016x}\n", xxhash_rust::xxh3::xxh3_64(&bits)));
        }
    }
    xxhash_rust::xxh3::xxh3_64(text.as_bytes())
}

/// (row, digest) — printed with `KT_PRINT_GOLDEN=1` on the enum before any kind moved.
#[rustfmt::skip]
const GOLDEN: &[(&str, u64)] = &[
    ("solid", 0xd41eb0b2d6a14f6c),
    ("text", 0x333866116f96e81b),
    ("gradient", 0xd6878afb4fd5f672),
    ("corners", 0x6bf073e96d4eb234),
    ("checker", 0x4d275b93bac63f67),
    ("noise", 0xc7eb00ea3bfe9ebb),
    ("smpte", 0x833b6c631cbfc729),
    ("timecode", 0xdb3a48e123337e2c),
    ("transparent", 0xf7fa05dde4495edd),
    ("rect", 0x964102f171dcd6f7),
    ("ellipse", 0xc73a295196640d45),
    ("line", 0x0f07f2373d715e19),
    ("leader", 0x1ab9c803b450bfd5),
    ("table", 0xeaef91eee8588557),
    ("table.ebu100", 0xf103eb155e037fa4),
    ("table.grey", 0x6078f7bcc39e4d29),
    ("table.steps", 0xcc8933f8c1dc86b2),
    ("table.ycbcr", 0x2f7ce50c358f4d84),
    ("window", 0x9febc6db00122de9),
];

#[test]
fn t1048_a_moved_generator_kind_keeps_its_frames_sound_and_text() {
    kt_plugins_builtin::install();
    let kinds = GeneratorKind::all();
    if std::env::var("KT_PRINT_GOLDEN").is_ok() {
        for k in &kinds {
            println!("    (\"{}\", 0x{:016x}),", row_name(k), digest(k));
        }
        return;
    }
    let mut problems = Vec::new();
    for k in &kinds {
        let name = row_name(k);
        match GOLDEN.iter().find(|(t, _)| *t == name) {
            None => problems.push(format!("{name}: no golden row (completeness lock)")),
            Some((_, want)) if digest(k) != *want => problems.push(format!("{name}: the kind moved")),
            _ => {}
        }
    }
    assert_eq!(kinds.len(), GOLDEN.len(), "a golden row without its kind");
    assert!(problems.is_empty(), "{problems:#?}");
}

/// Test 1050 (plugin-api-stage1, D13): a generator kind whose plugin this build lacks. A file from a
/// build with more plugins opens: the event's kind reads as `Missing`, the save writes its bytes
/// back unchanged, and the source is a NAMED placeholder — an opaque frame that says what is absent,
/// not a black or transparent slab.
#[test]
fn t1050_a_missing_generator_kind_is_kept_and_named() {
    use kt_model::project_file::{from_text, to_text};
    use kt_model::*;
    kt_plugins_builtin::install();
    let raw = r#"{"kind":"wobble_gen","amp":0.25,"mode":"sine"}"#;
    let missing: GeneratorKind = serde_json::from_str(raw).expect("an unknown generator kind still reads");
    assert!(matches!(&missing, GeneratorKind::Missing(m) if m.id() == "wobble_gen"), "{missing:?}");
    assert_eq!(gen_kind_path(&missing), "gen:wobble_gen", "the media address of a missing kind is its id");

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
    p.media.push(MediaRef::new(MediaId(1), "gen:wobble_gen"));
    p.clips.push(ClipEvent {
        r#gen: Some(missing.clone()),
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
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: Vec::new(),
        opacity: 1.0,
        fx_off: false,
    });
    let text = to_text(&p, None);
    let read = from_text(&text).expect("a file with a missing generator kind opens");
    let kind = read.project.clips[0].r#gen.clone().expect("the event keeps its kind");
    let GeneratorKind::Missing(m) = &kind else { panic!("not kept as missing: {kind:?}") };
    assert_eq!(serde_json::to_string(&m.raw).unwrap(), raw, "the raw content changed");
    assert_eq!(to_text(&read.project, None), text, "the save changed the bytes");

    let mut src = kt_media::factory::open_gen(&GenState::new(kind));
    let f = src.read_video_frame(Tick(0), Some(Quality { width: 160, height: 90 })).expect("a frame");
    assert!(f.rgba.chunks_exact(4).all(|px| px[3] == 255), "the placeholder frame is opaque");
    assert!(f.rgba.chunks_exact(4).any(|px| px[..3] != f.rgba[..3]), "the placeholder frame carries its label");
    assert!(!src.info().has_audio, "a missing kind is silent");
}
