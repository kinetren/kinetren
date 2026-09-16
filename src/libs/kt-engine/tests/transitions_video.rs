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
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
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
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick(9_000),
        out_offset: Tick(9_000),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    p
}

fn warm(vr: &VideoRenderer, cache: &kt_cache::BurstCache, t: Tick, q: kt_media::Quality) {
    use kt_media::MediaSource;
    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence);
    for (media, src_pts, q) in vr.layers_at(t, q) {
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
    a.iter()
        .zip(b)
        .map(|(&av, &bv)| ((av as u32 * ik + bv as u32 * k + 2048) >> 12) as u8)
        .collect()
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
    let q = kt_media::Quality { width: 64, height: 36 };
    // в окне перехода активны ОБА клипа (ручки), вне — один
    assert_eq!(vr.layers_at(Tick(84_000), q).len(), 2, "handles active inside window");
    assert_eq!(vr.layers_at(Tick(78_000), q).len(), 1);
    assert_eq!(vr.layers_at(Tick(99_000), q).len(), 1);
    // левая ручка B читает источник ДО куска: src = 30000 + (84000-90000)
    let layers = vr.layers_at(Tick(84_000), q);
    assert!(layers.contains(&(MediaId(2), vr.grid.align_floor(Tick(24_000)), q)));
}

