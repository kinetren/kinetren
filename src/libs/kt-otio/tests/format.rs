//! Тесты 144-147 (пп.144-147 porting-notes, Р.24): формат-слой kt-otio.
//!
//! 144 — типизированный round-trip и ИДЕМПОТЕНТНОСТЬ записи;
//! 145 — passthrough чужого (поля, схемы, metadata-неймспейсы);
//! 146 — версии схем (upgrade Clip.1, отказы хребта, kt.v);
//! 147 — пути/URL (relativize, file://, synth:).

use kt_otio::{
    Clip, Composable, EffectNode, Error, FileFormat, Gap, LinearTimeWarp, Marker, MarkerNode,
    MediaRefNode, RationalTime, Stack, TimeRange, Timeline, Track, Transition, detect,
    parse_document, write_document,
};
use kt_time::Tick;
use serde_json::{Map, Value, json};
use std::path::Path;

fn rt(ticks: i64) -> RationalTime {
    RationalTime::from_ticks(Tick(ticks))
}

fn range(start: i64, dur: i64) -> TimeRange {
    TimeRange { start_time: rt(start), duration: rt(dur) }
}

fn clip(name: &str, url: &str, source_in: i64, dur: i64) -> Composable {
    let mut refs = indexmap::IndexMap::new();
    refs.insert(
        kt_otio::DEFAULT_MEDIA_KEY.to_string(),
        Some(MediaRefNode::External(kt_otio::ExternalReference {
            name: String::new(),
            target_url: url.into(),
            available_range: None,
            available_image_bounds: None,
            metadata: Map::new(),
            extra: Map::new(),
        })),
    );
    Composable::Clip(Clip {
        name: name.into(),
        source_range: Some(range(source_in, dur)),
        media_references: refs,
        active_media_reference_key: kt_otio::DEFAULT_MEDIA_KEY.into(),
        effects: vec![],
        markers: vec![],
        enabled: true,
        metadata: Map::new(),
        extra: Map::new(),
    })
}

fn gap(dur: i64) -> Composable {
    Composable::Gap(Gap {
        name: String::new(),
        source_range: Some(range(0, dur)),
        effects: vec![],
        markers: vec![],
        enabled: true,
        metadata: Map::new(),
        extra: Map::new(),
    })
}

/// Богатый документ: дорожки обоих видов, gap, переход, метка-диапазон,
/// эффекты (в т.ч. LinearTimeWarp −1), metadata.kt.
fn rich_timeline() -> Timeline {
    let mut video = Track {
        name: "MAIN".into(),
        kind: "Video".into(),
        children: vec![clip("a", "media/a.mp4", 0, 90_000), gap(45_000), clip("b", "media/b.mp4", 9_000, 90_000)],
        source_range: None,
        effects: vec![],
        markers: vec![],
        enabled: true,
        metadata: Map::new(),
        extra: Map::new(),
    };
    if let Composable::Clip(c) = &mut video.children[2] {
        c.effects.push(EffectNode::LinearTimeWarp(LinearTimeWarp {
            name: String::new(),
            effect_name: "LinearTimeWarp".into(),
            time_scalar: -1.0,
            enabled: true,
            metadata: Map::new(),
            extra: Map::new(),
        }));
    }
    let audio = Track {
        name: "DIALOG".into(),
        kind: "Audio".into(),
        children: vec![
            clip("x", "media/a.mp4", 0, 90_000),
            Composable::Transition(Transition {
                name: String::new(),
                transition_type: "SMPTE_Dissolve".into(),
                in_offset: rt(9_000),
                out_offset: rt(4_500),
                metadata: Map::new(),
                extra: Map::new(),
            }),
            clip("y", "media/b.mp4", 18_000, 90_000),
        ],
        source_range: None,
        effects: vec![],
        markers: vec![],
        enabled: true,
        metadata: Map::new(),
        extra: Map::new(),
    };
    let mut stack_meta = Map::new();
    stack_meta.insert("kt".into(), json!({"v": 1}));
    let tracks = Stack {
        name: "tracks".into(),
        children: vec![Composable::Track(video), Composable::Track(audio)],
        source_range: None,
        effects: vec![],
        markers: vec![MarkerNode::Marker(Marker {
            name: "chorus".into(),
            color: Marker::default_color(),
            marked_range: range(90_000, 180_000),
            comment: String::new(),
            metadata: Map::new(),
            extra: Map::new(),
        })],
        enabled: true,
        metadata: Map::new(),
        extra: Map::new(),
    };
    let mut meta = Map::new();
    meta.insert("kt".into(), json!({"v": 1, "settings": {"fps": 30, "width": 1280, "height": 720, "sample_rate": 48000}}));
    Timeline {
        name: "demo".into(),
        global_start_time: None,
        metadata: meta,
        tracks,
        extra: Map::new(),
    }
}

