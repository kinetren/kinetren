//! Тест 796b (Р.67): ОДИНОЧНАЯ КАРТИНКА в плане, композите и экспорте.
//!
//! Владелец: «статичные картинки не отображаюся, вообще.. там вечный
//! Loading... мало того что не работает - так ты еще об этом узнаешь
//! только в момент просмотра». Фикстур со стиллами не было ни одной, и
//! путь «картинка → план → кадр → файл» никогда не был под тестом.
//!
//! Требует системный FFmpeg (как conformance-тест 10 и export_options).

use kt_media::{MediaSource, Quality};

/// Короткий отпечаток кадра: сравнивать массивы по 230 КБ в `assert_eq!`
/// значит завалить вывод теста мегабайтом чисел.
fn fp(bytes: &[u8]) -> u64 {
    let mut h = 1469598103934665603u64;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}
use kt_model::*;
use kt_time::Tick;

const Q: Quality = Quality { width: 320, height: 180 };

static GEN: std::sync::OnceLock<()> = std::sync::OnceLock::new();

fn fixture(name: &str) -> String {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    GEN.get_or_init(|| {
        let st = std::process::Command::new("bash")
            .arg(dir.join("gen.sh"))
            .status()
            .expect("run fixtures/gen.sh (нужен ffmpeg в PATH)");
        assert!(st.success(), "gen.sh failed");
    });
    dir.join("out").join(name).to_string_lossy().to_string()
}

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: Q.width,
        height: Q.height,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    }
}

fn clip(id: u64, media: u64, lane: u32, dur_s: f64) -> ClipEvent {
    ClipEvent {
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
        start: Tick::ZERO,
        duration: Tick((dur_s * 90_000.0) as i64),
        source_in: Tick::ZERO,
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

/// Композит момента `t`, прогретый ровно тем, что заказал план.
fn compose(p: &Project, t: Tick) -> (Vec<u8>, bool) {
    let mut video = kt_engine::VideoRenderer::new(p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        std::collections::HashMap::new();
    for m in &p.media {
        let s = kt_media::ffmpeg::FfmpegSource::open(&m.path)
            .unwrap_or_else(|e| panic!("{}: {e}", m.path));
        srcs.insert(m.id, Box::new(s));
    }
    for (addr, pts, dq) in video.layers_at(t, Q) {
        let f = srcs
            .get_mut(&addr)
            .unwrap_or_else(|| panic!("заказан адрес {addr:?}, а источника по нему нет"))
            .read_video_frame(pts, Some(dq))
            .unwrap_or_else(|e| panic!("кадр картинки на {pts:?}: {e}"));
        cache.insert_video(
            kt_engine::source_id(addr),
            Some(dq),
            kt_engine::video::video_cache_idx(pts),
            f.into(),
        );
    }
    video.compose_from_cache(&cache, t, Q)
}

#[test]
fn t796b_a_still_is_planned_composed_and_exported() {
    // --- событие 10 с на КАРТИНКЕ: кадр есть на любом тике -------------
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), fixture("still.jpg")));
    p.clips.push(clip(1, 1, 0, 10.0));
    p.ensure_tracks(LaneKind::Video, 1);
    p.validate().expect("событие на картинке законно");

    let mut shots = Vec::new();
    for t in [Tick(0), Tick((3.7 * 90_000.0) as i64), Tick((9.96 * 90_000.0) as i64)] {
        let (bytes, complete) = compose(&p, t);
        assert!(complete, "композит на {t:?} не укомплектован — это и есть вечное «Loading…»");
        assert!(bytes.iter().any(|&b| b != 0), "кадр картинки на {t:?} пуст");
        shots.push(bytes);
    }
    assert_eq!(fp(&shots[0]), fp(&shots[1]), "на 3.7 с пришёл другой кадр: у картинки он один");
    assert_eq!(fp(&shots[0]), fp(&shots[2]), "на 9.96 с (почти конец события) кадра не стало");

    // --- детерминизм: второй прогон байт-в-байт ------------------------
    let (again, _) = compose(&p, Tick(0));
    assert_eq!(fp(&again), fp(&shots[0]), "композит картинки недетерминирован");

    // --- PNG С АЛЬФОЙ поверх цветного фона: фон виден сквозь ----------
    let mut over = Project::new(settings());
    over.media.push(MediaRef::new(MediaId(1), fixture("still.jpg")));
    over.media.push(MediaRef::new(MediaId(2), fixture("still_alpha.png")));
    // дорожка 0 — ВЕРХНЯЯ: полупрозрачный PNG кладётся на неё, jpeg — фоном
    over.clips.push(clip(2, 2, 0, 4.0));
    over.clips.push(clip(1, 1, 1, 4.0));
    over.ensure_tracks(LaneKind::Video, 2);
    over.validate().expect("картинка поверх картинки законна");
    let (blended, complete) = compose(&over, Tick(0));
    assert!(complete, "композит двух картинок не укомплектован");
    // фон (нижний слой) — тот же jpeg; наложение полупрозрачного PNG
    // ОБЯЗАНО дать что-то среднее, а не чистый верхний слой
    let (top, _) = compose(
        &{
            let mut only = Project::new(settings());
            only.media.push(MediaRef::new(MediaId(2), fixture("still_alpha.png")));
            only.clips.push(clip(2, 2, 0, 4.0));
            only.ensure_tracks(LaneKind::Video, 1);
            only.validate().unwrap();
            only
        },
        Tick(0),
    );
    assert_ne!(fp(&blended), fp(&top), "альфа PNG не сработала: верхний слой лёг непрозрачным");
    assert_ne!(fp(&blended), fp(&shots[0]), "верхнего слоя на композите не видно вовсе");

    // --- ЭКСПОРТ 1 с: файл есть и в нём кадры картинки ----------------
    let dir = std::env::temp_dir().join(format!("kt-t796b-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let out = dir.join("still.mp4");
    let mut short = p.clone();
    short.clips[0].duration = Tick(90_000);
    short.validate().unwrap();
    let sources = || -> Vec<(MediaId, Box<dyn MediaSource>)> {
        short
            .media
            .iter()
            .map(|m| {
                let s = kt_media::ffmpeg::FfmpegSource::open(&m.path).expect("источник");
                (m.id, Box::new(s) as Box<dyn MediaSource>)
            })
            .collect()
    };
    // аудио и видео — СВОИ источники: делить один между проходами значит
    // делить и позицию чтения (инвариант 5)
    kt_engine::export::export_mp4(&short, sources(), sources(), out.to_str().unwrap())
        .expect("экспорт события на картинке");
    let size = std::fs::metadata(&out).expect("файла экспорта нет").len();
    assert!(size > 1000, "файл экспорта подозрительно мал: {size} байт");
    let _ = std::fs::remove_dir_all(&dir);
}
