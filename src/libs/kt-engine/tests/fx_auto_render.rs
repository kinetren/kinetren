//! Тест 191 (Р.30): автоматизация параметров в рендере. Кривые
//! разрешаются при построении плана кадра — композит анимированного клипа
//! в момент t байт-в-байт равен композиту клипа со СТАТИЧЕСКИ выставленным
//! значением кривой в этот момент. Пост-FX кэш не отдаёт стейлов между
//! разными t; правка кривой меняет fx_hash (кадры пересчитываются).

use kt_engine::VideoRenderer;
use kt_model::*;
use kt_time::Tick;
use std::sync::Arc;

const W: u32 = 64;
const H: u32 = 36;
const SPAN: i64 = 90_000;

fn frame() -> Arc<kt_media::VideoFrame> {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            rgba[i] = (x * 4) as u8;
            rgba[i + 1] = (y * 7) as u8;
            rgba[i + 2] = ((x + y) * 3) as u8;
            rgba[i + 3] = 255;
        }
    }
    Arc::new(kt_media::VideoFrame {
        pts: Tick(0),
        width: W,
        height: H,
        rgba: rgba.into_boxed_slice(),
        has_alpha: false,
    })
}

fn project(fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: W,
        height: H,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(SPAN),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx,
        fx_bypass: vec![],
        fx_auto: auto,
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn compose(p: &Project, t: Tick, cache: &kt_cache::BurstCache) -> (Vec<u8>, bool) {
    let mut vr = VideoRenderer::new(p);
    let q = kt_media::Quality { width: W, height: H };
    for (media, src_pts, q) in vr.layers_at(t, q) {
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video::video_cache_idx(src_pts),
            frame(),
        );
    }
    vr.compose_from_cache(cache, t, q)
}

#[test]
fn t191_animated_equals_static_at_each_moment() {
    let base = vec![FxInstance::blur(8), FxInstance::glow(0.5)];
    let auto = vec![
        vec![ParamCurve {
            param: "radius".into(),
            ease: false,
            keys: vec![AutoKey { t: Tick(0), v: 1.0 }, AutoKey { t: Tick(SPAN), v: 47.0 }],
            source: Default::default(),
        }],
        vec![ParamCurve {
            param: "strength".into(),
            ease: false,
            keys: vec![AutoKey { t: Tick(0), v: 0.0 }, AutoKey { t: Tick(SPAN), v: 2.0 }],
            source: Default::default(),
        }],
    ];
    let animated = project(base.clone(), auto);
    // моменты по сетке 30 fps (шаг 3000 тиков): начало, середина, у конца
    for t in [Tick(0), Tick(SPAN / 2), Tick(87_000)] {
        // статический эквивалент: значения кривых в этот момент. Кривая
        // сэмплируется по кадру ИСТОЧНИКА, который показан в этот тик, —
        // поэтому и здесь берём позицию, выровненную по сетке проекта
        let step = 90_000 / 30;
        let u = (t.0 / step * step) as f64 / SPAN as f64;
        let r = 1.0 + (47.0 - 1.0) * u;
        let s = 2.0 * u;
        let static_p =
            project(vec![FxInstance::blur(r.round() as u32), FxInstance::glow(s)], vec![]);
        let cache_a = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        let cache_s = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        let (a, ca) = compose(&animated, t, &cache_a);
        let (b, cb) = compose(&static_p, t, &cache_s);
        assert!(ca && cb);
        assert_eq!(a, b, "анимированный != статическому в t={} (u={u})", t.0);
    }
}

#[test]
fn t191_postfx_cache_no_stale_across_time_and_curve_edits() {
    let base = vec![FxInstance::blur(8)];
    let curve = |v1: f64| {
        vec![vec![ParamCurve {
            param: "radius".into(),
            ease: false,
            keys: vec![AutoKey { t: Tick(0), v: 1.0 }, AutoKey { t: Tick(SPAN), v: v1 }],
            source: Default::default(),
        }]]
    };
    let p = project(base.clone(), curve(47.0));
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let (t0, t1) = (Tick(0), Tick(SPAN / 2));
    let (a0, _) = compose(&p, t0, &cache);
    let (a1, _) = compose(&p, t1, &cache);
    assert_ne!(a0, a1, "разные моменты анимации дают разные кадры");
    // повтор тем же кэшем — байт-идентичен (мемоизация честная по t)
    let (a0b, _) = compose(&p, t0, &cache);
    assert_eq!(a0, a0b);
    // правка кривой меняет fx_hash: тот же кэш, но кадр другой (не стейл)
    let p2 = project(base, curve(20.0));
    let (a1c, _) = compose(&p2, t1, &cache);
    assert_ne!(a1, a1c, "правка кейфрейма обязана инвалидировать кадры");
}