#[test]
fn t137_dissolve_survives_fx_chain_on_pair() {
    // FX-клип в паре: пост-FX кэш + бленд; повторный compose из кэша
    // байт-идентичен (наследие t66)
    let mut p = project_pair();
    p.clips[0].fx = vec![FxInstance::noise(0.3)];
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
        audio: Default::default(),
        auto: vec![],
        invert: false,
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
        audio: Default::default(),
        auto: vec![],
        invert: false,
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

/// Тест 162 (видео-часть): при перекрытии на ОДНОЙ дорожке сверху тот, кто
/// начался ПОЗЖЕ — правило Vegas (решение пользователя после Р.28: «давай
/// сделаем как в vegas — сверху тот кто позже»; до этого действовало
/// правило v1 «сверху тот, у кого start меньше»).
#[test]
fn t162_overlap_puts_the_later_clip_on_top() {
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
    // сверху — клип 2 (начался позже): кадр равен ЕГО источнику
    // (source_in 30_000, старт 45_000 → pts = 30_000 + (t − 45_000))
    let expect = synth_frame(vr.grid.align_floor(Tick(30_000 + (t.0 - 45_000))), q);
    assert_eq!(out, expect, "в зоне наезда сверху клип, начавшийся позже");
    // и это НЕ кадр раннего клипа — правило действительно сменилось
    assert_ne!(out, synth_frame(vr.grid.align_floor(t), q), "ранний клип больше не сверху");
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
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
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
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
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

/// Тест 225 (Р.33 С4): вид перехода доезжает до КАРТИНКИ, а не только до
/// файла. Кадры синтетического источника — сплошной цвет, поэтому
/// «вайп» обязан дать ровно две однотонные области с резкой границей,
/// а кроссфейд в тот же момент — один смешанный цвет на весь кадр.
#[test]
fn t225_transition_kind_reaches_the_picture() {
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(90_000); // середина окна 81_000..99_000
    let render = |kind: TransitionKind| {
        let mut p = project_pair();
        p.transitions[0].kind = kind.clone();
        p.validate().unwrap();
        let mut vr = VideoRenderer::new(&p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        warm(&vr, &cache, t, q);
        let (out, complete) = vr.compose_from_cache(&cache, t, q);
        assert!(complete, "{kind:?}: слои прогреты");
        // детерминизм: повторный проход байт-в-байт
        let (again, _) = vr.compose_from_cache(&cache, t, q);
        assert_eq!(out, again, "{kind:?}: повторный composite обязан совпасть");
        out
    };
    let px = |buf: &[u8], x: usize, y: usize| {
        let i = (y * q.width as usize + x) * 4;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    };
    let a = synth_frame(Tick(90_000), q);
    let b = synth_frame(Tick(30_000), q);
    let (ca, cb) = (px(&a, 0, 0), px(&b, 0, 0));
    assert_ne!(ca, cb, "цвета половин обязаны различаться, иначе тест слеп");

    // кроссфейд: смесь ВЕЗДЕ, обеих исходных красок в кадре нет
    let cross = render(TransitionKind::Crossfade);
    let mix = px(&cross, 5, 5);
    assert_eq!(mix, px(&cross, 58, 30), "кроссфейд однороден по кадру");
    assert_ne!(mix, ca);
    assert_ne!(mix, cb);

    // вайп влево без размытия: B открывается СПРАВА, граница резкая
    let wipe = render(TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.0 });
    assert_eq!(px(&wipe, 2, 18), ca, "слева ещё уходящий кадр");
    assert_eq!(px(&wipe, 61, 18), cb, "справа уже входящий");
    let blended = (0..q.width as usize)
        .filter(|&x| {
            let p = px(&wipe, x, 18);
            p != ca && p != cb
        })
        .count();
    assert!(blended <= 1, "у резкого вайпа смешанных пикселей нет: {blended}");

    // шторки по вертикали: одинаково у ОБОИХ краёв, чужой цвет в центре
    let barn = render(TransitionKind::Barn { vertical: false, softness: 0.0 });
    assert_eq!(px(&barn, 32, 1), ca, "у верхней кромки ещё уходящий");
    assert_eq!(px(&barn, 32, 34), ca, "и у нижней тоже — створки симметричны");
    assert_eq!(px(&barn, 32, 18), cb, "в центре уже входящий");

    // сдвиг вниз: половины меняются местами по вертикали
    let push = render(TransitionKind::Push { dir: Dir::Down });
    assert_eq!(px(&push, 32, 30), ca, "низ — уходящий, его вытолкнуло вниз");
    assert_eq!(px(&push, 32, 4), cb, "сверху въехал входящий");

    // через чёрный: в середине перехода кадр гаснет целиком
    let dip = render(TransitionKind::Dip { color_r: 0, color_g: 0, color_b: 0 });
    let mid = px(&dip, 32, 18);
    assert!(mid[0] < 4 && mid[1] < 4 && mid[2] < 4, "середина dip'а — цвет: {mid:?}");
    assert_eq!(mid[3], 255, "альфа при dip'е не теряется");
}

/// Тест 225b: сольный фейд (край дорожки) с видом — форма живёт в АЛЬФЕ,
/// а не в равномерной прозрачности. Кроссфейд-фейд остаётся прежним.
#[test]
fn t225_solo_fade_with_kind_shapes_alpha() {
    let q = kt_media::Quality { width: 64, height: 36 };
    let render = |kind: TransitionKind, t: Tick| {
        let mut p = project_pair();
        // убираем пару: у клипа 1 остаётся фейд-аут в пустоту
        p.transitions.clear();
        p.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(ClipId(1)),
            b: None,
            in_offset: Tick(18_000),
            out_offset: Tick(0),
            kind,
            audio: Default::default(),
            auto: vec![],
            invert: false,
        });
        p.clips.retain(|c| c.id == ClipId(1));
        p.validate().unwrap();
        let mut vr = VideoRenderer::new(&p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        warm(&vr, &cache, t, q);
        let (out, _) = vr.compose_from_cache(&cache, t, q);
        out
    };
    let alpha = |buf: &[u8], x: usize, y: usize| buf[(y * q.width as usize + x) * 4 + 3];
    let t = Tick(81_000); // середина окна 72_000..90_000

    // кроссфейд-фейд: альфа ОДНА на весь кадр (прежнее поведение)
    let cross = render(TransitionKind::Crossfade, t);
    assert_eq!(alpha(&cross, 2, 18), alpha(&cross, 61, 18), "фейд однороден");

    // вайп-фейд: одна сторона ещё непрозрачна, другая уже пуста
    let wipe = render(TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.0 }, t);
    let (l, r) = (alpha(&wipe, 2, 18), alpha(&wipe, 61, 18));
    assert!(l > 250 && r < 5, "вайп-фейд обязан резать кадр по альфе: {l}/{r}");
}

