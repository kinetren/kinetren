//! Тест 184 (Р.29): GPU-твин теста 67 — «превью == экспорт» на GPU-пути.
//! Кадр экспортного потока (render_raw_video_range_with + GPU-бэкенд)
//! байт-в-байт равен превью-композиту того же момента: и «холодному»
//! (свежий движок — второй GpuFxEngine на том же адаптере), и «прогретому»
//! (тот же движок, ReadbackCache). Плюс: детерминизм экспорта между
//! прогонами на одном устройстве и допуск-сверка GPU-экспорта с CPU.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use kt_engine::{ExportBackends, VideoRenderer, render_raw_video};
use kt_gpu::{FrameTol, assert_frames_close, test_engine};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::Tick;

fn project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 128,
        height: 72,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    for id in [1u64, 2] {
        p.media.push(MediaRef { id: MediaId(id), path: "synth:ramp".into() });
    }
    let sec = 90_000i64;
    let clip = |id: u64, lane: u32, start: i64, dur: i64, media: u64, opacity: f32| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![
            FxInstance::Mirror { horizontal: true, vertical: false },
            FxInstance::glow(1.0),
            FxInstance::vhs(2, 0.4),
        ],
        opacity,
        fx_off: false,
    };
    p.clips = vec![
        clip(1, 0, 0, 2 * sec, 1, 1.0),
        clip(2, 1, sec / 2, sec, 2, 0.75), // верхний слой с уровнем
    ];
    p.validate().unwrap();
    p
}

fn sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    [1u64, 2]
        .iter()
        .map(|&id| {
            (
                MediaId(id),
                Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp))
                    as Box<dyn MediaSource>,
            )
        })
        .collect()
}

fn gpu_export_frames(backends: &ExportBackends, q: Quality, grab: &[usize]) -> Vec<Vec<u8>> {
    let p = project();
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut n = 0usize;
    kt_engine::export::render_raw_video_range_with(
        &p,
        sources(),
        q,
        None,
        backends,
        &Arc::new(AtomicBool::new(false)),
        |_| {},
        |rgba| {
            if grab.contains(&n) {
                out.push(rgba.to_vec());
            }
            n += 1;
            Ok(())
        },
    )
    .unwrap();
    out
}

#[test]
fn t184_gpu_preview_equals_gpu_export_cold_and_warm() {
    let Some(e1) = test_engine() else { return };
    let Some(e2) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let target = 17usize;
    let backends = ExportBackends { video_gpu: Some(e1.clone()) };
    let export_frame = gpu_export_frames(&backends, q, &[target]).remove(0);

    // «превью»: независимый кэш, прямой декод слоёв, тот же план кадра
    let p = project();
    let video = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let t = Tick(target as i64 * 3000);
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources().into_iter().collect();
    for (media, src_pts, q) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    use kt_engine::VideoGpuExec;
    // холодный: СВЕЖИЙ движок на том же адаптере
    let plan = video.frame_plan(&cache, t, q);
    let mut cold = Vec::new();
    let complete = e2.execute_into(&plan, &mut cold).unwrap();
    assert!(complete);
    assert_eq!(cold, export_frame, "холодный GPU-композит == GPU-экспорт");
    // прогретый: движок экспорта, план уже в его ReadbackCache
    let mut warm = Vec::new();
    e1.execute_into(&plan, &mut warm).unwrap();
    assert_eq!(warm, export_frame, "прогретый (ReadbackCache) == GPU-экспорт");
}

#[test]
fn t184b_gpu_export_deterministic_and_close_to_cpu() {
    let Some(e1) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let grab = [0usize, 17, 40];
    let backends = ExportBackends { video_gpu: Some(e1) };
    let a = gpu_export_frames(&backends, q, &grab);
    let b = gpu_export_frames(&backends, q, &grab);
    assert_eq!(a, b, "GPU-экспорт обязан быть детерминирован на устройстве");

    // CPU-эталон тех же кадров — в допуске билинейной арифметики
    let p = project();
    let mut cpu: Vec<Vec<u8>> = Vec::new();
    let mut n = 0usize;
    render_raw_video(&p, sources(), q, |rgba| {
        if grab.contains(&n) {
            cpu.push(rgba.to_vec());
        }
        n += 1;
        Ok(())
    })
    .unwrap();
    for ((g, c), idx) in a.iter().zip(&cpu).zip(grab) {
        assert_frames_close(c, g, q.width, q.height, FrameTol::BILINEAR, &format!("frame {idx}"));
    }
}

