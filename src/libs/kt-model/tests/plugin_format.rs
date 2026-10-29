//! t973 (plugin-api-stage1, D3; capability `plugins`): moving a kind into a plugin does not change a
//! byte of the project file, and an effect this build does not register is kept and written back.
//!
//! The FROZEN ORACLE below is a verbatim copy of the derive of every kind moved so far (the t643
//! technique): the build after the move must write exactly the text the build before wrote.

use kt_model::project_file::{from_text, to_text};
use kt_model::*;
use kt_time::Tick;
use serde::Serialize;

/// Today's derive output of the moved kinds — copied from `FxInstance` before each move, never
/// edited afterwards. Stage 2.4 adds `posterize`; each later stage adds its kinds.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Frozen {
    /// `FxInstance::Posterize` before 2.4 (`#[serde(default = "default_levels")] levels: u32`).
    Posterize { levels: u32 },
    /// `FxInstance::StereoWidth` before stage 3 (`#[serde(default = "default_one_f32")] width: f32`).
    StereoWidth { width: f32 },
    /// The three below before stage 3, field for field (`skip_serializing_if` copied as is).
    Eq {
        #[serde(skip_serializing_if = "is_zero")]
        low_gain_db: f32,
        low_freq_hz: f32,
        #[serde(skip_serializing_if = "is_zero")]
        mid_gain_db: f32,
        mid_freq_hz: f32,
        #[serde(skip_serializing_if = "is_one")]
        mid_q: f32,
        #[serde(skip_serializing_if = "is_zero")]
        high_gain_db: f32,
        high_freq_hz: f32,
    },
    Distortion {
        drive_db: f32,
        tone: f32,
        #[serde(skip_serializing_if = "is_one")]
        mix: f32,
    },
    Bitcrusher {
        bits: f32,
        rate_hz: f32,
        #[serde(skip_serializing_if = "is_one")]
        mix: f32,
    },
}

fn is_zero(v: &f32) -> bool {
    *v == 0.0
}

fn is_one(v: &f32) -> bool {
    *v == 1.0
}

/// A file that leaves every field out read the variant's serde defaults; the plugin reads the same.
fn empty_reads() -> Vec<(&'static str, String)> {
    vec![
        (r#"{"kind":"eq"}"#, frozen(Frozen::Eq {
            low_gain_db: 0.0, low_freq_hz: 200.0, mid_gain_db: 0.0, mid_freq_hz: 1000.0,
            mid_q: 1.0, high_gain_db: 0.0, high_freq_hz: 4000.0,
        })),
        (r#"{"kind":"distortion"}"#, frozen(Frozen::Distortion { drive_db: 12.0, tone: 0.5, mix: 1.0 })),
        (r#"{"kind":"bitcrusher"}"#, frozen(Frozen::Bitcrusher { bits: 8.0, rate_hz: 12000.0, mix: 1.0 })),
        (r#"{"kind":"stereo_width"}"#, frozen(Frozen::StereoWidth { width: 1.0 })),
    ]
}

fn frozen(f: Frozen) -> String {
    serde_json::to_string(&f).unwrap()
}

/// The moved kinds, each with its default and a non-default value: (the slot, its frozen form).
// values with more digits than an f32 holds are the point: the file must keep the f32's form
#[allow(clippy::excessive_precision)]
fn moved_kinds() -> Vec<(FxInstance, String)> {
    kt_plugins_builtin::install();
    vec![
        (FxInstance::posterize(), frozen(Frozen::Posterize { levels: 6 })),
        (FxInstance::plugin_with("posterize", &[4.0]).unwrap(), frozen(Frozen::Posterize { levels: 4 })),
        (FxInstance::plugin_with("posterize", &[32.0]).unwrap(), frozen(Frozen::Posterize { levels: 32 })),
        (FxInstance::stereo_width(), frozen(Frozen::StereoWidth { width: 1.0 })),
        (FxInstance::eq(), frozen(Frozen::Eq {
            low_gain_db: 0.0, low_freq_hz: 200.0, mid_gain_db: 0.0, mid_freq_hz: 1000.0,
            mid_q: 1.0, high_gain_db: 0.0, high_freq_hz: 4000.0,
        })),
        (
            FxInstance::plugin_with("eq", &[-6.5, 120.0, 3.25, 2500.0, 0.7, 0.123456789, 8000.0]).unwrap(),
            frozen(Frozen::Eq {
                low_gain_db: -6.5, low_freq_hz: 120.0, mid_gain_db: 3.25, mid_freq_hz: 2500.0,
                mid_q: 0.7, high_gain_db: 0.123456789, high_freq_hz: 8000.0,
            }),
        ),
        (FxInstance::distortion(), frozen(Frozen::Distortion { drive_db: 12.0, tone: 0.5, mix: 1.0 })),
        (
            FxInstance::plugin_with("distortion", &[30.0, 0.25, 0.6]).unwrap(),
            frozen(Frozen::Distortion { drive_db: 30.0, tone: 0.25, mix: 0.6 }),
        ),
        (FxInstance::bitcrusher(), frozen(Frozen::Bitcrusher { bits: 8.0, rate_hz: 12000.0, mix: 1.0 })),
        (
            FxInstance::plugin_with("bitcrusher", &[3.5, 4410.0, 0.5]).unwrap(),
            frozen(Frozen::Bitcrusher { bits: 3.5, rate_hz: 4410.0, mix: 0.5 }),
        ),
        (FxInstance::plugin_with("stereo_width", &[0.0]).unwrap(), frozen(Frozen::StereoWidth { width: 0.0 })),
        // more digits than an f32 holds (a dragged or automated value): the variant's f32 field
        // wrote "0.12345679"; an f64 row would write "0.123456789" (red proof of the row's Store)
        (
            FxInstance::plugin_with("stereo_width", &[0.123456789]).unwrap(),
            frozen(Frozen::StereoWidth { width: 0.123456789 }),
        ),
    ]
}

/// The frozen variant also READ files that left out a defaulted field: the plugin reads them the same.
#[test]
fn t973_a_moved_kind_reads_the_old_defaults() {
    kt_plugins_builtin::install();
    for (file, want) in empty_reads() {
        let read: FxInstance = serde_json::from_str(file).unwrap();
        assert_eq!(serde_json::to_string(&read).unwrap(), want, "{file} does not read the variant's defaults");
    }
    let read: FxInstance = serde_json::from_str(r#"{"kind":"posterize"}"#).unwrap();
    // against the FROZEN default (6), not `posterize()`: a moved default would agree with itself
    assert_eq!(serde_json::to_string(&read).unwrap(), frozen(Frozen::Posterize { levels: 6 }), "a posterize without `levels` reads as the default 6");
    let clamped: FxInstance = serde_json::from_str(r#"{"kind":"posterize","levels":99}"#).unwrap();
    assert_eq!(
        serde_json::to_string(&clamped).unwrap(),
        r#"{"kind":"posterize","levels":99}"#,
        "an out-of-range value in a file is kept as read, as the variant kept it"
    );
}

fn project_with(fx: Vec<FxInstance>) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:ramp"));
    let n = fx.len();
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
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx,
        fx_bypass: Vec::new(),
        fx_auto: vec![Vec::new(); n],
        opacity: 1.0,
        fx_off: false,
    });
    p
}

