//! Тест 485 (Р.52 С2): ГОЛДЕН ТИТРА — сцена «титр kt-text поверх
//! solid-генератора». Отдельный эталон `GOLDEN_TITLE_VIDEO`:
//! GOLDEN_VIDEO основного golden-теста не двигается (его проект без
//! генераторов), а титровый эталон — сторож дисциплины детерминизма
//! kt-text (вендоренный шрифт, пины harfrust/ab_glyph): их смена
//! краснеет ЗДЕСЬ, и перегенерация обязана быть осознанной
//! (kt-text/CLAUDE.md, FONT-MANIFEST.txt).

use kt_engine::{VideoRenderer, render_raw_video};
use kt_media::generator::GeneratorSource;
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::Tick;

fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

type Sources = Vec<(MediaId, Box<dyn MediaSource>)>;

fn title_project() -> (Project, Sources) {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    let mut title = GeneratorKind::default_text();
    if let GeneratorKind::Text { text, bold, size, fill_r, fill_g, fill_b, .. } = &mut title {
        *text = "Kinotrain\nР.52 титр".into();
        *bold = true;
        *size = 0.16;
        (*fill_r, *fill_g, *fill_b) = (255, 136, 0);
    }
    let solid = GeneratorKind::Solid { color_r: 16, color_g: 32, color_b: 64 };
    p.media.push(MediaRef { id: MediaId(1), path: format_gen_path(&solid) });
    p.media.push(MediaRef { id: MediaId(2), path: format_gen_path(&title) });
    let clip = |id: u64, lane: u32, media: u64| ClipEvent {
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane,
        start: Tick(0),
        duration: Tick(60_000),
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
    p.clips = vec![clip(1, 1, 1), clip(2, 0, 2)];
    p.validate().unwrap();
    let sources: Sources = vec![
        (MediaId(1), Box::new(GeneratorSource::new(solid))),
        (MediaId(2), Box::new(GeneratorSource::new(title))),
    ];
    (p, sources)
}

fn run_hash() -> (u64, Vec<u8>) {
    let (p, sources) = title_project();
    let q = Quality { width: 320, height: 180 };
    let mut all = Vec::new();
    let mut frame7 = Vec::new();
    let mut n = 0usize;
    render_raw_video(&p, sources, q, |rgba| {
        if n == 7 {
            frame7 = rgba.to_vec();
        }
        all.extend_from_slice(rgba);
        n += 1;
        Ok(())
    })
    .unwrap();
    (fnv(&all), frame7)
}

#[test]
fn t485_title_export_is_deterministic() {
    let (a, frame) = run_hash();
    let (b, _) = run_hash();
    assert_eq!(a, b, "экспорт титра обязан быть байт-в-байт между прогонами");
    // титр реально в кадре: есть пиксели цвета заливки поверх фона
    let orange = frame
        .chunks_exact(4)
        .filter(|px| px[0] > 200 && px[1] > 90 && px[1] < 180 && px[2] < 60)
        .count();
    assert!(orange > 100, "титра в кадре не видно: {orange} пикселей заливки");
    let bg = frame.chunks_exact(4).filter(|px| px == &[16, 32, 64, 255]).count();
    assert!(bg > 1000, "фон-солид под титром пропал: {bg}");
}

/// Эталон зафиксирован на linux x86_64 (как GOLDEN_VIDEO). Осознанная
/// смена шрифта/пинов = перегенерация с записью в FONT-MANIFEST.txt.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn t485_title_matches_committed_golden() {
    let (v, _) = run_hash();
    assert_eq!(v, GOLDEN_TITLE_VIDEO, "титровый поток разошёлся с эталоном: 0x{v:016x}");
}

// ПЕРЕГЕНЕРИРОВАН в С3 того же раунда: заливка пошла через единый
// premult-композит слоёв оформления — у краевых (антиалиасных) пикселей
// цветной заливки сдвинулись младшие разряды. Прежний (С2, fill-only
// путь): 0x4cea52ff97ff15a5.
const GOLDEN_TITLE_VIDEO: u64 = 0x95267f57405a8c95;

#[test]
#[ignore = "генератор эталона: cargo test -p kt-engine --test title_golden -- --ignored print"]
fn t485_print_golden() {
    let (v, _) = run_hash();
    println!("GOLDEN_TITLE_VIDEO = 0x{v:016x}");
}

/// Превью == экспорт на титровой сцене (инвариант 7): кадр №7 экспорта
/// байт-в-байт равен превью-композиту того же момента.
#[test]
fn t485_title_preview_equals_export() {
    let (_, export_frame) = run_hash();
    let (p, sources) = title_project();
    let q = Quality { width: 320, height: 180 };
    let mut video = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let t = Tick(7 * 3000);
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources.into_iter().collect();
    for (media, src_pts, q) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    let (preview, complete) = video.compose_from_cache(&cache, t, q);
    assert!(complete, "композит титра обязан быть полон");
    assert_eq!(preview, export_frame, "превью и экспорт обязаны совпадать байт-в-байт");
}