/// Тест 301 (Р.44; бывший t187c наоборот): экспорт-рендер БЕЗ GPU больше
/// НЕ отказывает — Pan3d считается на CPU (обратная гомография в слитом
/// гео-проходе), отказ GpuRequired удалён. Кадры реально несут эффект, а
/// при живом адаптере CPU и GPU сходятся в допуске GEOM.
#[test]
fn t301_export_without_gpu_renders_3d() {
    let with_fx = |fx: Vec<FxInstance>| {
        let mut p = project();
        p.clips[0].fx = fx;
        p.clips[1].fx = vec![];
        p.validate().unwrap();
        p
    };
    let pan3d = FxInstance::Pan3d {
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x: 0.0,
        rot_y: 15.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale: 0.8,
        camera: None,
    };
    let q = Quality { width: 128, height: 72 };
    let render = |p: &kt_model::Project, backends: &ExportBackends| -> Vec<u8> {
        let mut got = Vec::new();
        let mut n = 0usize;
        kt_engine::export::render_raw_video_range_with(
            p,
            sources(),
            q,
            None,
            backends,
            &Arc::new(AtomicBool::new(false)),
            |_| {},
            |rgba| {
                if n == 10 {
                    got = rgba.to_vec();
                }
                n += 1;
                Ok(())
            },
        )
        .expect("экспорт-рендер без GPU обязан работать с 3D");
        got
    };
    let cpu_only = ExportBackends::default();
    let cpu3d = render(&with_fx(vec![pan3d.clone()]), &cpu_only);
    let cpu_plain = render(&with_fx(vec![]), &cpu_only);
    assert_ne!(cpu3d, cpu_plain, "Pan3d на CPU обязан менять картинку");
    if let Some(e) = test_engine() {
        let gpu = ExportBackends { video_gpu: Some(e) };
        let gpu3d = render(&with_fx(vec![pan3d]), &gpu);
        assert_frames_close(&cpu3d, &gpu3d, q.width, q.height, FrameTol::GEOM, "t301-cpu-vs-gpu");
    }
}

// ---- 444-446 (Р.50, Б3): поддерево вложения на GPU ---------------------------

/// Проект со вложением: два перекрывающихся события внутри ленты, уровень и
/// цепочка — на ЭКЗЕМПЛЯРЕ. Именно эта конфигурация отличает «границу
/// порядка обработки» от её отсутствия.
fn nested_project(outer_fx: Vec<FxInstance>, outer_opacity: f32) -> Project {
    let sec = 90_000i64;
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 128,
        height: 72,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    for id in [1u64, 2] {
        p.media.push(MediaRef { id: MediaId(id), path: "synth:ramp".into() });
    }
    let ev = |id: u64, lane: u32, start: i64, dur: i64, media: u64, opacity: f32| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity,
        fx_off: false,
    };
    let mut s = Sequence::new(MediaId(100), "Кусок");
    s.clips = vec![ev(2, 1, 0, 2 * sec, 1, 1.0), ev(3, 0, sec / 2, sec, 2, 0.5)];
    s.video_lanes = 2;
    p.sequences.push(s);
    let mut inst = ev(10, 0, 0, 2 * sec, 100, outer_opacity);
    inst.fx = outer_fx;
    p.clips = vec![inst];
    p.validate().unwrap();
    p
}

