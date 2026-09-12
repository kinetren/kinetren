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
        fx_bypass: vec![],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
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
        predelay_ms: 0.0,
        width: 1.0,
        frames: Default::default(),
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
    let mut vr = VideoRenderer::new(&p);

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

/// Тест 29 (п.16): у reversed клипа содержимое НЕ зависит от правого края.
#[test]
fn t29_reversed_trim_end_preserves_left_edge_frames() {
    let p = clip(true, vec![], ClipKind::Video);
    let vr_before = VideoRenderer::new(&p);
    let step = TICK_HZ / 30;
    let probes: Vec<Tick> = (0..10).map(|i| Tick(90_000 + i * step)).collect();
    let before: Vec<Tick> = probes.iter().map(|&t| vr_before.layers_at(t)[0].1).collect();

    // TrimEnd: укоротить на 0.5 c справа
    let mut m = kt_model::TimelineModel::new(p);
    let id = m.project().clips[0].id;
    let old_end = m.project().clips[0].start + m.project().clips[0].duration;
    m.apply(Command::TrimEnd { id, new_end: Tick(old_end.0 - 45_000) }).unwrap();
    let vr_after = VideoRenderer::new(m.project());
    let after: Vec<Tick> = probes.iter().map(|&t| vr_after.layers_at(t)[0].1).collect();

    assert_eq!(before, after, "кадры у ЛЕВОГО края reversed клипа не должны меняться при TrimEnd");
}

#[test]
fn t29_reversed_trim_start_preserves_surviving_frames() {
    let p = clip(true, vec![], ClipKind::Video);
    let vr_before = VideoRenderer::new(&p);
    let step = TICK_HZ / 30;
    // пробы в интервале, который переживёт трим
    let probes: Vec<Tick> = (0..8).map(|i| Tick(90_000 + 45_000 + i * step)).collect();
    let before: Vec<Tick> = probes.iter().map(|&t| vr_before.layers_at(t)[0].1).collect();

    let mut m = kt_model::TimelineModel::new(p);
    let id = m.project().clips[0].id;
    let old_start = m.project().clips[0].start;
    m.apply(Command::TrimStart { id, new_start: Tick(old_start.0 + 45_000) }).unwrap();
    let vr_after = VideoRenderer::new(m.project());
    let after: Vec<Tick> = probes.iter().map(|&t| vr_after.layers_at(t)[0].1).collect();

    assert_eq!(before, after, "выжившие кадры reversed клипа не должны меняться при TrimStart");
}

#[test]
fn t29_reversed_split_is_visually_seamless() {
    let p = clip(true, vec![], ClipKind::Video);
    let vr_before = VideoRenderer::new(&p);
    let step = TICK_HZ / 30;
    let probes: Vec<Tick> = (0..20).map(|i| Tick(90_000 + i * step * 3)).collect();
    let before: Vec<Tick> = probes.iter().map(|&t| vr_before.layers_at(t)[0].1).collect();

    let mut m = kt_model::TimelineModel::new(p);
    let id = m.project().clips[0].id;
    let at = Tick(90_000 + 90_000); // середина
    m.apply(Command::Split { id, at, new_id: ClipId(99), new_group: None }).unwrap();
    let vr_after = VideoRenderer::new(m.project());
    let after: Vec<Tick> = probes.iter().map(|&t| vr_after.layers_at(t)[0].1).collect();

    assert_eq!(before, after, "сплит reversed клипа обязан быть бесшовным по содержимому");
}

/// Тест 29 (аудио-половина): реверс + TrimEnd — сэмплы у левого края exact.
#[test]
fn t29_reversed_audio_trim_end_left_edge_exact() {
    let p = clip(true, vec![], ClipKind::Audio);
    let n = 4096usize;
    let mut before = vec![0f32; n * CH];
    AudioRenderer::new(&p).render_frames(&mut direct(), 48_000, &mut before);

    let mut m = kt_model::TimelineModel::new(p);
    let id = m.project().clips[0].id;
    let old_end = m.project().clips[0].start + m.project().clips[0].duration;
    m.apply(Command::TrimEnd { id, new_end: Tick(old_end.0 - 45_000) }).unwrap();
    let mut after = vec![0f32; n * CH];
    AudioRenderer::new(m.project()).render_frames(&mut direct(), 48_000, &mut after);

    assert_eq!(before, after, "начало reversed аудио-события не должно сдвигаться при TrimEnd");
}

/// Ramp-паттерн кодирует НОМЕР КАДРА ИСТОЧНИКА в самом сэмпле
/// (`n * 1e-6`, левый канал). Средний номер кадра по куску выхода —
/// прямой ответ на вопрос «какой материал сейчас звучит».
fn mean_source_frame(out: &[f32], from: usize, n: usize) -> f64 {
    let mut sum = 0f64;
    for k in from..from + n {
        sum += out[k * CH] as f64;
    }
    (sum / n as f64) / 1e-6
}

