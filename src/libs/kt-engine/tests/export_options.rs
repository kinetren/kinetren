//! Тест 25 (docs/porting-notes.md, п.5): выбор энкодера и roundtrip mp4.
//! Требует системный FFmpeg (муксер kt-ffmpeg) — как conformance-тест 10.

use kt_engine::export::{ExportOptions, export_mp4_with};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::Tick;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

fn tiny_project() -> Project {
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 320, height: 180, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    for (id, kind, lane) in [(1u64, ClipKind::Video, 0u32), (2, ClipKind::Audio, 0)] {
        p.clips.push(ClipEvent {
            id: ClipId(id),
            link_group: Some(GroupId(1)),
            kind,
            lane,
            start: Tick(0),
            duration: Tick(90_000), // 1 c
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
    }
    p.validate().unwrap();
    p
}

fn sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    vec![(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 10), AudioPattern::Sine { hz: 440.0 }))
            as Box<dyn MediaSource>,
    )]
}

#[test]
fn t25_force_libx264_and_roundtrip_via_own_source() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("t25.mp4").to_string_lossy().to_string();
    let opts = ExportOptions {
        force_encoder: Some("libx264".into()),
        quality: 28,
        video_bitrate: None,
        audio_bitrate: 128_000,
        resolution: None,
    };
    let stats = export_mp4_with(
        &tiny_project(),
        sources(),
        sources(),
        &out,
        None,
        &opts,
        &Arc::new(AtomicBool::new(false)),
        &Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .expect("экспорт с force libx264");
    assert_eq!(stats.encoder, "libx264", "force-энкодер обязан быть выбран");
    assert_eq!(stats.video_frames, 30);

    // roundtrip НАШИМ ЖЕ декодером: длительность и потоки сходятся
    let mut src = kt_media::ffmpeg::FfmpegSource::open(&out).expect("открытие результата");
    let info = src.info().clone();
    assert!(info.has_video && info.has_audio);
    let dur_s = info.duration.0 as f64 / 90_000.0;
    assert!((dur_s - 1.0).abs() < 0.15, "длительность ~1 c, вышло {dur_s:.3}");
    let f = src
        .read_video_frame(Tick(0), Some(Quality { width: 320, height: 180 }))
        .expect("кадр 0 читается");
    assert_eq!((f.width, f.height), (320, 180));
    let b = src.read_audio_burst(0).expect("аудио читается");
    let energy: f32 = b.data.iter().map(|s| s * s).sum();
    assert!(energy > 1e-3, "аудио не тишина");
}

#[test]
fn t25_force_unknown_encoder_is_explicit_error() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("bad.mp4").to_string_lossy().to_string();
    let opts =
        ExportOptions { force_encoder: Some("no_such_encoder".into()), ..Default::default() };
    let err = export_mp4_with(
        &tiny_project(),
        sources(),
        sources(),
        &out,
        None,
        &opts,
        &Arc::new(AtomicBool::new(false)),
        &Arc::new(AtomicBool::new(false)),
        |_| {},
    );
    assert!(err.is_err(), "неизвестный force-энкодер = явная ошибка, не фолбэк");
}

#[test]
fn t25_auto_encoder_selects_something() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("auto.mp4").to_string_lossy().to_string();
    let stats = export_mp4_with(
        &tiny_project(),
        sources(),
        sources(),
        &out,
        None,
        &ExportOptions::default(),
        &Arc::new(AtomicBool::new(false)),
        &Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .expect("экспорт с автовыбором");
    assert!(!stats.encoder.is_empty(), "имя выбранного энкодера должно быть видно (UI)");
}

/// Тест 33 (п.9): в файле РЕАЛЬНО есть все кадры — дроп любого (EAGAIN
/// без ретрая в муксере) — провал. Считаем кадры собственным декодером.
#[test]
fn t33_exported_file_contains_every_frame() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("t33.mp4").to_string_lossy().to_string();
    // 2 секунды = 60 кадров при 30 fps
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 320, height: 180, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    for (id, kind, lane) in [(1u64, ClipKind::Video, 0u32), (2, ClipKind::Audio, 0)] {
        p.clips.push(ClipEvent {
            id: ClipId(id),
            link_group: Some(GroupId(1)),
            kind,
            lane,
            start: Tick(0),
            duration: Tick(180_000),
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
    }
    p.validate().unwrap();
    let stats = export_mp4_with(
        &p,
        sources(),
        sources(),
        &out,
        None,
        &ExportOptions { force_encoder: Some("libx264".into()), ..Default::default() },
        &Arc::new(AtomicBool::new(false)),
        &Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .expect("экспорт");
    assert_eq!(stats.video_frames, 60, "движок отдал 60 кадров");

    // считаем кадры В ФАЙЛЕ: span по всем ключам + frames_decoded
    let mut src = kt_media::ffmpeg::FfmpegSource::open(&out).unwrap();
    let step = 90_000 / 30;
    let keys: Vec<Tick> = (0..60).map(|i| Tick(i * step)).collect();
    let mut emitted = 0usize;
    src.read_video_span(&keys, Some(Quality { width: 320, height: 180 }), &mut |_, _| emitted += 1)
        .expect("чтение всех ключей");
    assert_eq!(emitted, 60, "каждый ключ сетки закрыт кадром");
    let (_reopens, decoded) = src.decode_debug_stats();
    assert!(
        decoded >= 60,
        "файл обязан содержать все 60 кадров (декодировано {decoded}) — дроп в муксере"
    );
}
