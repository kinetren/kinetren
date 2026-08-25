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
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 128, height: 72, sample_rate: 48000 });
    for (id, path) in
        [(1, "synth:ramp"), (2, "synth:dc:0.5"), (3, "synth:impulse"), (4, "synth:ramp")]
    {
        p.media.push(MediaRef { id: MediaId(id), path: path.into() });
    }
    let sec = 90_000i64;
    p.clips = vec![
        // конкатенация встык (ramp продолжает источник)
        ClipEvent {
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
            fx: vec![],
        },
        ClipEvent {
            id: ClipId(2),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick(sec),
            duration: Tick(sec),
            source_in: Tick(sec),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx: vec![],
        },
        // кроссфейд: DC внахлёст на второй ramp-клип
        ClipEvent {
            id: ClipId(3),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick(sec + sec / 2),
            duration: Tick(sec),
            source_in: Tick(0),
            media: MediaId(2),
            gain_db: -3.0,
            reversed: false,
            fx_bypass: vec![],
            fx: vec![],
        },
        // slowed с автоматизацией + реверб на отдельной дорожке
        ClipEvent {
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
            fx: vec![FxInstance::SlowedReverb {
                rate: 0.7,
                mix: 0.4,
                reverb_delay_ms: 85.0,
                damp: 0.32,
                decay: 0.58,
                wet_gain: 0.95,
                automation: vec![
                    SlowedKeyframe { t: 0.3, rate: 1.0, mix: 0.4 },
                    SlowedKeyframe { t: 0.7, rate: 0.5, mix: 0.1 },
                ],
            }],
        },
        // видео: два слоя с перекрытием (z-композит)
        ClipEvent {
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
            fx: vec![],
        },
        ClipEvent {
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
            fx: vec![],
        },
    ];
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

// ПЕРЕГЕНЕРИРОВАН 2026-08-23: автоматизация slowed переведена в
// SOURCE-домен по явному решению пользователя (п.1 v1.3, тест 31) —
// звучание клипа с автоматизацией изменилось осознанно. Видео-эталон
// НЕ менялся (правка чисто аудио).
const GOLDEN_AUDIO: u64 = 0x469c89aa2adc3b14;
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
    for (media, src_pts) in video.layers_at(t) {
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
            FxInstance::Glow { strength: 1.0 },
            FxInstance::Vhs { rgb_shift: 2, bleed: 0.4 },
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
    for (media, src_pts) in video.layers_at(t) {
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
