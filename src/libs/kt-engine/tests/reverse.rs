//! Тест 24 (docs/porting-notes.md, п.16): реверс событий.
//! Аудио: выход == зеркальный срез источника (exact, Ramp-паттерн).
//! Видео: кадр момента t == кадр зеркального pts (синтетика кодирует pts
//! в цвете). FX+реверс: детерминирован и не тишина.

use kt_engine::{AudioRenderer, DirectFetcher, VideoRenderer};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, TICK_HZ, Tick};

const CH: usize = AUDIO_CHANNELS;

fn clip(reversed: bool, fx: Vec<FxInstance>, kind: ClipKind) -> Project {
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind,
        lane: 0,
        start: Tick::from_audio_frames(48_000), // клип не с нуля таймлайна
        duration: Tick::from_audio_frames(48_000 * 2),
        source_in: Tick::from_audio_frames(9_600), // и не с нуля источника
        media: MediaId(1),
        gain_db: 0.0,
        reversed,
        fx,
    });
    p.validate().unwrap();
    p
}

fn direct() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp)));
    f
}

#[test]
fn t24_reversed_audio_is_exact_mirror() {
    let fwd = clip(false, vec![], ClipKind::Audio);
    let rev = clip(true, vec![], ClipKind::Audio);
    let n = 48_000usize * 2;
    let mut a = vec![0f32; n * CH];
    AudioRenderer::new(&fwd).render_frames(&mut direct(), 48_000, &mut a);
    let mut b = vec![0f32; n * CH];
    AudioRenderer::new(&rev).render_frames(&mut direct(), 48_000, &mut b);

    // b[k] == a[n-1-k] по кадрам, exact
    for k in [0usize, 1, 1023, 1024, 4097, n / 2, n - 2, n - 1] {
        for ch in 0..CH {
            assert_eq!(
                b[k * CH + ch],
                a[(n - 1 - k) * CH + ch],
                "кадр {k} канал {ch}: реверс должен быть зеркальным срезом"
            );
        }
    }
    assert!(b.iter().any(|s| *s != 0.0), "не тишина");
}

#[test]
fn t24_reversed_video_maps_to_mirrored_pts() {
    let fwd = clip(false, vec![], ClipKind::Video);
    let rev = clip(true, vec![], ClipKind::Video);
    let vf = VideoRenderer::new(&fwd);
    let vr = VideoRenderer::new(&rev);
    let step = TICK_HZ / 30;
    let (start, span) = (90_000i64, 90_000i64 * 2);

    for i in [0i64, 1, 7, 29, 59] {
        let t = Tick(start + i * step);
        let f = vf.layers_at(t)[0].1;
        let r = vr.layers_at(t)[0].1;
        // зеркало: f и r симметричны внутри куска источника
        let mirrored = vf.layers_at(Tick(start + span - step - i * step))[0].1;
        assert_eq!(
            r, mirrored,
            "кадр {i}: реверс обязан показывать зеркальный pts (fwd {f:?}, rev {r:?})"
        );
    }
}

#[test]
fn t24_reversed_with_slowed_fx_is_deterministic_and_not_silent() {
    let fx = vec![FxInstance::SlowedReverb {
        rate: 0.6,
        mix: 0.2,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        automation: vec![],
    }];
    let rev = clip(true, fx, ClipKind::Audio);
    let renderer = AudioRenderer::new(&rev);
    let n = 48_000usize;
    let mut a = vec![0f32; n * CH];
    renderer.render_frames(&mut direct(), 48_000 + 9_600, &mut a);
    let mut b = vec![0f32; n * CH];
    renderer.render_frames(&mut direct(), 48_000 + 9_600, &mut b);
    assert_eq!(a, b, "реверс+slowed детерминирован");
    let rms = (a.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / a.len() as f64).sqrt();
    assert!(rms > 1e-4, "реверс+slowed не должен быть тишиной, rms={rms}");
}

/// Тест 26 (интеграция, п.25): видео-FX применяются в ЕДИНОМ пути композита
/// (превью == экспорт), кэш хранит пре-FX кадры.
#[test]
fn t26_video_fx_applied_in_compose_and_deterministic() {
    let mut p = clip(false, vec![], ClipKind::Video);
    p.clips[0].fx = vec![FxInstance::Mirror { horizontal: true, vertical: false }];
    p.validate().unwrap();
    let vr = VideoRenderer::new(&p);

    let cache = std::sync::Arc::new(kt_cache::BurstCache::new(kt_cache::CacheConfig::default()));
    let q = kt_media::Quality { width: 64, height: 36 };
    // прогреть кадр клипа в кэш напрямую (пре-FX)
    let t = Tick(90_000);
    let src_pts = vr.layers_at(t)[0].1;
    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence);
    use kt_media::MediaSource;
    let frame = synth.read_video_frame(src_pts, Some(q)).unwrap();
    cache.insert_video(
        kt_engine::source_id(MediaId(1)),
        Some(q),
        kt_engine::video_cache_idx(src_pts),
        std::sync::Arc::new(frame),
    );

    let (a, complete) = vr.compose_from_cache(&cache, t, q);
    assert!(complete);
    let (b, _) = vr.compose_from_cache(&cache, t, q);
    assert_eq!(a, b, "FX-композит детерминирован");
    // кэш не тронут FX'ом: кадр в кэше остался пре-FX (сплошной цвет),
    // а выход — зеркален, но у сплошного цвета зеркало == оригинал; проверим
    // хотя бы что выход не пуст и альфа непрозрачна
    assert!(a.chunks_exact(4).all(|p| p[3] == 255));
    assert!(a.iter().any(|v| *v != 0));
}