/// Тест 232 (Р.34 К4): параметры вида перехода анимируются тем же
/// механизмом, что и у эффектов. Ключи живут в тиках ОТ НАЧАЛА окна;
/// вид разрешается на каждый момент, поэтому картинка обязана меняться
/// не только по прогрессу перехода, но и по кривой.
#[test]
fn t232_transition_params_animate() {
    let q = kt_media::Quality { width: 64, height: 36 };
    let render = |auto: Vec<ParamCurve>, t: Tick| {
        let mut p = project_pair();
        // вайп без размытия: угол кромки виден по форме границы
        p.transitions[0].kind = TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.0 };
        p.transitions[0].auto = auto;
        p.validate().unwrap();
        let mut vr = VideoRenderer::new(&p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        warm(&vr, &cache, t, q);
        let (out, complete) = vr.compose_from_cache(&cache, t, q);
        assert!(complete, "слои прогреты");
        out
    };
    // окно перехода — [81_000, 99_000): кривая угла от 0° к 80°
    let curve = vec![ParamCurve {
        param: "angle".into(),
        ease: false,
        keys: vec![AutoKey { t: Tick(0), v: 0.0 }, AutoKey { t: Tick(18_000), v: 80.0 }],
    }];
    let t = Tick(96_000); // ближе к концу окна — угол уже большой
    let plain = render(vec![], t);
    let animated = render(curve.clone(), t);
    assert_ne!(plain, animated, "кривая угла обязана менять картинку");

    // в САМОМ начале окна кривая ещё в нуле — кадр как без неё
    let t0 = Tick(81_000);
    assert_eq!(
        render(vec![], t0),
        render(curve.clone(), t0),
        "в начале окна значение кривой равно статическому"
    );

    // ключ вне окна законен (спрятан правкой края), загрузка не падает
    let mut p = project_pair();
    p.transitions[0].kind = TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.0 };
    p.transitions[0].auto = vec![ParamCurve {
        param: "angle".into(),
        ease: false,
        keys: vec![AutoKey { t: Tick(0), v: 0.0 }, AutoKey { t: Tick(900_000), v: 80.0 }],
    }];
    p.validate().expect("ключ за краем окна — законный спрятанный ключ");

    // кривая на чужой параметр — отказ валидации, а не тишина
    p.transitions[0].auto[0].param = "no_such".into();
    assert!(p.validate().is_err(), "кривая-сирота обязана валить загрузку");
}

