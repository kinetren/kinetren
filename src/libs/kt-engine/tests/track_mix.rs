//! Р.62 (change B): стадия ДОРОЖКИ в аудио — уровень, mute и solo.
//!
//! Главное утверждение отрицательное: у дорожки БЕЗ свойств путь прежний
//! байт-в-байт (на нём стоят t2/t3 и GOLDEN_AUDIO), потому что множитель
//! дорожки складывается с gain события в одно число и порядок сложения
//! f32 не меняется ни на такт.

use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, db_to_linear};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::LaneKind::Audio as A;
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    }
}

fn audio_clip(id: u64, lane: u32, dur_f: i64, media: u64) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Audio,
        lane,
        start: Tick::ZERO,
        duration: Tick::from_audio_frames(dur_f),
        source_in: Tick::ZERO,
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn project(clips: Vec<ClipEvent>) -> Project {
    let mut p = Project::new(settings());
    let mut ids: Vec<MediaId> = clips.iter().map(|c| c.media).collect();
    ids.sort();
    ids.dedup();
    for id in ids {
        p.media.push(MediaRef { id, path: format!("synth:{}", id.0) });
    }
    p.clips = clips;
    p.normalize();
    p.validate().unwrap();
    p
}

fn direct(sources: &[(u64, AudioPattern)]) -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    for &(id, pat) in sources {
        f.add_source(MediaId(id), Box::new(SyntheticSource::new(Tick(90_000 * 60), pat)));
    }
    f
}

fn render(p: &Project, f: &mut dyn AudioFetcher, frames: i64) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let mut out = vec![0f32; frames as usize * CH];
    let s = r.render_frames(f, 0, &mut out);
    assert_eq!(s.underruns, 0, "underrun при полностью доступном источнике");
    out
}

/// Тест 732 (Р.62, change B): уровень дорожки, mute и solo.
#[test]
fn t732_track_level_mute_and_solo() {
    let d = AUDIO_BURST_FRAMES as i64;
    let two = || project(vec![audio_clip(1, 0, d, 1), audio_clip(2, 1, d, 2)]);
    let src =
        || direct(&[(1, AudioPattern::Dc { value: 0.25 }), (2, AudioPattern::Dc { value: 0.5 })]);

    // БЕЗ СВОЙСТВ — прежняя сумма без весов (то же утверждает t2)
    let base = render(&two(), &mut src(), d);
    assert_eq!(base[0], 0.75);

    // УРОВЕНЬ дорожки — ещё один множитель после весов и gain события
    let mut p = two();
    p.track_at_mut(A, 0).unwrap().gain_db = -6.0;
    let out = render(&p, &mut src(), d);
    assert_eq!(out[0], 0.25 * db_to_linear(-6.0) + 0.5, "уровень дорожки применён не точно");

    // …и он ПЕРЕМНОЖАЕТСЯ с gain события: −6 дБ у дорожки и +6 дБ у
    // события возвращают исходное в пределах округления f32
    let mut p = two();
    p.track_at_mut(A, 0).unwrap().gain_db = -6.0;
    p.clips[0].gain_db = 6.0;
    let out = render(&p, &mut src(), d);
    assert!((out[0] - 0.75).abs() < 1e-6, "−6 дБ дорожки и +6 дБ события не сошлись: {}", out[0]);

    // MUTE — тишина ДОРОЖКИ, соседняя звучит как раньше
    let mut p = two();
    p.track_at_mut(A, 0).unwrap().mute = true;
    let out = render(&p, &mut src(), d);
    assert_eq!(out[0], 0.5, "заглушённая дорожка обязана молчать");
    let mut p = two();
    p.track_at_mut(A, 1).unwrap().mute = true;
    assert_eq!(render(&p, &mut src(), d)[0], 0.25);

    // SOLO — множество СВОЕГО вида: звучат только соло-дорожки
    let three =
        || project(vec![audio_clip(1, 0, d, 1), audio_clip(2, 1, d, 2), audio_clip(3, 2, d, 3)]);
    let src3 = || {
        direct(&[
            (1, AudioPattern::Dc { value: 0.25 }),
            (2, AudioPattern::Dc { value: 0.5 }),
            (3, AudioPattern::Dc { value: 0.125 }),
        ])
    };
    assert_eq!(render(&three(), &mut src3(), d)[0], 0.875, "сцена: звучат все три");
    let mut p = three();
    p.track_at_mut(A, 0).unwrap().solo = true;
    p.track_at_mut(A, 2).unwrap().solo = true;
    let out = render(&p, &mut src3(), d);
    assert_eq!(out[0], 0.375, "в соло обязаны звучать ровно две дорожки");

    // СНЯТИЕ СОЛО возвращает ПРЕЖНИЕ mute, а не «все включены»
    let mut p = three();
    p.track_at_mut(A, 1).unwrap().mute = true;
    p.track_at_mut(A, 0).unwrap().solo = true;
    assert_eq!(render(&p, &mut src3(), d)[0], 0.25, "в соло звучит только соло-дорожка");
    p.track_at_mut(A, 0).unwrap().solo = false;
    assert_eq!(render(&p, &mut src3(), d)[0], 0.375, "снятие соло вернуло заглушённую дорожку");
}

/// Тест 732b: гашение — МНОЖИТЕЛЬ 0, а не выброс события из графа.
/// Событие заглушённой дорожки по-прежнему рендерится (его FX-состояние
/// живёт), поэтому снятие mute посреди хвоста реверба продолжает хвост, а
/// не начинает его с холодного старта (инвариант 3).
#[test]
fn t732b_mute_keeps_the_fx_state_alive() {
    let d = AUDIO_BURST_FRAMES as i64;
    let mut clip = audio_clip(1, 0, 8 * d, 1);
    clip.fx = vec![FxInstance::slowed_reverb(1.0, 0.9, 120.0)];
    let p0 = project(vec![clip]);
    let src = || direct(&[(1, AudioPattern::Dc { value: 0.5 })]);

    // эталон: НИ РАЗУ не глушили — хвост идёт непрерывно
    let r = AudioRenderer::new(&p0);
    let mut f = src();
    let mut store = kt_engine::FxStateStore::default();
    let mut chunk = vec![0f32; d as usize * CH];
    let mut plain_tail = Vec::new();
    for i in 0..4 {
        r.render_frames_with(&mut f, &mut store, i * d, &mut chunk);
        if i == 3 {
            plain_tail = chunk.clone();
        }
    }

    // а теперь: два бёрста с mute, потом mute снят. Состояние цепочки
    // обязано быть ТЕМ ЖЕ — значит после снятия звучит ровно то же, что
    // и без глушения (реверб не начинается заново).
    let mut muted = p0.clone();
    muted.track_at_mut(A, 0).unwrap().mute = true;
    let rm = AudioRenderer::new(&muted);
    let ro = AudioRenderer::new(&p0);
    let mut f = src();
    let mut store = kt_engine::FxStateStore::default();
    let mut out = vec![0f32; d as usize * CH];
    for i in 0..4 {
        let r = if i < 2 { &rm } else { &ro };
        r.render_frames_with(&mut f, &mut store, i * d, &mut out);
        if i < 2 {
            assert!(out.iter().all(|s| *s == 0.0), "заглушённая дорожка обязана молчать");
        }
    }
    assert_eq!(out, plain_tail, "после снятия mute хвост начался заново — состояние умерло");
}