/// Тест 144: round-trip типов и идемпотентность записи.
#[test]
fn t144_roundtrip_is_typed_and_idempotent() {
    let tl = rich_timeline();
    let s1 = write_document(&tl);
    let parsed = parse_document(&s1).expect("own document parses");
    assert_eq!(parsed, tl, "typed round-trip must be lossless");
    let s2 = write_document(&parsed);
    assert_eq!(s1, s2, "write(parse(write(x))) must equal write(x)");

    // детект по содержимому + базовые свойства формата
    assert_eq!(detect(&s1).unwrap(), FileFormat::Otio);
    assert!(s1.starts_with("{\n    \"OTIO_SCHEMA\": \"Timeline.1\","), "tag first, 4-space indent");
    assert!(s1.ends_with('\n'));
    // тики пишутся как f64 с точным целым значением
    assert!(s1.contains("\"rate\": 90000.0"), "rate must be 90000.0");
    assert!(s1.contains("\"value\": 90000.0"));

    // чужой документ с иным форматированием: после первой записи — идемпотентно
    let foreign = r#"{"OTIO_SCHEMA":"Timeline.1","name":"x","tracks":{"OTIO_SCHEMA":"Stack.1","children":[{"OTIO_SCHEMA":"Track.1","kind":"Video","children":[{"OTIO_SCHEMA":"Gap.1","source_range":{"OTIO_SCHEMA":"TimeRange.1","start_time":{"OTIO_SCHEMA":"RationalTime.1","value":0,"rate":24},"duration":{"OTIO_SCHEMA":"RationalTime.1","value":48.00,"rate":24}}}]}]}}"#;
    let d1 = parse_document(foreign).expect("foreign minimal file parses");
    let w1 = write_document(&d1);
    let w2 = write_document(&parse_document(&w1).unwrap());
    assert_eq!(w1, w2, "idempotency after first normalization");
}

