//! Р.62 (change B): ЦЕПОЧКА ДОРОЖКИ — на сумме её событий.
//!
//! Утверждения, ради которых раунд затевался: цепочка звучит на сумме (а
//! не дописана в каждое событие), её хвост тянется ЗА последним событием,
//! выход выровнен со входом (упреждение съедено кольцами), а состояние
//! живёт по СТАБИЛЬНОМУ id — вставка дорожки выше соседей не глушит.

use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::LaneKind::Audio as A;
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const D: i64 = AUDIO_BURST_FRAMES as i64;

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

fn clip(id: u64, lane: u32, start_f: i64, dur_f: i64, media: u64) -> ClipEvent {
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
        start: Tick::from_audio_frames(start_f),
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

/// Отрендерить `bursts` бёрстов подряд ОДНИМ потоком (состояние живёт).
fn stream(p: &Project, f: &mut dyn AudioFetcher, bursts: i64) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let mut store = FxStateStore::default();
    r.sync_fx(&mut store);
    let mut all = Vec::new();
    let mut buf = vec![0f32; D as usize * CH];
    for i in 0..bursts {
        r.render_frames_with(f, &mut store, i * D, &mut buf);
        all.extend_from_slice(&buf);
    }
    all
}

fn peak(v: &[f32]) -> f32 {
    v.iter().fold(0f32, |a, b| a.max(b.abs()))
}

/// Тест 733 (Р.62): цепочка дорожки звучит НА СУММЕ, хвост тянется за
/// последним событием, выход выровнен со входом.
#[test]
fn t733_track_chain_runs_on_the_sum_and_keeps_its_tail() {
    // две реплики встык на ОДНОЙ дорожке; реверб — на ДОРОЖКЕ
    let clips = vec![clip(1, 0, 0, D, 1), clip(2, 0, D, D, 1)];
    let mut p = project(clips);
    p.track_at_mut(A, 0).unwrap().fx = vec![FxInstance::slowed_reverb(1.0, 0.9, 120.0)];
    p.validate().unwrap();

    let mut f = direct(&[(1, AudioPattern::Dc { value: 0.5 })]);
    let out = stream(&p, &mut f, 24);
    let burst = |i: usize| &out[(i * D as usize) * CH..((i + 1) * D as usize) * CH];

    // ХВОСТ ЗА ПОСЛЕДНИМ СОБЫТИЕМ: материал кончился на 2·D, а дорожка
    // ещё звучит — цепочка дозванивает на тишине
    assert!(peak(burst(3)) > 1e-4, "хвост цепочки дорожки не тянется за последним событием");

    // …и он ЗАТУХАЕТ, а не тянется бесконечно (реверб 120 мс = ~5.6
    // бёрстов на линию, поэтому сравниваем с ДАЛЁКИМ концом)
    assert!(
        peak(burst(23)) < peak(burst(3)),
        "хвост не затухает: {} против {}",
        peak(burst(23)),
        peak(burst(3))
    );

    // ЦЕПОЧКА НА СУММЕ, а не в каждом событии: та же цепочка, дописанная
    // каждому событию, даёт ДРУГОЙ звук (у каждого свой реверб со своим
    // состоянием, и на шве хвост первого обрывается)
    let mut per_clip = project(vec![clip(1, 0, 0, D, 1), clip(2, 0, D, D, 1)]);
    for c in per_clip.clips.iter_mut() {
        c.fx = vec![FxInstance::slowed_reverb(1.0, 0.9, 120.0)];
    }
    per_clip.validate().unwrap();
    let mut f2 = direct(&[(1, AudioPattern::Dc { value: 0.5 })]);
    let other = stream(&per_clip, &mut f2, 6);
    assert_ne!(out, other, "цепочка дорожки звучит как дописанная в каждое событие");
}

