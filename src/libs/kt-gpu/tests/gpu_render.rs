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
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 128, height: 72, sample_rate: 48000 });
    for id in [1u64, 2] {
        p.media.push(MediaRef { id: MediaId(id), path: "synth:ramp".into() });
    }
    let sec = 90_000i64;
    let clip = |id: u64, lane: u32, start: i64, dur: i64, media: u64, opacity: f32| ClipEvent {
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

fn gpu_export_frames(
    backends: &ExportBackends,
    q: Quality,
    grab: &[usize],
) -> Vec<Vec<u8>> {
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
    for (media, src_pts) in video.layers_at(t) {
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

/// Р.29: экспорт проекта с 3D-эффектами БЕЗ GPU обязан отказать ДО
/// создания файла (тихо испорченный файл — клипы без 3D — хуже ошибки).
#[test]
fn t187c_export_without_gpu_refuses_3d() {
    let mut p = project();
    p.clips[0].fx = vec![FxInstance::Pan3d {
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x: 0.0,
        rot_y: 15.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale: 1.0,
        camera: None,
    }];
    p.validate().unwrap();
    assert_eq!(kt_engine::gpu_required_clips(&p).len(), 1);
    // CPU-обёртка (без бэкендов) — preflight срабатывает до муксера
    let err = kt_engine::export_mp4_with(
        &p,
        vec![],
        vec![],
        "/nonexistent/dir/out.mp4",
        None,
        &kt_engine::ExportOptions::default(),
        &Arc::new(AtomicBool::new(false)),
        &Arc::new(AtomicBool::new(false)),
        |_| {},
    )
    .unwrap_err();
    assert!(
        matches!(err, kt_engine::export::ExportError::GpuRequired(1)),
        "ожидался GpuRequired: {err}"
    );
    assert!(
        !std::path::Path::new("/nonexistent/dir/out.mp4").exists(),
        "файл не должен был появиться"
    );
}
