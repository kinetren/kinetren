//! Тест 17 (docs/porting-notes.md): согласованность prefetch-окна с РЕАЛЬНЫМ
//! потреблением источника варпом (регрессия п.13 — «после случайных прыжков
//! slowed&reverb играет тишину»: линейный маппинг wanted грел не те бёрсты).

use kt_cache::{BurstCache, CacheConfig};
use kt_engine::{AudioFetcher, AudioRenderer, CacheFetcher, DirectFetcher};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};
use std::sync::Arc;

const CH: usize = AUDIO_CHANNELS;
const CLIP_FRAMES: i64 = 48_000 * 6; // 6 секунд

fn slowed_project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick::from_audio_frames(0),
        duration: Tick::from_audio_frames(CLIP_FRAMES),
        source_in: Tick::from_audio_frames(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![FxInstance::SlowedReverb {
            rate: 0.5,
            mix: 0.15,
            reverb_delay_ms: 85.0,
            damp: 0.32,
            decay: 0.58,
            wet_gain: 0.95,
            predelay_ms: 0.0,
            width: 1.0,
            frames: Default::default(),
            automation: vec![
                SlowedKeyframe { t: 0.0, rate: 0.9, mix: 0.15 },
                SlowedKeyframe { t: 0.5, rate: 0.4, mix: 0.15 },
                SlowedKeyframe { t: 1.0, rate: 0.8, mix: 0.15 },
            ],
        }],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn direct() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 220.0 })),
    );
    f
}

/// Обёртка-логгер: пишет каждый прочитанный burst.
struct Recording {
    inner: DirectFetcher,
    log: Vec<(MediaId, i64)>,
}

impl AudioFetcher for Recording {
    fn audio_burst(&mut self, media: MediaId, idx: i64) -> Option<Arc<kt_media::AudioBurst>> {
        self.log.push((media, idx));
        self.inner.audio_burst(media, idx)
    }
}

fn rms(buf: &[f32]) -> f64 {
    (buf.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>() / buf.len() as f64).sqrt()
}

/// Главный инвариант: wanted ⊇ consumed при любых прыжках.
#[test]
fn t17_wanted_bursts_cover_fx_consumption_after_random_seeks() {
    let project = slowed_project();
    let renderer = AudioRenderer::new(&project);
    let mut store = kt_engine::FxStateStore::new();
    renderer.sync_fx(&mut store);

    // детерминированные «случайные» позиции: LCG
    let mut x: u64 = 0x2545F4914F6CDD1D;
    let mut positions = Vec::new();
    for _ in 0..8 {
        x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        positions.push(((x >> 20) as i64) % (CLIP_FRAMES - 48_000));
    }
    positions.extend([0, (CLIP_FRAMES as f64 * 0.95) as i64]);

    for &pos in &positions {
        store.reset_all(); // как player.seek: сброс FX-state, hard_seek + preroll
        let mut fetcher = Recording { inner: direct(), log: Vec::new() };
        let frames = 48_000 / 2;
        let mut out = vec![0f32; frames * CH];
        renderer.render_frames_with(&mut fetcher, &mut store, pos, &mut out);

        let wanted = renderer.wanted_audio_bursts(pos..pos + frames as i64);
        for &(media, idx) in &fetcher.log {
            let covered = wanted.iter().any(|(m, r)| *m == media && r.contains(&idx));
            assert!(
                covered,
                "burst (media {:?}, idx {idx}) прочитан рендером, но НЕ входит в \
                 wanted_audio_bursts({pos}..{}) — prefetch грел бы не те бёрсты",
                media,
                pos + frames as i64
            );
        }
    }
}

/// Прогрев кэша строго по wanted → рендер без промахов, байт-в-байт равен
/// холодному эталону, и это не тишина.
#[test]
fn t17_seek_into_slowed_clip_from_warm_cache_matches_cold_reference() {
    let project = slowed_project();
    let renderer = AudioRenderer::new(&project);
    let frames = 48_000usize; // окно 1 c
    let positions = [
        (CLIP_FRAMES as f64 * 0.3) as i64,
        (CLIP_FRAMES as f64 * 0.6) as i64,
        (CLIP_FRAMES as f64 * 0.93) as i64,
    ];

    for &pos in &positions {
        // холодный эталон
        let mut cold_store = kt_engine::FxStateStore::new();
        renderer.sync_fx(&mut cold_store);
        let mut cold = vec![0f32; frames * CH];
        renderer.render_frames_with(&mut direct(), &mut cold_store, pos, &mut cold);

        // прогрев кэша ровно по wanted
        let cache = Arc::new(BurstCache::new(CacheConfig::default()));
        let mut warm = DirectFetcher::new(Some(cache.clone()));
        warm.add_source(
            MediaId(1),
            Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 220.0 })),
        );
        for (media, range) in renderer.wanted_audio_bursts(pos..pos + frames as i64) {
            for idx in range {
                warm.audio_burst(media, idx);
            }
        }

        // рендер только из кэша
        let mut store = kt_engine::FxStateStore::new();
        renderer.sync_fx(&mut store);
        let mut fetcher = CacheFetcher::new(cache);
        let mut out = vec![0f32; frames * CH];
        let stats = renderer.render_frames_with(&mut fetcher, &mut store, pos, &mut out);

        assert_eq!(fetcher.miss_count(), 0, "pos {pos}: промахи после прогрева по wanted");
        assert_eq!(stats.underruns, 0, "pos {pos}: underruns после прогрева");
        assert_eq!(out, cold, "pos {pos}: тёплый рендер разошёлся с холодным эталоном");
        assert!(rms(&out) > 0.01, "pos {pos}: рендер не должен быть тишиной, rms={}", rms(&out));
    }
}

