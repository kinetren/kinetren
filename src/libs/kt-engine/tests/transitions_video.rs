//! Тест 137 (п.137 porting-notes, Р.24): видео-dissolve по переходам.
//!
//! Пара клипов в окне перехода блендится линейно (целочисленный k/4096) в
//! один слой; недостающая половина — полный вес присутствующей +
//! complete=false; ручки продолжают источник линейно (reversed — держит
//! кламп-кадр). Superset t66-харнесса: кадры — сплошной цвет от pts.

use kt_engine::VideoRenderer;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;

fn project_pair() -> Project {
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
    for id in [1u64, 2] {
        p.media.push(MediaRef { id: MediaId(id), path: format!("synth:{id}") });
    }
    for (id, start, src_in, media) in [(1u64, 0i64, 0i64, 1u64), (2, 90_000, 30_000, 2)] {
        p.clips.push(ClipEvent {
            id: ClipId(id),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(start),
            duration: Tick(90_000),
            source_in: Tick(src_in),
            media: MediaId(media),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            opacity: 1.0,
        });
    }
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick(9_000),
        out_offset: Tick(9_000),
        kind: TransitionKind::Crossfade,
    });
    p.validate().unwrap();
    p
}

fn warm(vr: &VideoRenderer, cache: &kt_cache::BurstCache, t: Tick, q: kt_media::Quality) {
    use kt_media::MediaSource;
    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence);
    for (media, src_pts) in vr.layers_at(t) {
        let frame = synth.read_video_frame(src_pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video_cache_idx(src_pts),
            std::sync::Arc::new(frame),
        );
    }
}

fn synth_frame(src_pts: Tick, q: kt_media::Quality) -> Vec<u8> {
    use kt_media::MediaSource;
    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence);
    synth.read_video_frame(src_pts, Some(q)).unwrap().rgba.into_vec()
}

fn dissolve(a: &[u8], b: &[u8], k: u32) -> Vec<u8> {
    let ik = 4096 - k;
    a.iter().zip(b).map(|(&av, &bv)| ((av as u32 * ik + bv as u32 * k + 2048) >> 12) as u8).collect()
}

#[test]
fn t137_dissolve_blends_pair_and_is_deterministic() {
    let p = project_pair();
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };
    let (w0, w1) = (81_000i64, 99_000i64);

    for t in [78_000i64, 81_000, 90_000, 96_000, 99_000] {
        let t = Tick(t);
        warm(&vr, &cache, t, q);
        let (out, complete) = vr.compose_from_cache(&cache, t, q);
        assert!(complete, "t {t:?}: all layers warmed");
        let a = synth_frame(vr.grid.align_floor(Tick(t.0)), q); // A: src = t (src_in 0)
        let b = synth_frame(vr.grid.align_floor(Tick(30_000 + (t.0 - 90_000))), q);
        let expect = if t.0 < w0 {
            a
        } else if t.0 < w1 {
            dissolve(&a, &b, (((t.0 - w0) * 4096) / (w1 - w0)).clamp(0, 4096) as u32)
        } else {
            b
        };
        assert_eq!(out, expect, "t {t:?}: dissolve math");
        // детерминизм: повторный compose байт-идентичен
        let (again, _) = vr.compose_from_cache(&cache, t, q);
        assert_eq!(out, again);
    }
}

#[test]
fn t137_missing_pair_half_gets_full_weight_and_incomplete() {
    use kt_media::MediaSource;
    let p = project_pair();
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(90_000);
    // греем ТОЛЬКО слой A (media 1)
    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence);
    let frame = synth.read_video_frame(vr.grid.align_floor(t), Some(q)).unwrap();
    cache.insert_video(
        kt_engine::source_id(MediaId(1)),
        Some(q),
        kt_engine::video_cache_idx(vr.grid.align_floor(t)),
        std::sync::Arc::new(frame),
    );
    let (out, complete) = vr.compose_from_cache(&cache, t, q);
    assert!(!complete, "missing half of the pair");
    assert_eq!(out, synth_frame(vr.grid.align_floor(t), q), "present half at full weight");
}

