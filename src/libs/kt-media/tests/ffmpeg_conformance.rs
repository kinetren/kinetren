//! Тест 10 (docs/porting-notes.md): conformance FfmpegSource на реальных
//! фикстурах (src/fixtures/gen.sh, генерируются при первом прогоне).
#![cfg(feature = "ffmpeg")]

use kt_media::ffmpeg::FfmpegSource;
use kt_media::{MediaSource, Quality};
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, AUDIO_SAMPLE_RATE, Tick};

const CH: usize = AUDIO_CHANNELS;

/// gen.sh запускается один раз на процесс (тесты параллельны).
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

// ---- Аудио ------------------------------------------------------------------

#[test]
fn t10_flac_sequential_matches_seeked_reads_exactly() {
    // Lossless 48k: burst n после seek == burst n при декоде с нуля, exact.
    let path = fixture("sine48k_stereo.flac");
    let mut seq = FfmpegSource::open(&path).unwrap();
    let mut reference = Vec::new();
    for idx in 0..40 {
        reference.push(seq.read_audio_burst(idx).unwrap());
    }
    // свежий источник, чтение вразброс (каждое — через seek-путь)
    let mut rnd = FfmpegSource::open(&path).unwrap();
    for &idx in &[25i64, 3, 39, 0, 17, 8, 30, 1] {
        let b = rnd.read_audio_burst(idx).unwrap();
        assert_eq!(
            b.data[..],
            reference[idx as usize].data[..],
            "burst {idx}: seek-путь обязан быть sample-accurate (lossless)"
        );
    }
}

#[test]
fn t10_aac_441_resampled_alignment_and_length() {
    let path = fixture("sine44k_mono.m4a");
    let mut src = FfmpegSource::open(&path).unwrap();
    assert!(src.info().has_audio);

    // длина: 2 c @ 44.1k → ~96000 кадров на 48k (энкодерный priming/padding
    // допускаем в пределах пары burst'ов)
    let mut total = 0i64;
    let mut idx = 0i64;
    while let Ok(b) = src.read_audio_burst(idx) {
        // конец: полностью нулевой burst после сигнала
        let silent = b.data.iter().all(|&s| s == 0.0);
        idx += 1;
        if silent && total > 0 {
            break;
        }
        total = idx * AUDIO_BURST_FRAMES as i64;
        if idx > 120 {
            break;
        }
    }
    assert!(
        (total - 2 * AUDIO_SAMPLE_RATE as i64).abs() <= 3 * AUDIO_BURST_FRAMES as i64,
        "2 c 44.1k → ~96000 кадров 48k, получили {total}"
    );

    // sample-accurate выравнивание: seek-чтение против последовательного,
    // допускаем лосси-разницу по значению, но НЕ по фазе: ищем сдвиг
    // максимальной корреляции — он обязан быть 0.
    let mut seq = FfmpegSource::open(&path).unwrap();
    let mut reference = Vec::new();
    for i in 0..30 {
        reference.push(seq.read_audio_burst(i).unwrap());
    }
    let mut rnd = FfmpegSource::open(&path).unwrap();
    let probe = rnd.read_audio_burst(20).unwrap();
    let r = &reference[20].data;
    let best_shift = (-8i64..=8)
        .max_by_key(|&shift| {
            let mut acc = 0f64;
            for f in 8..(AUDIO_BURST_FRAMES as i64 - 8) {
                let a = probe.data[(f * CH as i64) as usize] as f64;
                let b = r[((f + shift) * CH as i64) as usize] as f64;
                acc += a * b;
            }
            (acc * 1e6) as i64
        })
        .unwrap();
    assert_eq!(best_shift, 0, "seek-путь AAC сдвинут на {best_shift} сэмплов");
}

// ---- Видео ------------------------------------------------------------------

#[test]
fn t10_h264_backward_seek_is_deterministic() {
    let path = fixture("testsrc_h264.mp4");
    let mut a = FfmpegSource::open(&path).unwrap();
    assert!(a.info().has_video);
    assert_eq!(a.info().fps, Some((30, 1)));

    let frame_tick = |n: i64| Tick(n * 3000); // 30 fps → 3000 тиков/кадр
    let q = Some(Quality { width: 320, height: 180 });

    // прямой доступ к кадру 5 со свежего источника
    let direct = a.read_video_frame(frame_tick(5), q).unwrap();
    // тот же источник: уйти вперёд на кадр 40, вернуться на 5 (backward seek
    // + пересоздание декодера — MT H.264 путь)
    let _far = a.read_video_frame(frame_tick(40), q).unwrap();
    let back = a.read_video_frame(frame_tick(5), q).unwrap();
    assert_eq!(direct.rgba, back.rgba, "backward seek H.264 обязан быть детерминирован");
    assert_eq!(direct.width, 320);
    assert_eq!(direct.height, 180);
}