/// Тест 145: passthrough — неизвестные поля, схемы и metadata-неймспейсы.
#[test]
fn t145_passthrough_preserves_foreign_data() {
    let text = json!({
        "OTIO_SCHEMA": "Timeline.1",
        "name": "f",
        "vendor_field": {"deep": [1, 2, {"x": true}]},
        "metadata": {"resolve": {"colorVersion": 3}, "fcp_xml": "keep"},
        "tracks": {
            "OTIO_SCHEMA": "Stack.1",
            "children": [{
                "OTIO_SCHEMA": "Track.1",
                "kind": "Video",
                "children": [
                    {
                        "OTIO_SCHEMA": "Clip.2",
                        "name": "known",
                        "custom_clip_field": 42,
                        "source_range": {"OTIO_SCHEMA": "TimeRange.1",
                            "start_time": {"OTIO_SCHEMA": "RationalTime.1", "value": 0.0, "rate": 90000.0},
                            "duration": {"OTIO_SCHEMA": "RationalTime.1", "value": 900.0, "rate": 90000.0}},
                        "media_references": {"DEFAULT_MEDIA": {
                            "OTIO_SCHEMA": "ExternalReference.1", "target_url": "a.mov",
                            "available_range": null}},
                        "active_media_reference_key": "DEFAULT_MEDIA"
                    },
                    {
                        "OTIO_SCHEMA": "FancyClip.7",
                        "zeta": 1,
                        "alpha": 2,
                        "source_range": {"OTIO_SCHEMA": "TimeRange.1",
                            "start_time": {"OTIO_SCHEMA": "RationalTime.1", "value": 0.0, "rate": 24.0},
                            "duration": {"OTIO_SCHEMA": "RationalTime.1", "value": 24.0, "rate": 24.0}}
                    }
                ]
            }]
        }
    })
    .to_string();

    let tl = parse_document(&text).expect("parses");
    let out = write_document(&tl);
    let reparsed = parse_document(&out).unwrap();
    assert_eq!(tl, reparsed);

    // неизвестное поле Timeline пережило round-trip
    assert_eq!(tl.extra.get("vendor_field").unwrap()["deep"][2]["x"], Value::Bool(true));
    // чужие metadata-неймспейсы verbatim
    assert_eq!(tl.metadata.get("resolve").unwrap()["colorVersion"], json!(3));
    assert!(out.contains("\"fcp_xml\": \"keep\""));

    // неизвестная схема — сырьё verbatim, порядок ЕЁ ключей исходный
    let Composable::Track(track) = &tl.tracks.children[0] else { panic!("track expected") };
    let Composable::Unknown(raw) = &track.children[1] else { panic!("unknown expected") };
    assert_eq!(raw.schema_tag(), Some("FancyClip.7"));
    let keys: Vec<&str> = raw.0.keys().map(String::as_str).collect();
    assert_eq!(keys, ["OTIO_SCHEMA", "zeta", "alpha", "source_range"], "raw key order kept");
    // и в выводе zeta стоит раньше alpha (как в источнике)
    let zeta = out.find("\"zeta\"").unwrap();
    let alpha = out.find("\"alpha\"").unwrap();
    assert!(zeta < alpha);

    // неизвестное поле известного Clip
    let Composable::Clip(known) = &track.children[0] else { panic!("clip expected") };
    assert_eq!(known.extra.get("custom_clip_field"), Some(&json!(42)));
    assert!(out.contains("\"custom_clip_field\": 42"));
}

