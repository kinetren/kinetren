//! Тест 11 (docs/porting-notes.md): golden-экспорт.
//! - сырые потоки (RGBA + f32) детерминированы от прогона к прогону;
//! - хэши совпадают с закоммиченными эталонами (платформенная привязка:
//!   linux x86_64 — на другой платформе libm/sinc-коэффициенты могут дать
//!   другие биты, тогда эталон пересматривается осознанно);
//! - «превью == экспорт»: кадр из экспортного потока байт-в-байт равен
//!   кадру превью-композита при том же Quality (единый путь — инвариант №7).

use kt_engine::{VideoRenderer, render_raw_audio, render_raw_video};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::Tick;

/// FNV-1a: без зависимостей, стабильный.
fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn f32s_as_bytes(s: &[f32]) -> Vec<u8> {
    s.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Проект теста 11: конкатенация + кроссфейд + slowed с автоматизацией.
fn golden_project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 128,
        height: 72,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    for (id, path) in
        [(1, "synth:ramp"), (2, "synth:dc:0.5"), (3, "synth:impulse"), (4, "synth:ramp")]
    {
        p.media.push(MediaRef { id: MediaId(id), path: path.into() });
    }
    let sec = 90_000i64;
    p.clips = vec![
        // конкатенация встык (ramp продолжает источник)
        ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick(0),
            duration: Tick(sec),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        },
        // Р.24: бывший нахлёст [1.5c, 2c) конвертирован в стык C = 1.75c +
        // переход (окно то же) — байты рендера обязаны совпасть со старым
        // GOLDEN (краеугольная проверка конверсии, т.136)
        ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(2),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick(sec),
            duration: Tick(sec * 3 / 4),
            source_in: Tick(sec),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        },
        // кроссфейд: DC через переход со вторым ramp-клипом
        ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(3),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick(sec * 7 / 4),
            duration: Tick(sec * 3 / 4),
            source_in: Tick(sec / 4),
            media: MediaId(2),
            gain_db: -3.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        },
        // slowed с автоматизацией + реверб на отдельной дорожке
        ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(4),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 1,
            start: Tick(0),
            duration: Tick(2 * sec),
            source_in: Tick(0),
            media: MediaId(3),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            // автоматизация slowed — общими кривыми (Р.33): ключи в тиках
            // МАТЕРИАЛА. Доли 0.3/0.7 прежнего домена при source_in = 0 и
            // длине 2 с дают ровно эти тики, поэтому звук байт-в-байт
            // прежний и GOLDEN_AUDIO не перегенерировался
            fx_auto: vec![vec![
                ParamCurve {
                    param: "rate".into(),

                    keys: AutoKey::linear(vec![
                        AutoKey::at(Tick(2 * sec * 3 / 10), 1.0),
                        AutoKey::at(Tick(2 * sec * 7 / 10), 0.5),
                    ]),
                    source: Default::default(),
                },
                ParamCurve {
                    param: "mix".into(),

                    keys: AutoKey::linear(vec![
                        AutoKey::at(Tick(2 * sec * 3 / 10), 0.4),
                        AutoKey::at(Tick(2 * sec * 7 / 10), 0.1),
                    ]),
                    source: Default::default(),
                },
            ]],
            fx: vec![FxInstance::SlowedReverb {
                rate: 0.7,
                mix: 0.4,
                reverb_delay_ms: 85.0,
                damp: 0.32,
                decay: 0.58,
                wet_gain: 0.95,
                predelay_ms: 0.0,
                width: 1.0,
                frames: Default::default(),
                automation: vec![],
            }],
            opacity: 1.0,
            fx_off: false,
        },
        // видео: два слоя с перекрытием (z-композит)
        ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(5),
            link_group: None,
            kind: ClipKind::Video,
            lane: 0,
            start: Tick(0),
            duration: Tick(2 * sec),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        },
        ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(6),
            link_group: None,
            kind: ClipKind::Video,
            lane: 1,
            start: Tick(sec / 2),
            duration: Tick(sec),
            source_in: Tick(0),
            media: MediaId(4),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        },
    ];
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(2)),
        b: Some(ClipId(3)),
        in_offset: Tick(sec / 4),
        out_offset: Tick(sec / 4),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    p
}

fn sources(ids: &[(u64, AudioPattern)]) -> Vec<(MediaId, Box<dyn MediaSource>)> {
    ids.iter()
        .map(|&(id, pat)| {
            (
                MediaId(id),
                Box::new(SyntheticSource::new(Tick(90_000 * 60), pat)) as Box<dyn MediaSource>,
            )
        })
        .collect()
}

fn all_sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    sources(&[
        (1, AudioPattern::Ramp),
        (2, AudioPattern::Dc { value: 0.5 }),
        (3, AudioPattern::Impulse),
        (4, AudioPattern::Ramp),
    ])
}