/// Тест 459 (Р.51, п.396): у направленных ядер ожидание считается
/// ФОРМУЛОЙ, а не вторым экземпляром той же формулы. Кадры — линейные
/// рампы (R по X, G по Y, B — маркер источника): билинейка на рампе при
/// целом сдвиге вырождается в точное значение, и «въехавшая» часть
/// обязана показывать НАСТОЯЩИЕ строки/колонки кадра B, а не размазанную
/// кромку. Именно эту слепоту имели t224 (зеркало против зеркала) и
/// t225 (сплошные цвета): push/whip сэмплировали B со сдвигом −dir
/// вместо +dir, кламп у края превращал B в полосу его первой
/// строки/колонки — и оба теста оставались зелёными.
#[test]
fn t459_push_whip_bring_real_b_content() {
    use kt_engine::transitions::{mix_into, TransMix};
    const W: usize = 64;
    const H: usize = 36;
    let ramp = |marker: u8| -> kt_media::VideoFrame {
        let mut rgba = vec![0u8; W * H * 4];
        for y in 0..H {
            for x in 0..W {
                let i = (y * W + x) * 4;
                rgba[i] = (x * 4) as u8; // R: рампа по X (0..252)
                rgba[i + 1] = (y * 7) as u8; // G: рампа по Y (0..245)
                rgba[i + 2] = marker; // B: чей кадр
                rgba[i + 3] = 255;
            }
        }
        kt_media::VideoFrame {
            pts: Tick(0),
            width: W as u32,
            height: H as u32,
            rgba: rgba.into_boxed_slice(),
            has_alpha: false,
        }
    };
    let a = ramp(50);
    let b = ramp(200);

    // Геометрия push: экран показывает ленту [A|B], сдвинутую на dir·k.
    // Тексел (x,y) берёт материал в (x,y) − shift; вне кадра — кадр B,
    // перенесённый на целый экран ПО НАПРАВЛЕНИЮ движения.
    let expect = |x: i32, y: i32, dx: i32, dy: i32, shift_x: i32, shift_y: i32| -> [u8; 3] {
        let (mx, my) = (x - shift_x, y - shift_y);
        let inside = (0..W as i32).contains(&mx) && (0..H as i32).contains(&my);
        let (sx, sy, marker) = if inside {
            (mx, my, 50)
        } else {
            (mx + dx * W as i32, my + dy * H as i32, 200)
        };
        [(sx * 4) as u8, (sy * 7) as u8, marker]
    };

    let mut dst = Vec::new();
    for dir in [Dir::Left, Dir::Right, Dir::Up, Dir::Down] {
        let (dx, dy) = dir.vec();
        let (dx, dy) = (dx as i32, dy as i32);
        for k in [1024u32, 2048] {
            // сдвиг при этих k целый и по X (64·k/4096), и по Y (36·k/4096)
            let kk = k as f32 / 4096.0;
            let (shift_x, shift_y) =
                ((dx as f32 * kk * W as f32) as i32, (dy as f32 * kk * H as f32) as i32);
            let m = TransMix::of(&TransitionKind::Push { dir });
            mix_into(&mut dst, Some(&a), Some(&b), k, m, W, H);
            for y in 0..H as i32 {
                for x in 0..W as i32 {
                    let i = ((y * W as i32 + x) * 4) as usize;
                    let want = expect(x, y, dx, dy, shift_x, shift_y);
                    for c in 0..3 {
                        let got = dst[i + c] as i32;
                        assert!(
                            (got - want[c] as i32).abs() <= 1,
                            "push {dir:?} k={k}: пиксель ({x},{y}) канал {c}: \
                             {got}, а материал там {}",
                            want[c]
                        );
                    }
                }
            }
            // whip с нулевым смазом — ровно push: 7 тапов в одной точке
            let mw = TransMix::of(&TransitionKind::Whip { dir, blur: 0.0 });
            let mut dw = Vec::new();
            mix_into(&mut dw, Some(&a), Some(&b), k, mw, W, H);
            assert_eq!(dw, dst, "whip {dir:?} blur=0 k={k} обязан совпасть с push байт-в-байт");
        }
    }
}

/// Тест 459b: та же геометрия доезжает через ПЛАН (VideoRenderer), а не
/// только через голое ядро — кэш прогревается рамповыми кадрами, и
/// въехавшая половина обязана показать середину кадра B.
#[test]
fn t459b_push_through_renderer_shows_real_b() {
    let q = kt_media::Quality { width: 64, height: 36 };
    let ramp_rgba = |marker: u8| -> Vec<u8> {
        let (w, h) = (64usize, 36usize);
        let mut rgba = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let i = (y * w + x) * 4;
                rgba[i] = (x * 4) as u8;
                rgba[i + 1] = (y * 7) as u8;
                rgba[i + 2] = marker;
                rgba[i + 3] = 255;
            }
        }
        rgba
    };
    let mut p = project_pair();
    p.transitions[0].kind = TransitionKind::Push { dir: Dir::Down };
    p.validate().unwrap();
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let t = Tick(90_000); // середина окна: сдвиг ровно полкадра
    for (media, src_pts, fq) in vr.layers_at(t, q) {
        let marker = if kt_engine::source_id(media) == kt_engine::source_id(MediaId(1)) {
            50
        } else {
            200
        };
        let frame = kt_media::VideoFrame {
            pts: src_pts,
            width: fq.width,
            height: fq.height,
            rgba: ramp_rgba(marker).into_boxed_slice(),
            has_alpha: false,
        };
        cache.insert_video(
            kt_engine::source_id(media),
            Some(fq),
            kt_engine::video_cache_idx(src_pts),
            std::sync::Arc::new(frame),
        );
    }
    let (out, complete) = vr.compose_from_cache(&cache, t, q);
    assert!(complete, "слои прогреты");
    let px = |x: usize, y: usize| {
        let i = (y * 64 + x) * 4;
        [out[i], out[i + 1], out[i + 2]]
    };
    // push Down, k=0.5: верхняя половина экрана — НИЖНЯЯ половина кадра B
    assert_eq!(px(32, 4), [128, (4 + 18) * 7, 200], "въехал настоящий B, а не кромка");
    // нижняя половина — верх кадра A, уехавший вниз
    assert_eq!(px(32, 30), [128, (30 - 18) * 7, 50], "A уезжает вниз своим верхом");
}