/// Конец клипа не должен тихо спадать (регрессия «оборванный хвост»:
/// жёсткое окно источника глушило sinc-прайминг у конца).
#[test]
fn t17_slowed_clip_end_has_no_silent_tail() {
    let project = slowed_project();
    let renderer = AudioRenderer::new(&project);
    let mut store = kt_engine::FxStateStore::new();
    renderer.sync_fx(&mut store);

    let tail_frames = AUDIO_BURST_FRAMES;
    let mid_pos = CLIP_FRAMES / 2;
    let end_pos = CLIP_FRAMES - tail_frames as i64;

    let mut mid = vec![0f32; tail_frames * CH];
    renderer.render_frames_with(&mut direct(), &mut store, mid_pos, &mut mid);
    store.reset_all();
    let mut end = vec![0f32; tail_frames * CH];
    renderer.render_frames_with(&mut direct(), &mut store, end_pos, &mut end);

    let (rm, re) = (rms(&mid), rms(&end));
    assert!(rm > 0.01, "середина клипа не тишина, rms={rm}");
    assert!(
        re > rm * 0.5,
        "последний burst клипа спал больше чем на 6 dB против середины: mid={rm:.4} end={re:.4}"
    );
}

/// Тест 285 (Р.43 п.295): у РЕВЕРС-ТРАНСПОРТА префетч греет то же, что
/// читает рендер.
///
/// Претензия пользователя про реверсы шла в одном ряду с underrun'ами.
/// Причина отдельная от зеркала (п.292): реверс-транспорт играет ВСЕ
/// события через `render_plain_clip` — dry и ЛИНЕЙНО, варпа этот путь не
/// знает вовсе («как Vegas»), — а `wanted_audio_bursts` считал окно ПО
/// ВАРПУ. На замедленном событии это два разных пролёта источника: префетч
/// грел одни бёрсты, рендер просил другие, и промах был вечным. Считается
/// он там в КАДРАХ, до 1024 за один пропущенный бёрст, — отсюда и скорость
/// роста счётчика.
///
/// Утверждение то же, что у t17, но для сырого пути: wanted ⊇ consumed.
#[test]
fn t285_raw_reverse_prefetch_covers_what_raw_render_reads() {
    let project = slowed_project();
    let renderer = AudioRenderer::new(&project);
    let frames = 48_000usize / 2;

    for &pos in &[0i64, CLIP_FRAMES / 3, CLIP_FRAMES / 2, (CLIP_FRAMES as f64 * 0.9) as i64] {
        let mut fetcher = Recording { inner: direct(), log: Vec::new() };
        let mut out = vec![0f32; frames * CH];
        renderer.render_frames_raw(&mut fetcher, pos, &mut out);

        let raw = renderer.wanted_audio_bursts_raw(pos..pos + frames as i64);
        for &(media, idx) in &fetcher.log {
            assert!(
                raw.iter().any(|(m, r)| *m == media && r.contains(&idx)),
                "pos {pos}: бёрст {idx} прочитан сырым рендером, но не входит в \
                 wanted_audio_bursts_raw — префетч грел бы не те бёрсты"
            );
        }

        // и показываем, ЧТО ИМЕННО было сломано: варп-окно этот бёрст не
        // покрывает, значит на реверсе промах был неизбежен
        let warped = renderer.wanted_audio_bursts(pos..pos + frames as i64);
        let missed = fetcher
            .log
            .iter()
            .filter(|(m, idx)| !warped.iter().any(|(wm, r)| wm == m && r.contains(idx)))
            .count();
        if pos > 0 {
            assert!(
                missed > 0,
                "pos {pos}: если варп-окно и так всё покрывает, тест ничего не сторожит"
            );
        }
    }
}