#[test]
fn t973_moving_a_kind_does_not_change_a_byte() {
    kt_plugins_builtin::install();
    for (slot, frozen) in moved_kinds() {
        let ours = serde_json::to_string(&slot).unwrap();
        assert_eq!(ours, frozen, "{}: the plugin slot writes other text than the variant did", slot.tag());
        // and it reads back into the same slot
        let back: FxInstance = serde_json::from_str(&frozen).unwrap();
        assert_eq!(back, slot, "{}: the variant's text reads into another slot", slot.tag());
    }
    // the whole file: written, read and written again — byte-identical
    let fx: Vec<FxInstance> = moved_kinds().into_iter().map(|(s, _)| s).collect();
    let text = to_text(&project_with(fx), None);
    let again = to_text(&from_text(&text).expect("reads").project, None);
    assert_eq!(text, again, "the second write differs from the first");
}

/// A chain `[blur, wobble_x, glow]` where `wobble_x` is not registered: the file opens, the slot
/// is `Missing` with its raw content, and the save writes the same bytes.
#[test]
fn t973_a_missing_plugin_keeps_its_bytes() {
    kt_plugins_builtin::install();
    let wobble = r#"{"kind":"wobble_x","amp":0.25,"freq":3,"mode":"sine"}"#;
    let fx = vec![
        FxInstance::blur(4),
        serde_json::from_str(wobble).expect("an unknown kind still reads"),
        FxInstance::glow(0.5),
    ];
    assert!(matches!(&fx[1], FxInstance::Missing(m) if m.id() == "wobble_x"), "{:?}", fx[1]);
    let text = to_text(&project_with(fx), None);
    assert!(text.contains("\"kind\": \"wobble_x\""), "the missing slot was not written: {text}");
    let read = from_text(&text).expect("a file with a missing plugin opens");
    let FxInstance::Missing(m) = &read.project.clips[0].fx[1] else { panic!("not kept as missing") };
    assert_eq!(serde_json::to_string(&m.raw).unwrap(), wobble, "the raw content changed");
    assert_eq!(to_text(&read.project, None), text, "the save changed the bytes");
}