/// Тест 146: политика версий схем.
#[test]
fn t146_schema_versions() {
    // Clip.1 → upgrade до Clip.2 (media_reference → media_references)
    let text = json!({
        "OTIO_SCHEMA": "Timeline.1",
        "tracks": {"OTIO_SCHEMA": "Stack.1", "children": [{
            "OTIO_SCHEMA": "Track.1", "kind": "Video", "children": [{
                "OTIO_SCHEMA": "Clip.1",
                "name": "old",
                "source_range": {"OTIO_SCHEMA": "TimeRange.1",
                    "start_time": {"OTIO_SCHEMA": "RationalTime.1", "value": 0.0, "rate": 90000.0},
                    "duration": {"OTIO_SCHEMA": "RationalTime.1", "value": 900.0, "rate": 90000.0}},
                "media_reference": {"OTIO_SCHEMA": "ExternalReference.1", "target_url": "x.mov",
                    "available_range": null}
            }]
        }]}
    })
    .to_string();
    let tl = parse_document(&text).unwrap();
    let Composable::Track(track) = &tl.tracks.children[0] else { panic!() };
    let Composable::Clip(c) = &track.children[0] else { panic!("Clip.1 must upgrade to typed Clip") };
    assert_eq!(c.active_media_reference_key, "DEFAULT_MEDIA");
    let Some(MediaRefNode::External(er)) = c.active_media() else { panic!() };
    assert_eq!(er.target_url, "x.mov");
    // и пишется уже как Clip.2
    assert!(write_document(&tl).contains("\"OTIO_SCHEMA\": \"Clip.2\""));

    // лист новее нашей версии → passthrough
    let leaf = text.replace("Clip.1", "Clip.99").replace("media_reference", "media_referencesX");
    let tl2 = parse_document(&leaf).unwrap();
    let Composable::Track(track2) = &tl2.tracks.children[0] else { panic!() };
    assert!(matches!(&track2.children[0], Composable::Unknown(r) if r.schema_tag() == Some("Clip.99")));

    // хребет новее — отказ
    let bad_track = text.replace("Track.1", "Track.2");
    assert!(matches!(parse_document(&bad_track), Err(Error::UnsupportedSchema(s)) if s == "Track.2"));
    let bad_tl = text.replace("Timeline.1", "Timeline.3");
    assert!(matches!(parse_document(&bad_tl), Err(Error::UnsupportedSchema(_))));

    // корень не Timeline — отказ
    let coll = r#"{"OTIO_SCHEMA": "SerializableCollection.1", "children": []}"#;
    assert!(matches!(parse_document(coll), Err(Error::UnsupportedRoot(_))));

    // kt.v новее нашей — гейт
    let mut tl3 = rich_timeline();
    tl3.metadata.insert("kt".into(), json!({"v": 999}));
    assert!(matches!(kt_otio::check_kt_version(&tl3), Err(Error::KtMetaTooNew(999))));
    assert_eq!(kt_otio::kt_version(&rich_timeline()), Some(1));

    // детект легаси
    assert_eq!(detect(r#"{"format_version": 1, "clips": []}"#).unwrap(), FileFormat::LegacyKtproj);
}

/// Тест 147: пути и file://-URL.
#[test]
fn t147_media_urls() {
    use kt_otio::{decode_file_url, encode_file_url, media_target_url, relativize, resolve_media_path};
    let base = Path::new("/home/user/projects/film");

    // relativize: рядом, глубже, выше
    assert_eq!(relativize(base, Path::new("/home/user/projects/film/media/a.mp4")).unwrap(), "media/a.mp4");
    assert_eq!(relativize(base, Path::new("/home/user/projects/other/b.mp4")).unwrap(), "../other/b.mp4");
    assert_eq!(relativize(base, Path::new("/home/user/projects/film")).unwrap(), ".");
    // общего только корень — не релятивизуем (фолбэк на file://)
    assert_eq!(relativize(base, Path::new("/mnt/disk/c.mp4")), None);

    // file:// с пробелами/процентами/UTF-8
    let tricky = Path::new("/mnt/видео архив/100% cut.mp4");
    let url = encode_file_url(tricky);
    assert!(url.starts_with("file:///mnt/"));
    assert!(!url.contains(' ') && !url.contains("видео"), "non-ASCII and spaces are escaped");
    assert_eq!(decode_file_url(&url).unwrap(), tricky);
    assert_eq!(decode_file_url("file://localhost/a%20b").unwrap(), Path::new("/a b"));
    assert_eq!(decode_file_url("relative/path"), None);

    // media_target_url: относительный / фолбэк / synth
    assert_eq!(media_target_url(base, "/home/user/projects/film/media/a.mp4"), "media/a.mp4");
    assert_eq!(media_target_url(base, "/mnt/disk/c.mp4"), encode_file_url(Path::new("/mnt/disk/c.mp4")));
    assert_eq!(media_target_url(base, "synth:Ramp"), "synth:Ramp");
    assert_eq!(media_target_url(base, "already/relative.mp4"), "already/relative.mp4");

    // resolve: все три вида на входе
    assert_eq!(resolve_media_path(base, "media/a.mp4"), base.join("media/a.mp4"));
    assert_eq!(resolve_media_path(base, "/abs/x.mp4"), Path::new("/abs/x.mp4"));
    assert_eq!(resolve_media_path(base, &url), tricky);
    assert_eq!(resolve_media_path(base, "synth:Ramp"), Path::new("synth:Ramp"));
}
