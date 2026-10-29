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
#[allow(dead_code)]
enum Frozen {
    Placeholder,
}

/// The moved kinds, each with its default and a non-default value: (the slot, its frozen form).
fn moved_kinds() -> Vec<(FxInstance, String)> {
    Vec::new()
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