fn run_export_hashes() -> (u64, u64, u64, u64) {
    let p = golden_project();
    let mut audio_hash = 0xcbf29ce484222325u64;
    let mut audio_frames = 0u64;
    let astats = render_raw_audio(&p, all_sources(), |chunk| {
        let b = f32s_as_bytes(chunk);
        audio_hash ^= fnv(&b);
        audio_hash = audio_hash.wrapping_mul(0x100000001b3);
        audio_frames += (chunk.len() / 2) as u64;
        Ok(())
    })
    .unwrap();
    let q = Quality { width: 128, height: 72 };
    let mut video_hash = 0xcbf29ce484222325u64;
    let vstats = render_raw_video(&p, all_sources(), q, |rgba| {
        video_hash ^= fnv(rgba);
        video_hash = video_hash.wrapping_mul(0x100000001b3);
        Ok(())
    })
    .unwrap();
    assert_eq!(astats.underruns, 0);
    assert_eq!(vstats.missing_video_layers, 0);
    (audio_hash, video_hash, astats.audio_frames, vstats.video_frames)
}

#[test]
fn t11_streams_are_deterministic_across_runs() {
    let a = run_export_hashes();
    let b = run_export_hashes();
    assert_eq!(a, b, "экспорт обязан быть байт-в-байт воспроизводим");
    // 2.5 секунды таймлайна (конец кроссфейд-клипа): 120000 аудио-кадров,
    // 75 видео-кадров
    assert_eq!(a.2, 120_000);
    assert_eq!(a.3, 75);
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn t11_streams_match_committed_golden() {
    let (audio, video, _, _) = run_export_hashes();
    // Эталоны зафиксированы на linux x86_64. Осознанная смена алгоритмов
    // рендера = осознанное обновление эталонов (с прослушиванием!).
    assert_eq!(audio, GOLDEN_AUDIO, "аудио-поток разошёлся с эталоном: 0x{audio:016x}");
    assert_eq!(video, GOLDEN_VIDEO, "видео-поток разошёлся с эталоном: 0x{video:016x}");
}

// ПЕРЕГЕНЕРИРОВАН 2026-09-07 (второй и последний раз): ось автоматизации
// переведена из SOURCE-домена в домен СОБЫТИЯ по явному решению
// пользователя (п.1 Р.37, тест 31) — звучание клипа с автоматизацией
// изменилось осознанно. Прежнее значение — 0x469c89aa2adc3b14.
//
// Сдвиг посчитан руками ДО прогона и совпал: у клипа 4 кривая rate
// 1.0 → 0.5 на долях 0.3/0.7, поэтому S(1) ушла с ≈0.7728 (решение ОДУ по
// материалу) на ровно 0.75 (трапеция по времени: 0.3·1 + 0.4·0.75 + 0.3·0.5)
// — около 2200 кадров источника, плюс перетаймилась кривая mix.
//
// Видео-эталон НЕ менялся и не должен: у видео-клипов проекта `fx: vec![]`,
// а варп-ветка включается только при slowed в цепочке.
const GOLDEN_AUDIO: u64 = 0x5a83eef245ab43f0;
const GOLDEN_VIDEO: u64 = 0x95b790352d74f586;

#[test]
#[ignore = "генератор эталонов: cargo test -- --ignored t11_print"]
fn t11_print_golden() {
    let (a, v, af, vf) = run_export_hashes();
    println!("GOLDEN_AUDIO = 0x{a:016x}; GOLDEN_VIDEO = 0x{v:016x}; frames a={af} v={vf}");
}

#[test]
fn t11_preview_equals_export_frame() {
    // Кадр №17 экспортного потока == превью-композит того же момента
    // тем же Quality (единый render-путь).
    let p = golden_project();
    let q = Quality { width: 128, height: 72 };
    let target = 17usize;
    let mut captured: Option<Vec<u8>> = None;
    let mut n = 0usize;
    render_raw_video(&p, all_sources(), q, |rgba| {
        if n == target {
            captured = Some(rgba.to_vec());
        }
        n += 1;
        Ok(())
    })
    .unwrap();
    let export_frame = captured.unwrap();

    // «превью»: независимый кэш, прямой декод слоёв, тот же композит
    let mut video = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let t = Tick(target as i64 * 3000);
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        all_sources().into_iter().collect();
    for (media, src_pts, q) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    let (preview_frame, complete) = video.compose_from_cache(&cache, t, q);
    assert!(complete);
    assert_eq!(preview_frame, export_frame, "превью и экспорт обязаны совпадать байт-в-байт");
}

/// Тест 67 (п.2 р.7): превью == экспорт с ВИДЕО-FX-цепочкой — и холодный
/// compose (цепочка применяется), и прогретый (кадр из пост-FX кэша)
/// байт-в-байт равны кадру экспортного потока. Пост-FX мемоизация не
/// нарушает инвариант №7.
#[test]
fn t67_preview_equals_export_with_fx_cold_and_warm() {
    let mut p = golden_project();
    // цепочка из жалобы пользователя на видео-клип
    for c in p.clips.iter_mut().filter(|c| c.kind == ClipKind::Video) {
        c.fx = vec![
            FxInstance::Mirror { horizontal: true, vertical: false },
            FxInstance::glow(1.0),
            FxInstance::vhs(2, 0.4),
        ];
    }
    p.validate().unwrap();
    let q = Quality { width: 128, height: 72 };
    let target = 17usize;
    let mut captured: Option<Vec<u8>> = None;
    let mut n = 0usize;
    render_raw_video(&p, all_sources(), q, |rgba| {
        if n == target {
            captured = Some(rgba.to_vec());
        }
        n += 1;
        Ok(())
    })
    .unwrap();
    let export_frame = captured.unwrap();

    let mut video = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let t = Tick(target as i64 * 3000);
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        all_sources().into_iter().collect();
    for (media, src_pts, q) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    let (cold, complete) = video.compose_from_cache(&cache, t, q);
    assert!(complete);
    assert_eq!(cold, export_frame, "холодный FX-композит == экспорт");
    let (warm, _) = video.compose_from_cache(&cache, t, q);
    assert_eq!(warm, export_frame, "прогретый (пост-FX кэш) == экспорт");
}

// ---- Р.52: генератор в рендере ------------------------------------------

type Sources = Vec<(MediaId, Box<dyn MediaSource>)>;

/// Проект t479: solid-генератор поверх синтетического фона.
fn generator_project() -> (Project, Sources) {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 128,
        height: 72,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:dc:0.25".into() });
    // Р.59, стадия 2: в пути медиа — ВИД, значения на событии
    p.media.push(MediaRef { id: MediaId(2), path: "gen:solid".into() });
    let clip = |id: u64, lane: u32, start: i64, dur: i64, media: u64| ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
        opacity: 1.0,
        fx_off: false,
    };
    // фон — вся сцена; solid входит на 30_000 (кадр №10) верхним слоем
    let mut g2 = clip(2, 0, 30_000, 60_000, 2);
    g2.r#gen = Some(kt_model::GeneratorKind::Solid { color_r: 255, color_g: 136, color_b: 0 });
    p.clips = vec![clip(1, 1, 0, 90_000, 1), g2];
    p.validate().unwrap();
    // источник генератора открывает САМ экспорт (Р.59, стадия 2): его
    // адрес производен от события, и вызывающему его взять неоткуда
    let sources: Sources =
        vec![(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000), AudioPattern::Silence)))];
    (p, sources)
}