#[test]
fn t10_vp9_decodes_and_seeks() {
    let path = fixture("testsrc_vp9.webm");
    let mut src = FfmpegSource::open(&path).unwrap();
    let q = Some(Quality { width: 160, height: 90 });
    let f10 = src.read_video_frame(Tick(10 * 3000), q).unwrap();
    let f2 = src.read_video_frame(Tick(2 * 3000), q).unwrap();
    let f10b = src.read_video_frame(Tick(10 * 3000), q).unwrap();
    assert_eq!(f10.rgba, f10b.rgba, "VP9 (threads=1) seek туда-обратно детерминирован");
    assert_ne!(f2.rgba, f10.rgba, "testsrc кадры 2 и 10 различаются");
}

#[test]
fn t10_keyframe_index_and_quality_ladder() {
    let path = fixture("muxed_av.mp4");
    let mut src = FfmpegSource::open(&path).unwrap();
    assert!(src.info().has_video && src.info().has_audio);

    // keyframe_before из пакетного индекса (g=15 → кейфрейм каждые 0.5 c)
    let kf = src.keyframe_before(Tick(90_000)).expect("keyframe перед 1.0 c");
    assert!(kf.0 <= 90_000 && kf.0 >= 90_000 - 16 * 3000, "kf={kf:?}");

    // лестница качеств: один pts, разные размеры
    let big =
        src.read_video_frame(Tick(30 * 3000), Some(Quality { width: 320, height: 180 })).unwrap();
    let small =
        src.read_video_frame(Tick(30 * 3000), Some(Quality { width: 160, height: 90 })).unwrap();
    assert_eq!((big.width, big.height), (320, 180));
    assert_eq!((small.width, small.height), (160, 90));

    // и аудио из муксованного файла читается
    let b = src.read_audio_burst(10).unwrap();
    assert!(b.data.iter().any(|&s| s.abs() > 0.05), "аудио муксованного файла слышно");
}

// ---- ogg с обложкой (регрессия п.26) ----------------------------------------

#[test]
fn t10_ogg_cover_art_no_abort_and_decodes() {
    // Раньше: mjpeg-обложка считалась видео-потоком, а её stream_index из
    // скан-контекста уходил в av_seek_frame нового ogg-контекста →
    // av_assert0(stream_index < ogg->nstreams) → abort ВСЕГО процесса.
    let mut src = FfmpegSource::open(&fixture("cover_art.ogg")).expect("открытие ogg");
    let info = src.info().clone();
    assert!(info.has_audio, "vorbis-поток должен быть виден");
    assert!(!info.has_video, "обложка (attached_pic) — не видео");

    // Аудио декодируется и не тишина.
    let b = src.read_audio_burst(0).expect("burst 0");
    let energy: f32 = b.data.iter().map(|s| s * s).sum();
    assert!(energy > 1e-3, "vorbis burst 0 не должен быть тишиной, energy={energy}");

    // Seek вперёд-назад не падает (тот самый путь с av_seek_frame).
    let far = (AUDIO_SAMPLE_RATE as i64) / (AUDIO_BURST_FRAMES as i64); // ~1 c
    let b1 = src.read_audio_burst(far).expect("burst после seek вперёд");
    let b0 = src.read_audio_burst(0).expect("burst после seek назад");
    let e1: f32 = b1.data.iter().map(|s| s * s).sum();
    let e0: f32 = b0.data.iter().map(|s| s * s).sum();
    assert!(e1 > 1e-3 && e0 > 1e-3);

    // Запрос видеокадра у аудио-файла — ошибка, не падение.
    assert!(
        src.read_video_frame(Tick(0), Some(Quality { width: 64, height: 64 })).is_err(),
        "у файла без видео read_video_frame должен вернуть ошибку"
    );
    let _ = CH;
}