fn warm_cache(p: &Project, q: Quality, t: Tick) -> (VideoRenderer, kt_cache::BurstCache) {
    let video = VideoRenderer::new(p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources().into_iter().collect();
    for (media, src_pts, dq) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    (video, cache)
}

#[test]
fn t444_nested_reel_conforms_gpu_to_cpu() {
    use kt_engine::VideoGpuExec;
    let Some(e) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let t = Tick(20 * 3000);
    // и с цепочкой на вложении, и без: цепочка меняет путь (границу
    // проходит суб-композит, а не кадр), и оба обязаны сходиться
    for fx in [vec![], vec![FxInstance::Mirror { horizontal: true, vertical: false }]] {
        let p = nested_project(fx.clone(), 0.5);
        let (mut video, cache) = warm_cache(&p, q, t);
        let plan = video.frame_plan(&cache, t, q);
        let mut gpu = Vec::new();
        let complete = e.execute_into(&plan, &mut gpu).expect("GPU обязан УМЕТЬ поддерево");
        assert!(complete, "прогретый композит с вложением обязан быть полным");
        drop(plan);
        let (cpu, cpu_complete) = video.compose_from_cache(&cache, t, q);
        assert!(cpu_complete);
        assert_frames_close(
            &cpu,
            &gpu,
            q.width,
            q.height,
            FrameTol::BILINEAR,
            &format!("вложение, цепочка {}", fx.len()),
        );
        // ...и это не «оба чёрные»
        assert!(gpu.iter().any(|&x| x != 0), "GPU-композит вложения чёрный");
    }
}

#[test]
fn t445_nest_boundary_is_not_twice_as_dark_on_gpu() {
    use kt_engine::VideoGpuExec;
    let Some(e) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let t = Tick(20 * 3000);
    // Полупрозрачное содержимое: ровно там, где премультипликация на
    // границе даёт «вдвое темнее». WGSL-зеркало `unpremultiply` обязано
    // совпадать с CPU до разряда — все текстуры движка Rgba8Uint.
    let p = nested_project(vec![], 1.0);
    let (mut video, cache) = warm_cache(&p, q, t);
    let plan = video.frame_plan(&cache, t, q);
    let mut gpu = Vec::new();
    e.execute_into(&plan, &mut gpu).unwrap();
    drop(plan);
    let (cpu, _) = video.compose_from_cache(&cache, t, q);
    assert_frames_close(&cpu, &gpu, q.width, q.height, FrameTol::EXACT, "граница вложения");
}

#[test]
fn t446_nested_gpu_is_deterministic_on_device() {
    use kt_engine::VideoGpuExec;
    let Some(e1) = test_engine() else { return };
    let Some(e2) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let t = Tick(20 * 3000);
    let p = nested_project(vec![FxInstance::glow(1.0)], 0.75);
    let (video, cache) = warm_cache(&p, q, t);
    let plan = video.frame_plan(&cache, t, q);
    let (mut a, mut b, mut c) = (Vec::new(), Vec::new(), Vec::new());
    e1.execute_into(&plan, &mut a).unwrap();
    // второй прогон ТОГО ЖЕ движка — через ReadbackCache: ключ обязан
    // различать поддеревья, иначе вернётся чужой суб-композит
    e1.execute_into(&plan, &mut b).unwrap();
    // и СВЕЖИЙ движок на том же адаптере — пул отдаёт грязные текстуры,
    // и незачищенный аккумулятор поддерева виден именно здесь
    e2.execute_into(&plan, &mut c).unwrap();
    assert_eq!(a, b, "повторный прогон дал другой кадр (ReadbackCache)");
    assert_eq!(a, c, "свежий движок дал другой кадр (грязный пул поддерева)");

    // ключ плана обязан РАЗЛИЧАТЬ редакции ленты: правим содержимое и
    // требуем другой кадр от ТОГО ЖЕ движка
    let mut p2 = nested_project(vec![FxInstance::glow(1.0)], 0.75);
    p2.sequences[0].clips[1].opacity = 0.9;
    p2.validate().unwrap();
    let (video2, cache2) = warm_cache(&p2, q, t);
    let plan2 = video2.frame_plan(&cache2, t, q);
    let mut d = Vec::new();
    e1.execute_into(&plan2, &mut d).unwrap();
    assert_ne!(a, d, "правка ВНУТРИ ленты не изменила ключ плана — ReadbackCache отдал стейл");

    // ГОЛОД ВНУТРИ ленты меняет картинку, не меняя НИ ОДНОГО поля
    // экземпляра: без плана поддерева в ключе движок вернул бы прогретый
    // кадр на пустом кэше — и никогда не перезапросил бы содержимое.
    let cold = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let plan_cold = video.frame_plan(&cold, t, q);
    let mut e = Vec::new();
    let complete = e1.execute_into(&plan_cold, &mut e).unwrap();
    assert!(!complete, "голод внутри ленты не дошёл до внешнего композита");
    assert_ne!(a, e, "на ПУСТОМ кэше вернулся прогретый кадр — план поддерева не в ключе");
}

/// Тест 496 (Р.53, п.409): кадр опознаётся по СОДЕРЖИМОМУ, а не по
/// адресу. Правка параметров генератора при НЕПОДВИЖНОМ курсоре даёт тот
/// же `(source, cache_idx, размер)` — и до этого пункта GPU отдавал
/// СТАРУЮ картинку: `UploadCache` переиспользовал текстуру, а
/// `ReadbackCache` — готовый композит. Тот же дефект у возврата
/// пропавшего файла вместо заглушки (t211).
///
/// В харнессе это невидимо (он весь CPU), поэтому тест живёт здесь.
#[test]
fn t496_gpu_frame_identity_follows_content() {
    use kt_engine::VideoGpuExec;
    let Some(e) = test_engine() else { return };
    let q = Quality { width: 64, height: 36 };
    let t = Tick(0);

    // проект из одного клипа-генератора; цвет меняем «под курсором»
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    let solid = |r: u8| GeneratorKind::Solid { color_r: r, color_g: 0, color_b: 0 };
    p.media.push(MediaRef { id: MediaId(1), path: kt_model::gen_kind_path(&solid(32)) });
    p.clips.push(ClipEvent {
        // Р.59, стадия 2: значения — на СОБЫТИИ
        r#gen: Some(solid(32)),
        gen_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();

    let shot = |red: u8, cache: &kt_cache::BurstCache| -> Vec<u8> {
        let mut src = kt_media::generator::GeneratorSource::new(solid(red));
        let frame = src.read_video_frame(Tick(0), Some(q)).unwrap();
        // ТОТ ЖЕ адрес: источник, индекс и размер не меняются
        cache.insert_video(
            kt_engine::source_id(kt_engine::gen_source_addr(ClipId(1))),
            Some(q),
            kt_engine::video::video_cache_idx(Tick(0)),
            frame.into(),
        );
        let video = VideoRenderer::new(&p);
        let plan = video.frame_plan(cache, t, q);
        let mut out = Vec::new();
        e.execute_into(&plan, &mut out).unwrap();
        out
    };

    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let dark = shot(32, &cache);
    let bright = shot(220, &cache);
    assert_eq!(dark[0], 32, "первый кадр — заданный цвет");
    assert_ne!(
        dark, bright,
        "правка параметров при неподвижном курсоре обязана менять картинку на GPU"
    );
    assert_eq!(bright[0], 220, "второй кадр — НОВЫЙ цвет, а не старая текстура");
}

// ---- 586-587 (Р.56, п.448): непрерывное время ленты и смешивание на GPU ---

/// Варп-эффект на ВИДЕО-экземпляре: значимы только скорость и режим кадров.
fn warp_fx(rate: f64, frames: FrameMode) -> FxInstance {
    FxInstance::SlowedReverb {
        rate,
        mix: 0.0,
        reverb_delay_ms: 85.0,
        damp: 0.32,
        decay: 0.58,
        wet_gain: 0.95,
        predelay_ms: 0.0,
        frames,
        width: 1.0,
        automation: vec![],
    }
}

/// Та же сцена, что `nested_project`, но у нижнего ребёнка ЖИВАЯ кривая
/// яркости: без чего-нибудь процедурного внутри полукадр неотличим от
/// кадра — футаж-то прибит к кадрам файла (п.448).
fn animated_nested_project(outer_fx: Vec<FxInstance>) -> Project {
    let mut p = nested_project(outer_fx, 1.0);
    let child = &mut p.sequences[0].clips[0];
    child.fx =
        vec![FxInstance::ColorGrade { brightness: 0.0, contrast: 0.0, saturation: 1.0, hue: 0.0 }];
    child.fx_auto = vec![vec![ParamCurve {
        param: "brightness".into(),
        ease: false,
        keys: vec![AutoKey { t: Tick(0), v: 0.0 }, AutoKey { t: Tick(60_000), v: 0.9 }],
        source: Default::default(),
    }]];
    p.validate().unwrap();
    p
}

/// Прогреть ОДИН кэш под момент `t` (ключи — ровно те, что заказал план).
///
/// Кэш обязан быть ОБЩИМ на все моменты одного движка: `UploadCache`
/// ключуется СЕРИЙНЫМ НОМЕРОМ содержимого (п.409), а свежий `BurstCache`
/// начинает нумерацию заново — два кэша на один движок дают ему одинаковые
/// ключи у РАЗНЫХ кадров, и текстура возвращается чужая. В программе кэш
/// один на сессию; тест обязан повторять обвязку движка.
fn warm_into(video: &VideoRenderer, cache: &kt_cache::BurstCache, q: Quality, t: Tick) {
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources().into_iter().collect();
    for (media, src_pts, dq) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
}

/// CPU- и GPU-исполнение ОДНОГО плана момента `t` на общем кэше.
fn cpu_and_gpu(
    e: &kt_gpu::GpuFxEngine,
    video: &mut VideoRenderer,
    cache: &kt_cache::BurstCache,
    q: Quality,
    t: Tick,
) -> (Vec<u8>, Vec<u8>) {
    use kt_engine::VideoGpuExec;
    warm_into(video, cache, q, t);
    let mut gpu = Vec::new();
    {
        let plan = video.frame_plan(cache, t, q);
        let complete = e.execute_into(&plan, &mut gpu).expect("GPU обязан уметь поддерево");
        assert!(complete, "прогретый композит с вложением обязан быть полным");
    }
    let (cpu, complete) = video.compose_from_cache(cache, t, q);
    assert!(complete, "CPU-композит с вложением обязан быть полным");
    (cpu, gpu)
}

/// Тест 586 (Р.56, п.448): ЗАМЕДЛЕННАЯ лента на GPU.
///
/// Красный на старом коде: внутреннее время ленты флорилось к сетке, и
/// соседние выходные кадры выходили одинаковыми на ОБОИХ бэкендах — вместе
/// с анимацией внутри композиции.
#[test]
fn t586_slowed_nest_conforms_and_is_deterministic_on_gpu() {
    let Some(e) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let p = animated_nested_project(vec![warp_fx(0.5, FrameMode::Nearest)]);
    let mut video = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());

    // ЧЁТНЫЙ кадр стоит на целом шаге материала, НЕЧЁТНЫЙ — на полушаге
    let (even, odd) = (Tick(20 * 3000), Tick(21 * 3000));
    let (cpu_e, gpu_e) = cpu_and_gpu(&e, &mut video, &cache, q, even);
    let (cpu_o, gpu_o) = cpu_and_gpu(&e, &mut video, &cache, q, odd);
    assert_frames_close(&cpu_e, &gpu_e, q.width, q.height, FrameTol::EXACT, "t586-even");
    assert_frames_close(&cpu_o, &gpu_o, q.width, q.height, FrameTol::EXACT, "t586-odd");
    assert_ne!(
        gpu_e, gpu_o,
        "полукадр замедленной ленты совпал с кадром: внутреннее время округляется \
         к сетке, и анимация внутри композиции идёт ступеньками"
    );
    assert!(gpu_o.iter().any(|&x| x != 0), "кадр чёрный — сверять нечего");

    // детерминизм НА ОДНОМ движке: второй прогон идёт через ReadbackCache,
    // и ключ плана обязан различать полукадры
    let (_, again) = cpu_and_gpu(&e, &mut video, &cache, q, odd);
    assert_eq!(gpu_o, again, "повторный прогон того же плана дал другой кадр");
}

/// Тест 587 (Р.56, п.448): СМЕШИВАНИЕ на вложении — на GPU.
///
/// Смешивают ЛИСТЬЯ, а не два суб-композита: GPU получает уже смешанный
/// пре-FX кадр обычным `LayerInput::Frame`, и второго dissolve-пасса не
/// появляется. План при этом обязан отличаться от «ближайшего» — иначе
/// ReadbackCache отдаст чужую картинку.
#[test]
fn t587_blend_on_a_nest_differs_from_nearest_on_gpu() {
    let Some(e) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let t = Tick(21 * 3000); // полукадр: у листьев есть пара
    let blend = nested_project(vec![warp_fx(0.5, FrameMode::Blend)], 1.0);
    let near = nested_project(vec![warp_fx(0.5, FrameMode::Nearest)], 1.0);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut vb = VideoRenderer::new(&blend);
    let mut vn = VideoRenderer::new(&near);

    let (cpu_b, gpu_b) = cpu_and_gpu(&e, &mut vb, &cache, q, t);
    let (cpu_n, gpu_n) = cpu_and_gpu(&e, &mut vn, &cache, q, t);
    assert_frames_close(&cpu_b, &gpu_b, q.width, q.height, FrameTol::EXACT, "t587-blend");
    assert_frames_close(&cpu_n, &gpu_n, q.width, q.height, FrameTol::EXACT, "t587-nearest");
    assert_ne!(
        gpu_b, gpu_n,
        "«смешивание» на вложении не отличается от «ближайшего»: режим не \
         доехал до листьев"
    );
    // и это не «одно из них чёрное»
    assert!(gpu_b.iter().any(|&x| x != 0) && gpu_n.iter().any(|&x| x != 0));

    // детерминизм на одном движке (ReadbackCache)
    let (_, again) = cpu_and_gpu(&e, &mut vb, &cache, q, t);
    assert_eq!(gpu_b, again, "повторный прогон смешанного плана дал другой кадр");
}

// ---- 632 (Р.58, п.459): ступень ЛЕНТЫ ≠ вход цепочки экземпляра --------------

/// Тот же проект, но у ленты ДВА экземпляра: зумящий (он и поднимает
/// ступень ленты) и близнец со своей цепочкой. Ступень одна на ленту, а
/// потребить её умеет не всякая цепочка — второму экземпляру суб-композит
/// ВПИСЫВАЕТСЯ (`NestSrc::fit_to`, на GPU это `Resample` во весь кадр).
fn twin_nest_project(twin_fx: Vec<FxInstance>) -> Project {
    let sec = 90_000i64;
    let mut p = nested_project(
        vec![FxInstance::Pan { mode: Default::default(), pan_x: 0.0, pan_y: 0.0, zoom: 3.0 }],
        1.0,
    );
    let mut twin = p.clips[0].clone();
    twin.id = ClipId(11);
    twin.start = Tick(10 * sec);
    twin.fx = twin_fx;
    p.clips.push(twin);
    p.validate().unwrap();
    p
}

#[test]
fn t632_reel_stage_and_chain_input_conform_gpu_to_cpu() {
    use kt_engine::VideoGpuExec;
    let Some(e) = test_engine() else { return };
    let q = Quality { width: 128, height: 72 };
    let sec = 90_000i64;
    let t_twin = Tick(10 * sec + 20 * 3000);
    let cases: Vec<(&str, Vec<FxInstance>)> = vec![
        // вписывание: цепочка ступени не потребляет
        ("empty", vec![]),
        ("glow", vec![FxInstance::glow(1.0)]),
        (
            "leading mix",
            vec![
                FxInstance::Pan { mode: Default::default(), pan_x: 0.0, pan_y: 0.0, zoom: 2.0 },
                FxInstance::LayerMix { amount: 0.5, from: 0 },
            ],
        ),
        // без вписывания: экземпляр сам просит ступень ленты
        (
            "same zoom",
            vec![FxInstance::Pan { mode: Default::default(), pan_x: 0.0, pan_y: 0.0, zoom: 3.0 }],
        ),
    ];
    for (name, fx) in cases {
        let p = twin_nest_project(fx);
        let (mut video, cache) = warm_cache(&p, q, t_twin);
        let plan = video.frame_plan(&cache, t_twin, q);
        let mut gpu = Vec::new();
        let complete = e.execute_into(&plan, &mut gpu).expect("GPU обязан УМЕТЬ поддерево");
        assert!(complete, "{name}: прогретый композит объявлен неполным");
        drop(plan);
        let (cpu, cpu_complete) = video.compose_from_cache(&cache, t_twin, q);
        assert!(cpu_complete, "{name}: CPU-композит объявлен неполным");
        // допуск BILINEAR: вписывание — ресемпл, а `resample.wgsl` считает
        // в f32 и округляет к чётному, `fit_into` — в f64 и от нуля
        assert_frames_close(&cpu, &gpu, q.width, q.height, FrameTol::BILINEAR, name);
        assert!(gpu.iter().any(|&x| x != 0), "{name}: GPU-композит чёрный — вложение выпало");
        assert!(cpu.iter().any(|&x| x != 0), "{name}: CPU-композит чёрный — вложение выпало");
    }
}
