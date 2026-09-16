//! Тест 149 (п.149 porting-notes, Р.24): конформанс с ЭТАЛОННОЙ
//! python-библиотекой OpenTimelineIO (официальной JSON Schema у формата нет —
//! норматив только реализация).
//!
//! Gated по образцу ffmpeg-конформанса: нет `python3 -c "import
//! opentimelineio"` → тест печатает причину и выходит зелёным;
//! KT_REQUIRE_OTIO_CONFORMANCE=1 (CI) — отсутствие эталона = провал.
//! Сверяется СЕМАНТИКА (модель/поля), не байты python-вывода.

use kt_model::*;
use kt_otio::{Retained, load_project, save_project};
use kt_time::Tick;
use std::process::Command;

fn python_otio() -> bool {
    Command::new("python3")
        .args(["-c", "import opentimelineio"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn gate() -> bool {
    if python_otio() {
        return true;
    }
    if std::env::var_os("KT_REQUIRE_OTIO_CONFORMANCE").is_some() {
        panic!("reference library required: pip install opentimelineio");
    }
    eprintln!("skipped: python3 -c 'import opentimelineio' failed (pip install opentimelineio)");
    false
}

fn sample() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "media/a.mp4".into() });
    p.media.push(MediaRef { id: MediaId(2), path: "media/b.mp4".into() });
    for (id, start, src, media) in [(1u64, 0i64, 0i64, 1u64), (2, 90_000, 30_000, 2)] {
        p.clips.push(ClipEvent {
            id: ClipId(id),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick(start),
            duration: Tick(90_000),
            source_in: Tick(src),
            media: MediaId(media),
            gain_db: if id == 1 { -3.0 } else { 0.0 },
            reversed: id == 2,
            fx: if id == 1 { vec![FxInstance::gain(1.5)] } else { vec![] },
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick(9_000),
        out_offset: Tick(4_500),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.markers.push(Marker {
        t: Tick(15_000),
        duration: Tick(30_000),
        name: "range".into(),
        color: None,
    });
    p.region = Some(Region { start: Tick(10_000), end: Tick(60_000) });
    p.loop_enabled = true;
    p.normalize();
    p.validate().unwrap();
    p
}

/// (а) Наш файл переживает круг через эталон: read_from_file → write_to_file
/// эталоном, затем наш ридер обязан восстановить ЭКВИВАЛЕНТНУЮ модель
/// (metadata.kt эталон обязан сохранить — это и есть контракт passthrough).
#[test]
fn t149_our_file_survives_reference_roundtrip() {
    if !gate() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("kt-otio-t149a-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let ours = dir.join("ours.otio");
    let back = dir.join("back.otio");
    let p = sample();
    std::fs::write(&ours, save_project(&p, &Retained::default(), Some(&dir))).unwrap();

    let script = format!(
        r#"import opentimelineio as otio
t = otio.adapters.read_from_file('{}')
otio.adapters.write_to_file(t, '{}')
"#,
        ours.display(),
        back.display()
    );
    let out = Command::new("python3").args(["-c", &script]).output().unwrap();
    assert!(
        out.status.success(),
        "reference library failed to read our file:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );

    let text = std::fs::read_to_string(&back).unwrap();
    let bundle = load_project(&text, Some(&dir)).expect("we read the reference rewrite");
    assert_eq!(bundle.project.clips, p.clips, "clips survive the reference round-trip");
    assert_eq!(bundle.project.transitions, p.transitions);
    assert_eq!(bundle.project.region, p.region);
    assert!(bundle.project.loop_enabled);
    let m = &bundle.project.markers[0];
    assert_eq!((m.t, m.duration, m.name.as_str()), (Tick(15_000), Tick(30_000), "range"));
    std::fs::remove_dir_all(&dir).ok();
}

/// (б) Таймлайн, построенный ЭТАЛОННЫМ API, читается нами; наш пересейв
/// эталон читает обратно и сверяет числа сам (python-ассерты).
#[test]
fn t149_reference_built_timeline_reads_and_resaves() {
    if !gate() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("kt-otio-t149b-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let theirs = dir.join("theirs.otio");
    let resaved = dir.join("resaved.otio");

    let build = format!(
        r#"import opentimelineio as otio
from opentimelineio.opentime import RationalTime, TimeRange
tl = otio.schema.Timeline(name='ref cut')
tr = otio.schema.Track(name='V', kind='Video')
def clip(name, url, start, dur):
    c = otio.schema.Clip(name=name)
    c.media_reference = otio.schema.ExternalReference(target_url=url)
    c.source_range = TimeRange(RationalTime(start, 24), RationalTime(dur, 24))
    return c
tr.append(clip('a', 'shots/a.mov', 12, 48))
tr.append(otio.schema.Transition(transition_type='SMPTE_Dissolve',
    in_offset=RationalTime(6, 24), out_offset=RationalTime(6, 24)))
tr.append(clip('b', 'shots/b.mov', 0, 36))
tl.tracks.append(tr)
tl.tracks.markers.append(otio.schema.Marker(name='mk',
    marked_range=TimeRange(RationalTime(0, 24), RationalTime(12, 24))))
otio.adapters.write_to_file(tl, '{}')
"#,
        theirs.display()
    );
    let out = Command::new("python3").args(["-c", &build]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));

    let text = std::fs::read_to_string(&theirs).unwrap();
    let bundle = load_project(&text, Some(&dir)).expect("we read a reference-built file");
    let p = &bundle.project;
    assert_eq!(p.clips.len(), 2);
    assert_eq!(p.transitions.len(), 1, "reference transition imported");
    assert_eq!(p.transitions[0].in_offset, Tick(6 * 90_000 / 24));
    assert_eq!(p.markers.len(), 1);
    std::fs::write(&resaved, save_project(p, &bundle.retained, Some(&dir))).unwrap();

    let verify = format!(
        r#"import opentimelineio as otio
t = otio.adapters.read_from_file('{}')
tr = t.tracks[0]
clips = [c for c in tr if isinstance(c, otio.schema.Clip)]
trans = [c for c in tr if isinstance(c, otio.schema.Transition)]
assert len(clips) == 2, clips
assert len(trans) == 1, trans
assert abs(trans[0].in_offset.rescaled_to(24).value - 6) < 1e-6
assert clips[0].source_range.duration.rescaled_to(24).value == 48
assert t.tracks.markers[0].name == 'mk'
"#,
        resaved.display()
    );
    let out = Command::new("python3").args(["-c", &verify]).output().unwrap();
    assert!(
        out.status.success(),
        "reference library rejects our resave:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::remove_dir_all(&dir).ok();
}
