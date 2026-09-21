//! Тест 341 (Р.48, п.341): автодакинг — музыка сама приглушается под
//! голосом.
//!
//! Проверяется вся цепочка: движок находит интервалы речи честным
//! рендером дорожки-источника, команда модели кладёт по ним кривую на
//! Gain музыки, звук в интервале действительно тише на заданную глубину,
//! а вне его — прежний; всё это ОДИН шаг undo и детерминированно.

use kt_engine::ducking::{DuckParams, duck_spans};
use kt_engine::{AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const SEC: i64 = 48_000;

/// Голос на дорожке 0 — два «слова» по 0.6 с (2.0..2.6 и 3.2..3.8);
/// музыка на дорожке 1 — ровный синус на все 6 секунд.
fn project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:voice".into() });
    p.media.push(MediaRef { id: MediaId(2), path: "synth:music".into() });
    let clip = |id: u64, lane: u32, media: u64, from_s: f64, len_s: f64| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Audio,
        lane,
        start: Tick::from_audio_frames((from_s * SEC as f64) as i64),
        duration: Tick::from_audio_frames((len_s * SEC as f64) as i64),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    p.clips.push(clip(1, 0, 1, 2.0, 0.6));
    p.clips.push(clip(2, 0, 1, 3.2, 0.6));
    p.clips.push(clip(3, 1, 2, 0.0, 6.0));
    p.validate().unwrap();
    p
}

fn fetcher() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    // «голос» — громкий тон, «музыка» — тихий (чтобы дакинг был виден)
    f.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: 300.0 })),
    );
    f.add_source(
        MediaId(2),
        Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: 1000.0 })),
    );
    f
}

/// RMS музыкальной дорожки (в проекте без голоса, чтобы мерить только
/// её) в окне [from_s, to_s).
fn music_rms(p: &Project, from_s: f64, to_s: f64) -> f64 {
    let mut solo = p.clone();
    solo.clips.retain(|c| c.lane == 1);
    let r = AudioRenderer::new(&solo);
    let mut f = fetcher();
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let from = (from_s * SEC as f64) as i64;
    let n = ((to_s - from_s) * SEC as f64) as usize;
    let mut out = vec![0f32; n * CH];
    r.render_frames_with(&mut f, &mut store, from, &mut out);
    (out.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / out.len() as f64).sqrt()
}

#[test]
fn t341_ducking_finds_speech_and_lowers_the_music() {
    let p = project();
    let params = DuckParams::default();
    let spans = duck_spans(&p, &mut fetcher(), 0, params);
    let as_sec: Vec<(f64, f64)> =
        spans.iter().map(|s| (s.start as f64 / SEC as f64, s.end as f64 / SEC as f64)).collect();
    println!("t341 интервалы речи: {as_sec:?}");
    // два слова с паузой 0.6 с — она короче порога слияния (0.3 с?) нет,
    // длиннее: интервалы обязаны остаться раздельными
    assert_eq!(spans.len(), 2, "ожидались два интервала речи, получено {as_sec:?}");
    assert!((as_sec[0].0 - 2.0).abs() < 0.1, "первое слово начинается около 2.0 с");
    assert!((as_sec[1].1 - 3.8).abs() < 0.1, "второе слово кончается около 3.8 с");

    // детерминизм: тот же проект — те же интервалы
    assert_eq!(spans, duck_spans(&p, &mut fetcher(), 0, params), "анализ не детерминирован");

    // кривая на музыку: один батч = один шаг undo
    let cmds = kt_model::ops::duck_curve_commands(
        &p,
        &[ClipId(3)],
        &spans,
        params.depth_db,
        params.attack_ms,
        params.release_ms,
    );
    assert!(!cmds.is_empty(), "команда обязана что-то сделать");
    let mut m = TimelineModel::new(p.clone());
    let before = m.undo_depth();
    m.apply_batch(cmds).expect("apply");
    assert_eq!(m.undo_depth(), before + 1, "дакинг обязан быть ОДНИМ шагом истории");

    let after = m.project().clone();
    let music = after.clips.iter().find(|c| c.id == ClipId(3)).unwrap();
    let curve = music
        .fx_auto
        .iter()
        .flatten()
        .find(|c| c.param == "db")
        .expect("кривая громкости обязана появиться");
    println!("t341 ключей: {}", curve.keys.len());
    assert!(
        curve.keys.len() <= 4 * spans.len(),
        "не больше четырёх ключей на интервал, получено {}",
        curve.keys.len()
    );

    // звук: в середине слова музыка тише на ~depth, вне — прежняя
    let quiet = music_rms(&after, 2.2, 2.5);
    let loud = music_rms(&after, 0.5, 1.5);
    let base_quiet = music_rms(&p, 2.2, 2.5);
    let db = 20.0 * (quiet / base_quiet).log10();
    println!("t341 приглушение в слове: {db:.2} дБ (ждём -{:.1})", params.depth_db);
    assert!(
        (db + params.depth_db).abs() < 1.5,
        "музыка под голосом обязана просесть на {} дБ, а просела на {db:.2}",
        params.depth_db
    );
    let db_out = 20.0 * (loud / music_rms(&p, 0.5, 1.5)).log10();
    assert!(db_out.abs() < 0.5, "вне речи громкость обязана остаться прежней ({db_out:.2} дБ)");

    // undo возвращает модель как была
    m.undo().expect("undo");
    assert_eq!(m.project().clips, p.clips, "undo обязан вернуть модель в исходное состояние");
}