/// Тест 479 (Р.52, родня t11): генератор в рендер-пути. Экспорт
/// детерминирован между прогонами; кадр в зоне генератора — ровно
/// заданный цвет; превью == экспорт байт-в-байт (инвариант 7).
/// GOLDEN_AUDIO/GOLDEN_VIDEO этой сценой НЕ трогаются: у них свой
/// проект без генераторов.
#[test]
fn t479_generator_preview_equals_export() {
    let q = Quality { width: 128, height: 72 };
    let target = 17usize; // кадр внутри зоны solid (t >= 30_000)
    let run = || {
        let (p, sources) = generator_project();
        let mut captured: Option<Vec<u8>> = None;
        let mut all = Vec::new();
        let mut n = 0usize;
        render_raw_video(&p, sources, q, |rgba| {
            if n == target {
                captured = Some(rgba.to_vec());
            }
            all.extend_from_slice(rgba);
            n += 1;
            Ok(())
        })
        .unwrap();
        (captured.unwrap(), fnv(&all))
    };
    let (frame_a, hash_a) = run();
    let (_, hash_b) = run();
    assert_eq!(hash_a, hash_b, "экспорт с генератором обязан быть детерминирован");

    // верхний слой — solid: каждый пиксель кадра ровно заданного цвета
    assert!(
        frame_a.chunks_exact(4).all(|px| px == [255, 136, 0, 255]),
        "кадр генератора обязан быть ровно заданного цвета"
    );

    // превью тем же путём, что t11: независимый кэш + layers_at
    let (p, sources) = generator_project();
    let mut video = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let t = Tick(target as i64 * 3000);
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources.into_iter().collect();
    // превью поднимает источники генераторов тем же списком, что экспорт
    for (addr, state) in kt_engine::gen_sources(&p) {
        srcs.insert(addr, kt_media::factory::open_gen(&state));
    }
    for (media, src_pts, q) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    let (preview_frame, complete) = video.compose_from_cache(&cache, t, q);
    assert!(complete, "композит с генератором обязан быть полон");
    assert_eq!(preview_frame, frame_a, "превью и экспорт обязаны совпадать байт-в-байт");
}