#[test]
fn t137_layers_include_handles_in_window() {
    let p = project_pair();
    let vr = VideoRenderer::new(&p);
    // в окне перехода активны ОБА клипа (ручки), вне — один
    assert_eq!(vr.layers_at(Tick(84_000)).len(), 2, "handles active inside window");
    assert_eq!(vr.layers_at(Tick(78_000)).len(), 1);
    assert_eq!(vr.layers_at(Tick(99_000)).len(), 1);
    // левая ручка B читает источник ДО куска: src = 30000 + (84000-90000)
    let layers = vr.layers_at(Tick(84_000));
    assert!(layers.contains(&(MediaId(2), vr.grid.align_floor(Tick(24_000)))));
}

#[test]
fn t137_dissolve_survives_fx_chain_on_pair() {
    // FX-клип в паре: пост-FX кэш + бленд; повторный compose из кэша
    // байт-идентичен (наследие t66)
    let mut p = project_pair();
    p.clips[0].fx = vec![FxInstance::Noise { amount: 0.3 }];
    p.validate().unwrap();
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(90_000);
    warm(&vr, &cache, t, q);
    let (first, c1) = vr.compose_from_cache(&cache, t, q);
    assert!(c1);
    let (second, c2) = vr.compose_from_cache(&cache, t, q);
    assert!(c2);
    assert_eq!(first, second, "postfx-cached dissolve is byte-stable");
}

// ---- тест 156: фейд у края/Gap (Р.25) ----------------------------------------

/// Фейд-ин: пары нет ПО МОДЕЛИ — слой уходит в прозрачность (под ним
/// чёрный фон композиции), а `complete` не портится: ждать нечего.
#[test]
fn t156_solo_fade_scales_alpha_and_stays_complete() {
    let mut p = project_pair();
    p.transitions.clear();
    p.transitions.push(TransitionEvent {
        id: TransitionId(2),
        a: None,
        b: Some(ClipId(1)),
        in_offset: Tick(0),
        out_offset: Tick(18_000), // фейд-ин: первые 0.2 c клипа
        kind: TransitionKind::Crossfade,
    });
    p.validate().unwrap();
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };

    for t in [0i64, 9_000, 17_999, 18_000] {
        let t = Tick(t);
        warm(&vr, &cache, t, q);
        let (out, complete) = vr.compose_from_cache(&cache, t, q);
        assert!(complete, "t {t:?}: фейд — не голод по кадру");
        let src = synth_frame(vr.grid.align_floor(t), q);
        let expect = if t.0 < 18_000 {
            // слой с альфой k/4096 поверх ЧЁРНОГО фона композиции
            let k = ((t.0 * 4096) / 18_000).clamp(0, 4096) as u32;
            over_black(&src, k)
        } else {
            src
        };
        assert_eq!(out, expect, "t {t:?}: фейд-ин");
    }
}

/// Фейд-аут — то же зеркально: к концу клипа картинка гаснет в чёрное.
#[test]
fn t156_fade_out_goes_dark_at_the_edge() {
    let mut p = project_pair();
    p.clips.retain(|c| c.id == ClipId(1));
    p.transitions.clear();
    p.transitions.push(TransitionEvent {
        id: TransitionId(3),
        a: Some(ClipId(1)),
        b: None,
        in_offset: Tick(18_000),
        out_offset: Tick(0),
        kind: TransitionKind::Crossfade,
    });
    p.validate().unwrap();
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };

    let mid = Tick(81_000); // середина окна [72000, 90000)
    warm(&vr, &cache, mid, q);
    let (out, complete) = vr.compose_from_cache(&cache, mid, q);
    assert!(complete);
    let src = synth_frame(vr.grid.align_floor(mid), q);
    assert_eq!(out, over_black(&src, 4096 - 2048), "в середине фейд-аута — половина яркости");

    // у самого края окна кадр почти погас
    let late = Tick(89_999);
    warm(&vr, &cache, late, q);
    let (out, _) = vr.compose_from_cache(&cache, late, q);
    let mean: f64 = out.chunks_exact(4).map(|p| p[0] as f64).sum::<f64>() / (out.len() / 4) as f64;
    assert!(mean < 1.0, "к концу окна фейд-аут гаснет, а вышло {mean}");
}

