//! Тест 113 (п.14 р.14): кадры в ОФЛАЙН-рендере (тот же путь, что экспорт)
//! идут строго вперёд, равномерно и без чёрных дыр — включая случай, когда
//! fps проекта не совпадает с fps источника.
//!
//! Пользователь: «подлагивание есть и в отрендеренных видео всегда». Одна
//! причина была в декодере (тест 112 на реальном файле), вторая проверяется
//! здесь: маппинг «тик проекта → кадр источника» и покадровая выдача
//! экспорта. Синтетика включена в режим собственной сетки кадров — цвет
//! кадра кодирует НОМЕР кадра источника, поэтому из готового RGBA видно,
//! какой именно кадр попал в вывод.

use kt_engine::export::render_raw_video;
use kt_media::MediaSource;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::Tick;

const DUR: Tick = Tick(90_000 * 3); // 3 с

fn project(fps: u32, reversed: bool) -> Project {
    let mut p = Project::new(ProjectSettings { fps, width: 64, height: 36, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: DUR,
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed,
        fx_bypass: vec![],
        fx: vec![],
    });
    p.validate().unwrap();
    p
}

fn sources(src_fps: u32) -> Vec<(MediaId, Box<dyn MediaSource>)> {
    vec![(
        MediaId(1),
        Box::new(SyntheticSource::new(DUR, AudioPattern::Silence).with_video_fps(src_fps))
            as Box<dyn MediaSource>,
    )]
}

/// Номера кадров источника, попавшие в вывод (цвет кодирует номер).
fn rendered_indices(project_fps: u32, src_fps: u32, reversed: bool) -> Vec<i64> {
    let p = project(project_fps, reversed);
    let q = kt_media::Quality { width: 64, height: 36 };
    let mut out = Vec::new();
    render_raw_video(&p, sources(src_fps), q, |rgba| {
        let idx = rgba[0] as i64 | (rgba[1] as i64) << 8 | (rgba[2] as i64) << 16;
        out.push(idx);
        Ok(())
    })
    .expect("офлайн-рендер");
    out
}

#[test]
fn t113_export_frames_are_monotonic_and_uniform() {
    // --- сетки НЕ совпадают: проект 60, источник 25
    let idx = rendered_indices(60, 25, false);
    assert_eq!(idx.len(), 180, "3 с при 60 fps = 180 кадров, получили {}", idx.len());
    assert!(idx.iter().all(|&i| i >= 0), "чёрный кадр (индекс не прочитан)");
    // 1) НИКОГДА не назад
    for w in idx.windows(2) {
        assert!(w[1] >= w[0], "кадр откатился назад: {} → {}", w[0], w[1]);
    }
    // 2) без пропусков: соседние кадры либо тот же, либо следующий
    for w in idx.windows(2) {
        assert!(w[1] - w[0] <= 1, "пропущен кадр источника: {} → {}", w[0], w[1]);
    }
    // 3) равномерность: длины серий одинаковых кадров отличаются ≤ 1
    let mut runs: Vec<usize> = Vec::new();
    let mut cur = idx[0];
    let mut len = 0usize;
    for &i in &idx {
        if i == cur {
            len += 1;
        } else {
            runs.push(len);
            cur = i;
            len = 1;
        }
    }
    runs.push(len);
    // крайние серии могут быть обрезаны границами диапазона — их не судим
    let mid = &runs[1..runs.len() - 1];
    let lo = mid.iter().min().copied().unwrap();
    let hi = mid.iter().max().copied().unwrap();
    assert!(hi - lo <= 1, "неравномерные дубли: серии от {lo} до {hi} ({runs:?})");

    // --- сетки совпадают: строго 1:1
    let same = rendered_indices(30, 30, false);
    assert_eq!(same.len(), 90);
    for (n, &i) in same.iter().enumerate() {
        assert_eq!(i, n as i64, "при равных сетках кадр {n} обязан быть 1:1, получили {i}");
    }

    // --- реверс: строго назад, тоже без дыр
    let rev = rendered_indices(30, 30, true);
    assert_eq!(rev.len(), 90);
    for w in rev.windows(2) {
        assert!(w[1] <= w[0], "реверс: кадр ушёл вперёд {} → {}", w[0], w[1]);
        assert!(w[0] - w[1] <= 1, "реверс: пропущен кадр {} → {}", w[0], w[1]);
    }
}
