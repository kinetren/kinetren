//! Тест 66 (п.2 р.7): пост-FX кэш видео — повторный compose байт-идентичен
//! и идёт из кэша (без повторного применения цепочки), смена параметров FX
//! или геометрии клипа даёт другой fx_hash → стейл не читается.
//! Тест 67 — в golden_export.rs (превью==экспорт с FX, холодный/прогретый).

use kt_engine::VideoRenderer;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;

fn project_with_fx(fx: Vec<FxInstance>, start_frames: i64) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    
    default_shape: Default::default(),});
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick::from_audio_frames(start_frames),
        duration: Tick::from_audio_frames(48_000 * 2),
        source_in: Tick::from_audio_frames(9_600),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn warm_pre_fx(vr: &VideoRenderer, cache: &kt_cache::BurstCache, t: Tick, q: kt_media::Quality) {
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

#[test]
fn t66_postfx_cached_and_no_stale() {
    // Noise: амплитуда меняет байты даже на плоском синтетическом кадре
    // (SyntheticSource кодирует pts сплошным цветом — Mirror/Vhs на нём
    // вырождаются в identity, стейл по байтам не отличить)
    let chain =
        vec![FxInstance::Mirror { horizontal: true, vertical: false }, FxInstance::noise(0.3)];
    let p = project_with_fx(chain.clone(), 48_000);
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(90_000 + 3000);
    warm_pre_fx(&vr, &cache, t, q);

    let (a, complete) = vr.compose_from_cache(&cache, t, q);
    assert!(complete);
    let bytes_after_first = cache.video_bytes();
    let hits_after_first = cache.stats().video_hits;

    // повторный compose: байт-идентичен, из кэша (байты не растут — нет
    // повторной вставки), пост-FX хит зарегистрирован
    let (b, complete2) = vr.compose_from_cache(&cache, t, q);
    assert!(complete2);
    assert_eq!(a, b, "повторный compose байт-идентичен");
    assert_eq!(cache.video_bytes(), bytes_after_first, "хит: без повторной вставки");
    assert!(cache.stats().video_hits > hits_after_first, "пост-FX читался из кэша");

    // смена ПАРАМЕТРА цепочки → другой fx_hash → пересчёт (байты кэша
    // растут: вставка нового ключа, а не стейл-хит) и другой результат
    let p2 = project_with_fx(
        vec![FxInstance::Mirror { horizontal: true, vertical: false }, FxInstance::noise(0.9)],
        48_000,
    );
    let mut vr2 = VideoRenderer::new(&p2);
    let (c, _) = vr2.compose_from_cache(&cache, t, q);
    assert_ne!(a, c, "изменённые параметры FX не должны читать стейл");
    assert!(cache.video_bytes() > bytes_after_first, "новый fx_hash → новая запись");

    // сдвиг клипа при той же цепочке → другой fx_hash (u/t-отображение
    // изменилось) → пересчёт со вставкой, а не стейл-хит
    let bytes_before_shift = cache.video_bytes();
    let p3 = project_with_fx(chain, 48_000 - 4800); // t остаётся внутри клипа
    let mut vr3 = VideoRenderer::new(&p3);
    warm_pre_fx(&vr3, &cache, t, q);
    let bytes_warmed = cache.video_bytes();
    assert!(bytes_warmed >= bytes_before_shift);
    let (_, complete3) = vr3.compose_from_cache(&cache, t, q);
    assert!(complete3);
    assert!(
        cache.video_bytes() > bytes_warmed,
        "сдвинутый клип обязан пересчитать пост-FX (другой fx_hash), не взять стейл"
    );
}

/// Пост-FX хит делает слой complete даже когда пре-FX кадр уже вытеснен:
/// прогреваем, выбиваем пре-FX перезаписью бюджета... проще: новый кэш
/// с только-пост-FX записью не собрать снаружи, поэтому проверяем слабее —
/// удаление пре-FX через invalidate_source сносит И пост-FX (нет осиротевших
/// стейлов при смене файла на диске).
#[test]
fn t66b_invalidate_source_drops_postfx() {
    let p = project_with_fx(vec![FxInstance::glow(1.0)], 48_000);
    let mut vr = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(90_000 + 3000);
    warm_pre_fx(&vr, &cache, t, q);
    let (_, complete) = vr.compose_from_cache(&cache, t, q);
    assert!(complete);
    assert!(cache.video_bytes() > 0);
    cache.invalidate_source(kt_engine::source_id(MediaId(1)));
    assert_eq!(cache.video_bytes(), 0, "invalidate снёс и пре-FX, и пост-FX");
    let (_, complete_cold) = vr.compose_from_cache(&cache, t, q);
    assert!(!complete_cold, "после инвалидации слоёв нет");
}

/// Р.31 п.4: экспортный composе НЕ мемоизирует пост-FX кадры — на
/// многослойном монтаже они вытесняли ещё не использованные пре-кадры
/// соседних дорожек из экспортного кэша, и рендер терял слои. Байты кадра
/// при этом обязаны совпадать с превьюшным compose.
#[test]
fn t66c_export_compose_skips_postfx_memoization() {
    let p = project_with_fx(
        vec![FxInstance::Mirror { horizontal: true, vertical: false }, FxInstance::noise(0.3)],
        0,
    );
    let q = kt_media::Quality { width: 640, height: 360 };
    let t = Tick(0);

    let cache_a = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut vr_a = VideoRenderer::new(&p);
    warm_pre_fx(&vr_a, &cache_a, t, q);
    let bytes_before = cache_a.video_bytes();
    let mut out_a = Vec::new();
    let complete = vr_a.compose_export_into(&cache_a, t, q, &mut out_a);
    assert!(complete);
    assert_eq!(
        cache_a.video_bytes(),
        bytes_before,
        "экспортный compose не имеет права раздувать кэш пост-FX кадрами"
    );

    // превьюшный compose на отдельном кэше — байты те же, но мемоизация есть
    let cache_b = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut vr_b = VideoRenderer::new(&p);
    warm_pre_fx(&vr_b, &cache_b, t, q);
    let mut out_b = Vec::new();
    vr_b.compose_from_cache_into(&cache_b, t, q, &mut out_b);
    assert_eq!(out_a, out_b, "экспортный и превьюшный compose обязаны совпадать байт-в-байт");
    assert!(cache_b.video_bytes() > bytes_before, "превью мемоизирует как раньше");
}

/// Тест 66d (Р.44): КАМЕРА — в ключе пост-FX мемоизации. CPU считает
/// Pan3d, а камера разрешается по-кадрово (правило активной камеры,
/// анимация) — статический fx_hash её не видит: без камеры в ключе правка
/// или перенос камеры отдавали бы стейл-кадр (GPU от этого защищён
/// put_cam3d в plan_key с Р.29).
#[test]
fn t66d_camera_edit_invalidates_postfx() {
    let pan3d = FxInstance::Pan3d {
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x: 0.0,
        rot_y: 0.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale: 0.5, // прозрачные поля: сдвиг квада виден и на плоском кадре
        camera: None,
    };
    let cam_at = |pos_x: f64, start: i64| CameraEvent {
        ease: true,
        id: CameraId(1),
        lane: 0,
        start: Tick(start),
        duration: Tick(90_000 * 4),
        name: String::new(),
        cam: CameraParams { pos_x, ..CameraParams::default() },
        keyframes: vec![],
        auto: vec![],
    };
    let with_cam = |cam: CameraEvent| {
        let mut p = project_with_fx(vec![pan3d.clone()], 48_000);
        p.camera_lanes = 1;
        p.cameras = vec![cam];
        p.validate().unwrap();
        p
    };
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };
    let t = Tick(90_000 + 3000);

    let p1 = with_cam(cam_at(0.0, 0));
    let mut vr1 = VideoRenderer::new(&p1);
    warm_pre_fx(&vr1, &cache, t, q);
    let (a, c1) = vr1.compose_from_cache(&cache, t, q);
    assert!(c1);

    // правка ПАРАМЕТРА камеры (fx_hash клипа НЕ меняется, кэш пережил
    // пересоздание рендерера) → кадр обязан пересчитаться
    let p2 = with_cam(cam_at(20.0, 0));
    let mut vr2 = VideoRenderer::new(&p2);
    let (b, c2) = vr2.compose_from_cache(&cache, t, q);
    assert!(c2);
    assert_ne!(a, b, "правка камеры не должна читать стейл пост-FX");

    // ПЕРЕНОС камерного события за пределы t: активной камеры нет —
    // дефолтная «50 мм»; кадр снова другой (не стейл от p2)
    let p3 = with_cam(cam_at(20.0, 90_000 * 10));
    let mut vr3 = VideoRenderer::new(&p3);
    let (c, c3) = vr3.compose_from_cache(&cache, t, q);
    assert!(c3);
    assert_ne!(b, c, "перенос камеры (смена активной) не должен читать стейл");
    // дефолтная камера == камера p1 (pos_x 0) → кадры равны, И это хит
    // мемоизации (тот же ключ) — камера в ключе не мешает переиспользованию
    assert_eq!(a, c, "дефолтная камера воспроизводит p1");
}