/// Тест 715 (Р.62): golden не двигаются (t11/t67 выше зелёные без
/// перегенерации — на совпадающих пропорциях ведущий узел тождествен), а
/// вертикальный источник в 16:9 даёт кадр с ПОЛЯМИ: замер геометрии
/// заливки по готовому кадру экспорта, как в F-MED-004.
#[test]
fn t715_vertical_source_gets_fields_in_export() {
    let mut p = golden_project();
    // один видео-клип (id 5) на медиа 1; аудио и второй слой не нужны
    p.clips.retain(|c| c.id == ClipId(5));
    p.transitions.clear();
    p.morphers.clear();
    let q = Quality { width: 128, height: 72 };
    let src: Vec<(MediaId, Box<dyn MediaSource>)> = vec![(
        MediaId(1),
        Box::new(
            SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence).with_video_size(36, 64),
        ),
    )];
    let mut captured: Option<Vec<u8>> = None;
    let mut n = 0usize;
    render_raw_video(&p, src, q, |rgba| {
        if n == 3 {
            captured = Some(rgba.to_vec());
        }
        n += 1;
        Ok(())
    })
    .unwrap();
    let f = captured.expect("frame 3");
    let px =
        |x: u32, y: u32| &f[((y * q.width + x) * 4) as usize..((y * q.width + x) * 4 + 3) as usize];
    let dark = |x: u32, y: u32| px(x, y).iter().all(|&v| v < 8);
    // поля слева и справа — фон, середина — заливка источника
    assert!(dark(0, 36) && dark(127, 36), "fields are background");
    assert!(!dark(64, 36), "centre is the source");
    let lit = (0..q.width).filter(|&x| !dark(x, 36)).count();
    // рамка «вписать» вертикального 36×64 в 128×72: 72·36/64 = 40.5 px
    assert!((39..=42).contains(&lit), "fitted width ≈ 40.5 px, got {lit}");
}

/// Тест 728b (Р.62): экспорт в разрешении ≠ проекта приводит ТЕМИ ЖЕ
/// долями. Вертикальный источник в проекте 1920×1080: кадр экспорта
/// 1280×720 подобен кадру 1920×1080 — доля ширины заливки и её центр
/// совпадают в допуске ресемплинга; правило и узел не пересчитываются.
#[test]
fn t728b_export_in_another_resolution_is_similar_to_preview() {
    let mut p = golden_project();
    p.clips.retain(|c| c.id == ClipId(5));
    p.clips[0].duration = Tick(3000 * 6);
    p.transitions.clear();
    p.morphers.clear();
    p.settings.width = 1920;
    p.settings.height = 1080;
    let src = || -> Vec<(MediaId, Box<dyn MediaSource>)> {
        vec![(
            MediaId(1),
            Box::new(
                SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence)
                    .with_video_size(36, 64),
            ),
        )]
    };
    let profile = |q: Quality| -> (f64, f64) {
        let mut captured: Option<Vec<u8>> = None;
        let mut n = 0usize;
        render_raw_video(&p, src(), q, |rgba| {
            if n == 3 {
                captured = Some(rgba.to_vec());
            }
            n += 1;
            Ok(())
        })
        .unwrap();
        let f = captured.expect("frame 3");
        let y = q.height / 2;
        let lit: Vec<u32> = (0..q.width)
            .filter(|&x| {
                let i = ((y * q.width + x) * 4) as usize;
                f[i..i + 3].iter().any(|&v| v >= 8)
            })
            .collect();
        assert!(!lit.is_empty(), "{}×{}: пустой кадр", q.width, q.height);
        let w = q.width as f64;
        (lit.len() as f64 / w, (lit[0] + lit[lit.len() - 1]) as f64 / 2.0 / w)
    };
    let (frac_a, mid_a) = profile(Quality { width: 1920, height: 1080 });
    let (frac_b, mid_b) = profile(Quality { width: 1280, height: 720 });
    // «вписать» 36×64 в 16:9: ширина заливки = 36/64 · 9/16 = 0.3164 холста
    let want = 36.0 / 64.0 * 9.0 / 16.0;
    assert!((frac_a - want).abs() < 2.0 / 1080.0, "1080p: доля {frac_a} ≠ {want}");
    assert!((frac_b - want).abs() < 2.0 / 720.0, "720p: доля {frac_b} ≠ {want}");
    assert!((frac_a - frac_b).abs() < 2.0 / 720.0, "доли разошлись: {frac_a} / {frac_b}");
    assert!((mid_a - 0.5).abs() < 1.0 / 720.0 && (mid_b - 0.5).abs() < 1.0 / 720.0);
}