/// Тест 733b: УПРЕЖДЕНИЕ цепочки дорожки не сдвигает выход. Лимитер с
/// окном анализа читает вперёд, а кольца возвращают выход на место:
/// относительно дорожки без цепочки сдвига нет.
#[test]
fn t733b_track_chain_lookahead_keeps_the_output_aligned() {
    // событие начинается НЕ в нуле: сдвиг был бы виден как смещение фронта
    let at = D / 2;
    let mut p = project(vec![clip(1, 0, at, D, 1)]);
    // нейтральный лимитер: окно анализа есть, а усиления нет
    // порог 0 дБ — усиления нет, а окно анализа (упреждение) есть
    p.track_at_mut(A, 0).unwrap().fx = vec![FxInstance::Limiter {
        threshold_db: 0.0,
        ceiling_db: 0.0,
        lookahead_ms: 5.0,
        release_ms: 80.0,
    }];
    p.validate().unwrap();
    let mut f = direct(&[(1, AudioPattern::Dc { value: 0.25 })]);
    let with = stream(&p, &mut f, 4);

    let plain = project(vec![clip(1, 0, at, D, 1)]);
    let mut f2 = direct(&[(1, AudioPattern::Dc { value: 0.25 })]);
    let without = stream(&plain, &mut f2, 4);

    let front =
        |v: &[f32]| v.chunks_exact(CH).position(|s| s[0].abs() > 1e-6).unwrap_or(usize::MAX);
    assert_eq!(
        front(&with),
        front(&without),
        "цепочка дорожки сдвинула выход — упреждение не съедено кольцами"
    );
}

/// Тест 734 (Р.62): ключ состояния — СТАБИЛЬНЫЙ id дорожки. Вставка
/// дорожки выше перенумеровывает соседей, но состояние их цепочек не
/// трогает (инвариант 3); удаление дорожки своё состояние уносит.
#[test]
fn t734_track_state_survives_insertion_above_and_dies_with_the_track() {
    let mut p = project(vec![clip(1, 0, 0, 4 * D, 1)]);
    p.track_at_mut(A, 0).unwrap().fx = vec![FxInstance::slowed_reverb(1.0, 0.9, 120.0)];
    p.validate().unwrap();
    let id = p.track_at(A, 0).unwrap().id;

    // эталон: четыре бёрста одним потоком
    let mut f = direct(&[(1, AudioPattern::Dc { value: 0.5 })]);
    let want = stream(&p, &mut f, 4);

    // а теперь посреди потока ВСТАВЛЯЕМ дорожку ВЫШЕ: номер соседа стал
    // другим, id — нет, значит состояние обязано продолжиться
    let mut m = TimelineModel::new(p.clone());
    let mut f = direct(&[(1, AudioPattern::Dc { value: 0.5 })]);
    let mut store = FxStateStore::default();
    let mut buf = vec![0f32; D as usize * CH];
    let mut got = Vec::new();
    for i in 0..4 {
        if i == 2 {
            m.apply(Command::InsertTrack { kind: A, at: 0 }).unwrap();
            assert_eq!(
                m.project().track_at(A, 1).unwrap().id,
                id,
                "сцена: дорожка с цепочкой переехала на номер 1, id прежний"
            );
        }
        let r = AudioRenderer::new(m.project());
        r.sync_fx(&mut store);
        r.render_frames_with(&mut f, &mut store, i * D, &mut buf);
        got.extend_from_slice(&buf);
    }
    assert_eq!(got, want, "вставка дорожки выше убила состояние цепочки ниже");

    // состояние дорожки с цепочкой живо, пока она жива
    assert!(store.track_mut(id).is_some(), "живая дорожка обязана сохранить состояние");

    // а УДАЛЕНИЕ самой дорожки с цепочкой уносит её состояние (пустая
    // вставленная остаётся — последнюю аудиодорожку убрать нельзя)
    m.apply(Command::RemoveTrack { kind: A, at: 1 }).unwrap();
    let r = AudioRenderer::new(m.project());
    r.sync_fx(&mut store);
    assert!(store.track_mut(id).is_none(), "удалённая дорожка не убрала своё состояние");
}
