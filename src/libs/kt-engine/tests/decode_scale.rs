//! Тесты 310-317 (Р.46, пп.320-322): СТУПЕНЬ ДЕКОДА — пре-FX кадр берётся
//! крупнее Quality, когда цепочка увеличивает и у файла есть запас натива.
//!
//! Три множителя ступени проверяются порознь (t310: формула), сэмплер
//! максимума зума по клипу — отдельно (t311), а главное следствие — заказ
//! префетча и чтение композита обязаны сойтись ВМЕСТЕ С РАЗМЕРОМ (t312):
//! разойдутся — вечный промах кэша, та же болезнь, что t247d лечил по оси
//! кадров.

use kt_engine::{VideoRenderer, video_cache_idx};
use kt_media::synthetic::CheckerSource;
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::{Tick, TickRange};

const SEC: i64 = 90_000;
const STEP: i64 = SEC / 30;

fn project(w: u32, h: u32, fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>) -> Project {
    let mut p = Project::new(ProjectSettings { fps: 30, width: w, height: h, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "checker".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(2 * SEC),
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

fn natives(w: u32, h: u32) -> std::collections::HashMap<MediaId, (u32, u32)> {
    std::collections::HashMap::from([(MediaId(1), (w, h))])
}

fn zoom(z: f64) -> FxInstance {
    FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: z }
}

/// Качество декода клипа (единственный слой проекта) при данном Quality.
fn dq(p: &Project, nat: Option<(u32, u32)>, q: Quality) -> Quality {
    let vr = match nat {
        Some((w, h)) => VideoRenderer::new_with_natives(p, &natives(w, h)),
        None => VideoRenderer::new(p),
    };
    vr.layers_at(Tick(STEP), q).first().expect("слой есть").2
}

/// Тест 310 (п.320): формула ступени. Сколько просит цепочка (квант тапов),
/// сколько может дать файл (запас натива, округление ВВЕРХ — решение
/// владельца) и потолок промежуточных буферов.
#[test]
fn t310_decode_stage_formula() {
    let q720 = Quality { width: 1280, height: 720 };
    let p2 = project(1280, 720, vec![zoom(2.0)], vec![]);
    // ДРОБНЫЙ запас: 1920/1280 = 1.5 → ступень 2, остаток дотянет sws
    assert_eq!(dq(&p2, Some((1920, 1080)), q720), Quality { width: 2560, height: 1440 });
    // полного запаса хватает ровно
    assert_eq!(dq(&p2, Some((2560, 1440)), q720), Quality { width: 2560, height: 1440 });
    // запаса нет: файл не крупнее Quality — декодируем как раньше
    assert_eq!(dq(&p2, Some((1280, 720)), q720), q720);
    assert_eq!(dq(&p2, Some((640, 360)), q720), q720, "мельче Quality не декодируем НИКОГДА");
    // натив неизвестен (заглушка, синтетика) — ступени нет
    assert_eq!(dq(&p2, None, q720), q720);
    // потолок промежуточных буферов: 4K-выход ступени не получает
    let q4k = Quality { width: 3840, height: 2160 };
    let p4 = project(3840, 2160, vec![zoom(2.0)], vec![]);
    assert_eq!(dq(&p4, Some((7680, 4320)), q4k), q4k, "MAX_INTER_DIM режет ступень");
    // цепочка не увеличивает — ступени нет ни при каком нативе
    for fx in [
        vec![],
        vec![zoom(1.0)],
        vec![zoom(0.5)],
        vec![FxInstance::Mirror { horizontal: true, vertical: false }],
        vec![FxInstance::EventGrid { cols: 2, rows: 2, center: true }],
        // нетто-единица (сценарий Р.44): сетка сжала, зум вернул
        vec![FxInstance::EventGrid { cols: 2, rows: 2, center: true }, zoom(2.0)],
    ] {
        let p = project(1280, 720, fx.clone(), vec![]);
        assert_eq!(dq(&p, Some((3840, 2160)), q720), q720, "нетто-зум <= 1: {fx:?}");
    }
    // зум чуть выше единицы гасится квантом, крупный — упирается в потолок тапов
    let p_small = project(1280, 720, vec![zoom(1.01)], vec![]);
    assert_eq!(dq(&p_small, Some((3840, 2160)), q720), q720, "квант 1/32 гасит дрожь");
    let q180 = Quality { width: 320, height: 180 };
    let p_big = project(1280, 720, vec![zoom(8.0)], vec![]);
    assert_eq!(
        dq(&p_big, Some((5120, 2880)), q180),
        Quality { width: 1280, height: 720 },
        "потолок ступени — 4"
    );
}

/// Тест 311 (п.320): сэмплер максимума зума по клипу. Ступень — по КЛИПУ
/// целиком (решение владельца), поэтому анимированный зум обязан ловиться
/// в своём максимуме, а не в момент первого кадра.
#[test]
fn t311_head_zoom_samples_the_whole_clip() {
    let q = Quality { width: 320, height: 180 };
    let nat = Some((1280, 720));
    let key = |t_s: f64, v: f64| AutoKey { t: Tick((t_s * SEC as f64) as i64), v };
    // зум 1 → 4 к концу клипа: на первом кадре цепочка ещё тождественна
    let curve = ParamCurve {
        param: "zoom".into(),
        ease: true,
        keys: vec![key(0.0, 1.0), key(2.0, 4.0)],
    };
    let p = project(320, 180, vec![zoom(1.0)], vec![vec![curve]]);
    assert_eq!(
        dq(&p, nat, q),
        Quality { width: 1280, height: 720 },
        "ступень берётся по максимуму кривой, а не по первому кадру"
    );
    // обратная кривая (зум в начале) — тот же максимум
    let back = ParamCurve {
        param: "zoom".into(),
        ease: true,
        keys: vec![key(0.0, 4.0), key(2.0, 1.0)],
    };
    let pb = project(320, 180, vec![zoom(1.0)], vec![vec![back]]);
    assert_eq!(dq(&pb, nat, q), Quality { width: 1280, height: 720 });
    // стоп силы слоя в нуле обнуляет зум префикса: ступени нет
    let mixed = project(
        320,
        180,
        vec![zoom(2.0), FxInstance::LayerMix { amount: 0.5, from: 0 }],
        vec![],
    );
    assert_eq!(dq(&mixed, nat, q), q, "смешение в нуле — цепочка префикса пуста");
    // реверс клипа не ломает ось: максимум тот же
    let mut pr = project(320, 180, vec![zoom(1.0)], vec![vec![ParamCurve {
        param: "zoom".into(),
        ease: true,
        keys: vec![key(0.0, 1.0), key(2.0, 4.0)],
    }]]);
    pr.clips[0].reversed = true;
    assert_eq!(dq(&pr, nat, q), Quality { width: 1280, height: 720 });
}

/// Тест 312 (п.321): ЗАКАЗ == ЧТЕНИЕ вместе с размером. Прогрев строго по
/// `wanted_frames` даёт полный композит; прогрев «как раньше», в Quality,
/// не даёт ничего — слой не находит своего кадра. Это t247d по оси
/// размера: разойдутся — вечный промах кэша.
#[test]
fn t312_order_equals_read_with_quality() {
    let q = Quality { width: 128, height: 72 };
    let p = project(128, 72, vec![zoom(2.0)], vec![]);
    let mut vr = VideoRenderer::new_with_natives(&p, &natives(512, 288));
    let range = TickRange::new(Tick(0), Tick(10 * STEP));
    let wanted = vr.wanted_frames(range, q);
    assert!(!wanted.is_empty());
    assert!(
        wanted.iter().all(|(_, _, dq)| *dq == Quality { width: 256, height: 144 }),
        "заказ обязан нести качество декода клипа"
    );

    // прогрев РОВНО по заказу
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut src = CheckerSource::new(Tick(10 * SEC), 512, 288);
    for (media, pts, dq) in &wanted {
        let f = src.read_video_frame(*pts, Some(*dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(*media),
            Some(*dq),
            video_cache_idx(*pts),
            f.into(),
        );
    }
    for i in 0..10 {
        let (_, complete) = vr.compose_from_cache(&cache, Tick(i * STEP), q);
        assert!(complete, "кадр {i}: композит читает то же, что грел префетч");
    }

    // прогрев в Quality (как до Р.46) — слой своего кадра не находит
    let cache_q = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    for (media, pts, _) in &wanted {
        let f = src.read_video_frame(*pts, Some(q)).unwrap();
        cache_q.insert_video(
            kt_engine::source_id(*media),
            Some(q),
            video_cache_idx(*pts),
            f.into(),
        );
    }
    let (_, complete) = vr.compose_from_cache(&cache_q, Tick(0), q);
    assert!(!complete, "кадр чужого качества не считается прогретым");
}

/// Тест 313 (п.321): ступень В КЛЮЧЕ пост-FX мемоизации. Один и тот же
/// клип на одном Quality из входа 1× и входа 2× даёт РАЗНЫЕ байты — без
/// ступени в ключе кэш отдал бы стейл.
#[test]
fn t313_postfx_key_includes_the_stage() {
    let q = Quality { width: 128, height: 72 };
    let p = project(128, 72, vec![zoom(2.0)], vec![]);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut src = CheckerSource::new(Tick(10 * SEC), 512, 288);
    let t = Tick(0);

    // кадры обоих качеств лежат в кэше одновременно (ключи разные)
    let mut vr1 = VideoRenderer::new(&p); // нативов нет → ступень 1
    let mut vr2 = VideoRenderer::new_with_natives(&p, &natives(512, 288));
    for vr in [&vr1, &vr2] {
        for (media, pts, dq) in vr.layers_at(t, q) {
            let f = src.read_video_frame(pts, Some(dq)).unwrap();
            cache.insert_video(
                kt_engine::source_id(media),
                Some(dq),
                video_cache_idx(pts),
                f.into(),
            );
        }
    }
    let (a, ca) = vr1.compose_from_cache(&cache, t, q);
    let (b, cb) = vr2.compose_from_cache(&cache, t, q);
    assert!(ca && cb, "оба композита полные");
    assert_ne!(a, b, "крупный вход обязан дать другую картинку, а не стейл из кэша");

    // повторный compose каждого — из мемоизации, байт-в-байт свой
    assert_eq!(vr1.compose_from_cache(&cache, t, q).0, a);
    assert_eq!(vr2.compose_from_cache(&cache, t, q).0, b);
}

/// Тест 314 (п.321): бюджет чанка экспорта считается по качеству ДЕКОДА.
/// Кадр ступени S занимает в кэше в S² раз больше, и чанк, посчитанный по
/// Quality, не дожил бы до композиции: LRU вытеснил бы пачку раньше, чем
/// её собрали, — ровно регрессия п.15 («чёрный рендер»).
#[test]
fn t314_export_chunk_respects_the_decode_stage() {
    // размеры подобраны так, что по Quality чанк влезает в бюджет, а по
    // качеству декода (×2 по стороне = ×4 памяти) — уже нет
    let q = Quality { width: 960, height: 540 };
    let dq = Quality { width: 1920, height: 1080 };
    let by_quality =
        kt_engine::export::export_chunk_frames(kt_engine::export::EXPORT_VIDEO_CACHE_BYTES, q.width, q.height, 1);
    let by_decode = kt_engine::export::export_chunk_frames(
        kt_engine::export::EXPORT_VIDEO_CACHE_BYTES,
        dq.width,
        dq.height,
        1,
    );
    assert!(by_decode * 4 <= by_quality, "чанк обязан падать вчетверо при ступени 2");

    // и живой рендер со ступенью доходит без пропавших слоёв
    let p = project(128, 72, vec![zoom(2.0)], vec![]);
    let sources: Vec<(MediaId, Box<dyn MediaSource>)> =
        vec![(MediaId(1), Box::new(CheckerSource::new(Tick(10 * SEC), 512, 288)))];
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut frames = 0u64;
    let stats = kt_engine::export::render_raw_video_range_with(
        &p,
        sources,
        Quality { width: 128, height: 72 },
        Some((Tick(0), Tick(30 * STEP))),
        &Default::default(),
        &cancel,
        |_| {},
        |_| {
            frames += 1;
            Ok(())
        },
    )
    .expect("render");
    assert_eq!(frames, 30, "все кадры диапазона отрендерены");
    assert_eq!(stats.missing_video_layers, 0, "ни один слой не потерялся на ступени");
}

/// Тест 317 (п.322): ПОЛОСКИ прогретости спрашивают качество ДЕКОДА.
/// Кэш качество-точный (t8), поэтому клип со ступенью, прогретый как
/// надо, при опросе по Quality выглядел бы полностью холодным —
/// нарушение инварианта №4 в глазах пользователя.
#[test]
fn t317_coverage_is_queried_at_the_decode_quality() {
    let q = Quality { width: 128, height: 72 };
    let p = project(128, 72, vec![zoom(2.0)], vec![]);
    let vr = VideoRenderer::new_with_natives(&p, &natives(512, 288));
    let dq_clip = vr.video_decode_qualities(q);
    assert_eq!(dq_clip, vec![(ClipId(1), Quality { width: 256, height: 144 })]);

    // префетч кладёт кадры строго по заказу
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut src = CheckerSource::new(Tick(10 * SEC), 512, 288);
    for (media, pts, dq) in vr.wanted_frames(TickRange::new(Tick(0), Tick(10 * STEP)), q) {
        let f = src.read_video_frame(pts, Some(dq)).unwrap();
        cache.insert_video(kt_engine::source_id(media), Some(dq), video_cache_idx(pts), f.into());
    }
    let sid = kt_engine::source_id(MediaId(1));
    let range = 0..(10 * STEP);
    let warm = cache.coverage(sid, kt_cache::BurstKind::video(Some(dq_clip[0].1)), range.clone());
    let cold = cache.coverage(sid, kt_cache::BurstKind::video(Some(q)), range);
    assert!(!warm.is_empty(), "по качеству декода клип прогрет");
    assert!(cold.is_empty(), "по Quality того же клипа кадров нет — полоска соврала бы");
}