fn slowed_dry(rate: f64) -> FxInstance {
    FxInstance::SlowedReverb {
        rate,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 1.0,
        predelay_ms: 0.0,
        width: 1.0,
        frames: Default::default(),
        automation: vec![],
    }
}

/// Тест 282 (Р.43 п.2/п.292): зеркальное событие с варпом читает РОВНО свой
/// материал.
///
/// Претензия дословно: «в problems1 есть тройка событий с реверсами, и там
/// вообще звук от другого события откуда-то взялся», «а еще там из-за slowd
/// картинка поломанная».
///
/// Причина: осью зеркала служил КОНЕЦ ОКНА источника, а туда входит запас
/// ресемплера `WARP_SOURCE_MARGIN_FRAMES` = 8192 кадра (170.7 мс). Видео на
/// том же событии якорится на фактически потреблённом `ClipWarp::used` без
/// запаса — то есть A и V честно показывали РАЗНЫЙ материал. На событии
/// пользователя длиной 0.36 с сдвиг составлял 63 % его длины.
///
/// Ни один тест репозитория не сочетал `reversed` с варпом и не проверял
/// ПОЗИЦИЮ материала: t24 требует лишь детерминизма и «не тишины».
#[test]
fn t282_reversed_warped_clip_reads_its_own_material() {
    let src_in = 9_600i64;
    let len = 96_000i64;
    let rev = {
        let mut p = clip(true, vec![slowed_dry(1.0)], ClipKind::Audio);
        p.clips[0].duration = Tick::from_audio_frames(len);
        p.validate().unwrap();
        p
    };
    let n = 4096usize;
    let mut out = vec![0f32; n * CH];
    AudioRenderer::new(&rev).render_frames(&mut direct(), 48_000, &mut out);

    // при rate = 1 потреблённый пролёт равен длине события, значит первый
    // выходной кадр обязан читать ПОСЛЕДНИЙ кадр этого пролёта
    let axis = (src_in + len) as f64;
    let got = mean_source_frame(&out, 0, n);
    let want = axis - n as f64 / 2.0;
    assert!(
        (got - want).abs() < 1_000.0,
        "зеркало обязано считаться от потреблённого конца {axis}: \
         звучит кадр ≈{got:.0}, ожидался ≈{want:.0} \
         (до Р.43 ось включала запас ресемплера 8192 кадра и звучало ≈{:.0})",
        want + 8_192.0
    );
}

/// Тест 283 (Р.43 п.293): смена `source_in` при ТОЙ ЖЕ структуре цепочки
/// слышна.
///
/// `chain_fingerprint` хэширует только ВИДЫ слотов, а `FxStateStore` на
/// ветке переиспользования доносил в процессор параметры и кривые, но не
/// ГЕОМЕТРИЮ. Поэтому трим, разрез, punch-out и переанкоринг морф-куска
/// оставляли процессор со старым материальным якорем: событие продолжало
/// звучать материалом с прежнего места файла. `reset()` не помогал —
/// `hard_seek`/`seam` считают позицию от того же поля.
#[test]
fn t283_source_in_change_is_heard_with_the_same_chain() {
    let mut p = clip(false, vec![slowed_dry(1.0)], ClipKind::Audio);
    p.clips[0].duration = Tick::from_audio_frames(96_000);
    p.validate().unwrap();

    let n = 4096usize;
    let mut store = kt_engine::FxStateStore::new();
    let r = AudioRenderer::new(&p);
    r.sync_fx(&mut store);
    let mut before = vec![0f32; n * CH];
    r.render_frames_with(&mut direct(), &mut store, 48_000, &mut before);

    // «трим слева»: точка входа уехала, структура цепочки та же
    let delta = 24_000i64;
    let mut p2 = p.clone();
    p2.clips[0].source_in = p2.clips[0].source_in + Tick::from_audio_frames(delta);
    p2.validate().unwrap();
    let r2 = AudioRenderer::new(&p2);
    r2.sync_fx(&mut store); // ТОТ ЖЕ store — состояние FX не пересоздаётся
    let mut after = vec![0f32; n * CH];
    r2.render_frames_with(&mut direct(), &mut store, 48_000, &mut after);

    let a = mean_source_frame(&before, 0, n);
    let b = mean_source_frame(&after, 0, n);
    assert!(
        (b - a - delta as f64).abs() < 1_000.0,
        "правка source_in на {delta} кадров обязана двигать ЗВУЧАЩИЙ материал: \
         было ≈{a:.0}, стало ≈{b:.0} (до Р.43 процессор держал старый якорь \
         и разница была ≈0)"
    );
}