/// Тест 230 (Р.34 К2): CamMotion сконвертирован в Pan с кривыми и рисует
/// ТО ЖЕ САМОЕ. Проверяется рендером, а не сравнением чисел: миграция
/// затрагивает и домен позиции ключа, и форму хранения, и `time_scale`, —
/// сойтись должны пиксели.
#[test]
fn t230_cammotion_migrates_to_pan_curves() {
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let keys = vec![
        CamKeyframe { t: 0.0, pan_x: -0.2, pan_y: 0.1, zoom: 1.0 },
        CamKeyframe { t: 0.5, pan_x: 0.0, pan_y: 0.0, zoom: 1.3 },
        CamKeyframe { t: 1.0, pan_x: 0.25, pan_y: -0.15, zoom: 1.6 },
    ];
    for (label, scale) in [("scale=1", 1.0f64), ("scale=2", 2.0)] {
        let cam = FxInstance::CamMotion {
            mode: PanMode::Fit,
            ease: true,
            time_scale: scale,
            keyframes: keys.clone(),
        };
        let before = project(vec![cam], vec![]);
        let mut after = before.clone();
        assert!(
            kt_model::migrate_legacy_cammotion(&mut after.clips[0]),
            "{label}: миграция обязана сработать"
        );
        assert!(
            matches!(after.clips[0].fx[0], FxInstance::Pan { .. }),
            "{label}: CamMotion обязан стать Pan"
        );
        assert_eq!(after.clips[0].fx_auto[0].len(), 3, "{label}: три кривые");
        after.validate().unwrap();
        // моменты ПО СЕТКЕ кадров (шаг 3000): старый путь читал сырой
        // прогресс клипа, новый — выровненную позицию материала
        for t in [Tick(0), Tick(21_000), Tick(45_000), Tick(69_000)] {
            let (a, ca) = compose(&before, t, &cache);
            let (b, cb) = compose(&after, t, &cache);
            assert!(ca && cb, "{label}: кадры прогреты");
            assert_eq!(a, b, "{label}: кадр в {t:?} обязан совпасть до байта");
        }
    }

    // ПУСТЫЕ кейфреймы у CamMotion означали не статику, а пресет v1
    // «наезд 1.0 → 1.35»: движение обязано пережить миграцию
    let empty = FxInstance::CamMotion {
        mode: PanMode::Fit,
        ease: true,
        time_scale: 1.0,
        keyframes: vec![],
    };
    let before = project(vec![empty], vec![]);
    let mut after = before.clone();
    assert!(kt_model::migrate_legacy_cammotion(&mut after.clips[0]));
    let zoom = after.clips[0].fx_auto[0].iter().find(|c| c.param == "zoom").expect("кривая наезда");
    assert_eq!(zoom.keys.len(), 2);
    assert!((zoom.keys[1].v - 1.35).abs() < 1e-9, "пресет v1 сохранён: {:?}", zoom.keys);
    for t in [Tick(0), Tick(SPAN / 2), Tick(69_000)] {
        let (a, _) = compose(&before, t, &cache);
        let (b, _) = compose(&after, t, &cache);
        assert_eq!(a, b, "пресет без кейфреймов: кадр в {t:?} совпадает");
    }

    // кривая НОВОГО домена главнее легаси-кейфреймов
    let cam = FxInstance::CamMotion {
        mode: PanMode::Fit,
        ease: true,
        time_scale: 1.0,
        keyframes: keys.clone(),
    };
    let mut c = project(vec![cam], vec![]).clips[0].clone();
    c.fx_auto = vec![vec![ParamCurve {
        param: "zoom".into(),
        ease: false,
        keys: vec![AutoKey { t: Tick(0), v: 4.0 }],
        source: Default::default(),
    }]];
    kt_model::migrate_legacy_cammotion(&mut c);
    let zoom = c.fx_auto[0].iter().find(|c| c.param == "zoom").unwrap();
    assert_eq!(zoom.keys.len(), 1, "существующую кривую миграция не трогает");
    assert_eq!(zoom.keys[0].v, 4.0);
}