/// Слой с альфой k/4096 поверх ЧЁРНОГО прозрачного фона композиции —
/// дословно арифметика scale_alpha_into + alpha_over движка.
fn over_black(src: &[u8], k: u32) -> Vec<u8> {
    let mut out = vec![0u8; src.len()];
    for p in 0..src.len() / 4 {
        let i = p * 4;
        let sa = ((255 * k + 2048) >> 12).min(255);
        if sa == 255 {
            out[i..i + 4].copy_from_slice(&src[i..i + 4]);
            continue;
        }
        if sa == 0 {
            continue;
        }
        for ch in 0..3 {
            // фон нулевой (dst = 0, da = 0) → остаётся только вклад слоя
            out[i + ch] = ((src[i + ch] as u32 * sa) / 255).min(255) as u8;
        }
        out[i + 3] = sa as u8;
    }
    out
}

/// Тест 162 (видео-часть): при перекрытии на ОДНОЙ дорожке сверху остаётся
/// тот, у кого start меньше — тайбрейк v1, который использует и демоция
/// клипов при импорте .ktproj.
#[test]
fn t162_overlap_keeps_v1_tiebreak() {
    let mut p = project_pair();
    p.transitions.clear();
    // клип 2 наезжает на клип 1 на своей же дорожке
    for c in &mut p.clips {
        if c.id == ClipId(2) {
            c.start = Tick(45_000);
        }
    }
    p.validate().expect("перекрытие законно (переслаивание включено)");
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(60_000); // внутри зоны наезда
    warm(&vr, &cache, t, q);
    let (out, complete) = vr.compose_from_cache(&cache, t, q);
    assert!(complete);
    // сверху — клип 1 (start меньше): кадр равен его источнику
    let expect = synth_frame(vr.grid.align_floor(t), q);
    assert_eq!(out, expect, "в зоне наезда сверху клип с меньшим start");
}

/// Тест 174 (п.12 Р.28): уровень видео-события — непрозрачность слоя.
///
/// «в sony vegas… можно вроде прозрачность менять — мышкой наводишься и
/// тянешь вверх или вниз». Слой умножается на уровень: под полупрозрачным
/// клипом просвечивает нижняя дорожка, а её нет — чёрное.
#[test]
fn t174_clip_opacity_scales_the_layer() {
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(45_000);
    // два слоя: lane 0 (сверху) и lane 1 (снизу), РАЗНЫЕ источники
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
    for id in [1u64, 2] {
        p.media.push(MediaRef { id: MediaId(id), path: format!("synth:{id}") });
    }
    for (id, lane, media) in [(1u64, 0u32, 1u64), (2, 1, 2)] {
        p.clips.push(ClipEvent {
            id: ClipId(id),
            link_group: None,
            kind: ClipKind::Video,
            lane,
            start: Tick(0),
            duration: Tick(90_000),
            source_in: Tick(if id == 1 { 0 } else { 900_000 }),
            media: MediaId(media),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            opacity: 1.0,
        });
    }
    p.validate().unwrap();

    let render = |p: &Project| {
        let mut vr = VideoRenderer::new(p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        warm(&vr, &cache, t, q);
        let (out, complete) = vr.compose_from_cache(&cache, t, q);
        assert!(complete, "слои прогреты");
        out
    };

    let opaque = render(&p);
    let mut p0 = p.clone();
    p0.clips[0].opacity = 0.0;
    let transparent = render(&p0);
    let mut p5 = p.clone();
    p5.clips[0].opacity = 0.5;
    let half = render(&p5);

    assert_ne!(opaque, transparent, "уровень 0 обязан открыть нижний слой");
    // середина между слоями — попиксельно (alpha_over со straight alpha)
    for i in (0..opaque.len()).step_by(4) {
        for ch in 0..3 {
            let (a, b, m) =
                (opaque[i + ch] as i32, transparent[i + ch] as i32, half[i + ch] as i32);
            assert!(
                (m - (a + b) / 2).abs() <= 2,
                "пиксель {i} канал {ch}: уровень 0.5 — середина {m} между {a} и {b}"
            );
        }
    }
    // уровень 1.0 ничего не меняет — байт-в-байт прежний кадр (GOLDEN цел)
    let mut p1 = p.clone();
    p1.clips[0].opacity = 1.0;
    assert_eq!(render(&p1), opaque, "уровень 1.0 — тождество");
}