/// Тест 461 (п.398, Р.51): инверсия перехода — «жест задом наперёд»,
/// исполняется планом как swap сторон + обратный прогресс. Кроссфейд —
/// тождество (целочисленный путь не трогается), у направленных видов
/// инверсия совпадает с противоположным направлением, у соло-фейда
/// форма режется с другой стороны.
#[test]
fn t461_transition_invert() {
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(90_000); // середина окна, k = 0.5
    let render = |kind: TransitionKind, invert: bool| {
        let mut p = project_pair();
        p.transitions[0].kind = kind;
        p.transitions[0].invert = invert;
        p.validate().unwrap();
        let mut vr = VideoRenderer::new(&p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        warm(&vr, &cache, t, q);
        let (out, complete) = vr.compose_from_cache(&cache, t, q);
        assert!(complete, "слои прогреты");
        out
    };
    let px = |buf: &[u8], x: usize, y: usize| {
        let i = (y * q.width as usize + x) * 4;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    };
    let a = synth_frame(Tick(90_000), q);
    let b = synth_frame(Tick(30_000), q);
    let (ca, cb) = (px(&a, 0, 0), px(&b, 0, 0));
    assert_ne!(ca, cb, "цвета сторон обязаны различаться, иначе тест слеп");

    // кроссфейд: инверсия — тождество, байт-в-байт (GOLDEN-путь не тронут)
    assert_eq!(
        render(TransitionKind::Crossfade, false),
        render(TransitionKind::Crossfade, true),
        "инверсия кроссфейда обязана быть тождеством"
    );

    // вайп: инверсия меняет стороны у той же границы
    let wipe = |inv| render(TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.0 }, inv);
    let (w0, w1) = (wipe(false), wipe(true));
    assert_eq!(px(&w0, 2, 18), ca, "прямой вайп: слева уходящий");
    assert_eq!(px(&w0, 61, 18), cb, "прямой вайп: справа входящий");
    assert_eq!(px(&w1, 2, 18), cb, "инверсный вайп: слева уже входящий");
    assert_eq!(px(&w1, 61, 18), ca, "инверсный вайп: справа ещё уходящий");

    // push: инверсия направленного вида == противоположное направление
    assert_eq!(
        render(TransitionKind::Push { dir: Dir::Right }, true),
        render(TransitionKind::Push { dir: Dir::Left }, false),
        "инверсный push вправо обязан совпасть с push влево"
    );

    // соло-фейд: форма режется с другой стороны
    let solo = |inv: bool| {
        let mut p = project_pair();
        p.transitions.clear();
        p.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(ClipId(1)),
            b: None,
            in_offset: Tick(18_000),
            out_offset: Tick(0),
            kind: TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.0 },
            audio: Default::default(),
            auto: vec![],
            invert: inv,
        });
        p.clips.retain(|c| c.id == ClipId(1));
        p.validate().unwrap();
        let mut vr = VideoRenderer::new(&p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        let t = Tick(81_000); // середина окна фейда 72_000..90_000
        warm(&vr, &cache, t, q);
        let (out, _) = vr.compose_from_cache(&cache, t, q);
        out
    };
    let alpha = |buf: &[u8], x: usize| buf[(18 * q.width as usize + x) * 4 + 3];
    let (s0, s1) = (solo(false), solo(true));
    assert!(
        alpha(&s0, 2) > 250 && alpha(&s0, 61) < 5,
        "прямой вайп-фейд режет справа: {}/{}",
        alpha(&s0, 2),
        alpha(&s0, 61)
    );
    assert!(
        alpha(&s1, 2) < 5 && alpha(&s1, 61) > 250,
        "инверсный вайп-фейд режет слева: {}/{}",
        alpha(&s1, 2),
        alpha(&s1, 61)
    );
}

/// Тест 464 (п.399, Р.51): картинка на НАЛОЖЕНИИ. Без перехода — прежний
/// жёсткий срез Vegas (поздний сверху непрозрачен); с переходом пара
/// схлопывается в Dissolve, и вид доезжает до кадра.
#[test]
fn t464_overlap_transition_reaches_picture() {
    let q = kt_media::Quality { width: 64, height: 36 };
    let mk = |with: Option<TransitionKind>| {
        let mut p = Project::new(ProjectSettings {
            fps: 30,
            width: 640,
            height: 360,
            sample_rate: 48000,
            pitch_engine: Default::default(),
        });
        for id in [1u64, 2] {
            p.media.push(MediaRef { id: MediaId(id), path: format!("synth:{id}") });
        }
        // лесенка: [0..120к) и [60к..180к), перекрытие [60к..120к)
        for (id, start, media) in [(1u64, 0i64, 1u64), (2, 60_000, 2)] {
            p.clips.push(ClipEvent {
                id: ClipId(id),
                link_group: None,
                kind: ClipKind::Video,
                lane: 0,
                start: Tick(start),
                duration: Tick(120_000),
                source_in: Tick::ZERO,
                media: MediaId(media),
                gain_db: 0.0,
                reversed: false,
                fx: vec![],
                fx_bypass: vec![],
                fx_auto: vec![],
                opacity: 1.0,
                fx_off: false,
            });
        }
        if let Some(kind) = with {
            p.transitions.push(TransitionEvent {
                id: TransitionId(1),
                a: Some(ClipId(1)),
                b: Some(ClipId(2)),
                in_offset: Tick::ZERO,
                out_offset: Tick::ZERO,
                kind,
                audio: Default::default(),
                auto: vec![],
                invert: false,
            });
        }
        p.validate().unwrap();
        p
    };
    let render = |p: &Project, t: Tick| {
        let mut vr = VideoRenderer::new(p);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        warm(&vr, &cache, t, q);
        let (out, complete) = vr.compose_from_cache(&cache, t, q);
        assert!(complete, "слои прогреты");
        out
    };
    let px = |buf: &[u8], x: usize, y: usize| {
        let i = (y * q.width as usize + x) * 4;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    };
    let t = Tick(90_000); // середина перекрытия
    let a = synth_frame(Tick(90_000), q);
    let b = synth_frame(Tick(30_000), q);
    let (ca, cb) = (px(&a, 0, 0), px(&b, 0, 0));
    assert_ne!(ca, cb, "цвета сторон различимы");

    // без перехода: жёсткий срез — весь кадр показывает ПОЗДНИЙ клип
    let bare = render(&mk(None), t);
    assert_eq!(px(&bare, 2, 18), cb, "поздний сверху непрозрачен (правило Vegas)");
    assert_eq!(px(&bare, 61, 18), cb);

    // с вайпом: обе стороны в кадре, граница в середине окна
    let wipe = render(
        &mk(Some(TransitionKind::Wipe { dir: Dir::Left, angle: 0.0, softness: 0.0 })),
        t,
    );
    assert_eq!(px(&wipe, 2, 18), ca, "слева ещё уходящий");
    assert_eq!(px(&wipe, 61, 18), cb, "справа уже входящий");

    // кроссфейд: однородная смесь по кадру
    let cross = render(&mk(Some(TransitionKind::Crossfade)), t);
    let mix = px(&cross, 5, 5);
    assert_eq!(mix, px(&cross, 58, 30), "кроссфейд однороден");
    assert_ne!(mix, ca);
    assert_ne!(mix, cb);
}
