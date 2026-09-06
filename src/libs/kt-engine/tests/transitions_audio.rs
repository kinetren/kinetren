//! Тест 136 (п.136 porting-notes, Р.24): аудио-кроссфейд по ПЕРЕХОДАМ.
//!
//! Веса equal-power приходят из TransitionEvent (fade-окна), «ручки» читают
//! источник за краями использованного куска. Краеугольный ассерт: конверсия
//! старого перекрытия (трим до C + переход тем же окном) даёт БАЙТ-в-байт
//! тот же рендер — на этом держится неизменность GOLDEN_AUDIO на этапе 6.

use kt_engine::AudioRenderer;
use kt_engine::DirectFetcher;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;

fn project(clips: Vec<ClipEvent>) -> Project {
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
    let mut ids: Vec<MediaId> = clips.iter().map(|c| c.media).collect();
    ids.sort();
    ids.dedup();
    for id in ids {
        p.media.push(MediaRef { id, path: format!("synth:{}", id.0) });
    }
    p.clips = clips;
    p.validate().unwrap();
    p
}

fn clip(id: u64, start_f: i64, dur_f: i64, src_in_f: i64, media: u64) -> ClipEvent {
    ClipEvent {
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick::from_audio_frames(start_f),
        duration: Tick::from_audio_frames(dur_f),
        source_in: Tick::from_audio_frames(src_in_f),
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

fn direct(medias: &[u64]) -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    for &id in medias {
        f.add_source(MediaId(id), Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp)));
    }
    f
}

fn render(p: &Project, medias: &[u64], total: usize) -> (Vec<f32>, u64) {
    let r = AudioRenderer::new(p);
    let mut f = direct(medias);
    let mut out = vec![0f32; total * CH];
    let stats = r.render_frames(&mut f, 0, &mut out);
    (out, stats.underruns)
}

/// Конверсия перекрытия по формуле upgrade_legacy: перекрытие [b0, a1) →
/// стык C = середина, тримы, переход с тем же окном. Эталон — аналитическая
/// формула старого weight_at (сам старый рендер-путь снесён, реш. 8 Р.24;
/// рендер-против-рендера проверяет неизменность GOLDEN_AUDIO в t11).
#[test]
fn t136_converted_overlap_matches_old_crossfade_math() {
    // старый мир: A[0,12288) src 0, B[8192,20480) src 0 — перекрытие [8192,12288);
    // новый мир: C = 10240, A тримится до C, B: source_in += C − b0
    let mut p = project(vec![clip(1, 0, 10_240, 0, 1), clip(2, 10_240, 10_240, 2_048, 2)]);
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick::from_audio_frames(2_048),
        out_offset: Tick::from_audio_frames(2_048),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
    });
    p.validate().unwrap();

    let total = 20_480usize;
    let (got, underruns) = render(&p, &[1, 2], total);
    assert_eq!(underruns, 0, "handles must not underrun");
    let (w0, w1) = (8_192i64, 12_288i64);
    for f in 0..total as i64 {
        // старая семантика: A читает src=f, B читает src=f−8192; веса
        // equal-power по прогрессу перекрытия — дословно формулы weight_at
        let a_val = SyntheticSource::sample_at(AudioPattern::Ramp, f, 0);
        let b_val = SyntheticSource::sample_at(AudioPattern::Ramp, f - w0, 0);
        let expect = if f < w0 {
            a_val
        } else if f < w1 {
            let t = (f - w0) as f64 / (w1 - w0) as f64;
            a_val * (((1.0 - t).max(0.0)).sqrt() as f32) + b_val * ((t.max(0.0)).sqrt() as f32)
        } else {
            b_val
        };
        assert_eq!(got[(f as usize) * CH], expect, "frame {f}: conversion must be exact");
    }
}

/// Веса и материал ручек — точная математика по кадрам.
#[test]
fn t136_fade_math_and_handle_material() {
    // стык на 10240, окно [8192, 12288); источники различимы source_in'ами
    let mut p = project(vec![clip(1, 0, 10_240, 0, 1), clip(2, 10_240, 10_240, 50_000, 2)]);
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick::from_audio_frames(2_048),
        out_offset: Tick::from_audio_frames(2_048),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
    });
    p.validate().unwrap();
    let (out, underruns) = render(&p, &[1, 2], 14_336);
    assert_eq!(underruns, 0);
    let (w0, w1) = (8_192i64, 12_288i64);
    for f in [0i64, 4_000, 8_191, 8_192, 9_000, 10_239, 10_240, 11_000, 12_287, 12_288, 14_000] {
        let a_src = f; // clip 1: source_in 0, тело [0,10240), ручка [10240,12288)
        let b_src = 50_000 + (f - 10_240); // clip 2: ручка слева от source_in
        let a_val = SyntheticSource::sample_at(AudioPattern::Ramp, a_src, 0);
        let b_val = SyntheticSource::sample_at(AudioPattern::Ramp, b_src, 0);
        let t = ((f - w0) as f64 / (w1 - w0) as f64).clamp(0.0, 1.0);
        let (wa, wb) = if f < w0 {
            (1.0f32, 0.0f32)
        } else if f < w1 {
            (((1.0 - t).max(0.0)).sqrt() as f32, (t.max(0.0)).sqrt() as f32)
        } else {
            (0.0, 1.0)
        };
        // порядок суммирования рендера: клип 1 (ранний) потом клип 2
        let expect = if f < w0 {
            a_val * wa
        } else if f < w1 {
            a_val * wa + b_val * wb
        } else {
            b_val * wb
        };
        assert_eq!(out[(f as usize) * CH], expect, "frame {f}");
    }
}

/// Ручка длиннее материала: source_in = 0 у incoming — тишина без underrun.
#[test]
fn t136_missing_left_handle_is_silent_without_underruns() {
    let mut p = project(vec![clip(1, 0, 10_240, 0, 1), clip(2, 10_240, 10_240, 0, 2)]);
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick::from_audio_frames(2_048),
        out_offset: Tick::from_audio_frames(2_048),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
    });
    p.validate().unwrap();
    let (out, underruns) = render(&p, &[1, 2], 12_288);
    assert_eq!(underruns, 0, "missing handle material is silence, not underrun");
    // в [w0, стык): звучит только затухающий A (у B материала нет)
    let (w0, w1) = (8_192i64, 12_288i64);
    for f in [8_192i64, 9_000, 10_000] {
        let t = (f - w0) as f64 / (w1 - w0) as f64;
        let expect =
            SyntheticSource::sample_at(AudioPattern::Ramp, f, 0) * (((1.0 - t).max(0.0)).sqrt() as f32);
        assert_eq!(out[(f as usize) * CH], expect, "frame {f}: A only");
    }
}

/// FX-путь (цепочка без варпа) обязан читать ручки так же, как plain-путь.
#[test]
fn t136_fx_path_handles_match_plain() {
    let mk = |fx: Vec<FxInstance>| {
        let mut c1 = clip(1, 0, 10_240, 0, 1);
        c1.fx = fx;
        let mut p = project(vec![c1, clip(2, 10_240, 10_240, 2_048, 2)]);
        p.transitions.push(TransitionEvent {
            id: TransitionId(1),
            a: Some(ClipId(1)),
            b: Some(ClipId(2)),
            in_offset: Tick::from_audio_frames(2_048),
            out_offset: Tick::from_audio_frames(2_048),
            kind: TransitionKind::Crossfade,
            audio: Default::default(),
        });
        p.validate().unwrap();
        p
    };
    let (plain, _) = render(&mk(vec![]), &[1, 2], 14_336);
    let (fxed, u) = render(&mk(vec![FxInstance::Gain { db: 0.0 }]), &[1, 2], 14_336);
    assert_eq!(u, 0);
    if let Some(i) = (0..plain.len()).find(|&i| plain[i] != fxed[i]) {
        panic!("fx-path handle differs from plain at sample {i}");
    }
}

/// Реверс-сосед (plain): ручки продолжают зеркальную формулу источника.
#[test]
fn t136_reversed_incoming_handle_mirrors_source() {
    let mut c2 = clip(2, 10_240, 10_240, 4_096, 2);
    c2.reversed = true;
    let mut p = project(vec![clip(1, 0, 10_240, 0, 1), c2]);
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick::from_audio_frames(2_048),
        out_offset: Tick::from_audio_frames(2_048),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
    });
    p.validate().unwrap();
    let (out, underruns) = render(&p, &[1, 2], 14_336);
    assert_eq!(underruns, 0);
    let (w0, w1) = (8_192i64, 12_288i64);
    // reversed: src = source_in + (len − 1 − local); в левой ручке local < 0
    // → материал ПРАВЕЕ использованного куска
    for f in [8_500i64, 10_239, 10_240, 11_000] {
        let local = f - 10_240;
        let b_src = 4_096 + (10_240 - 1 - local);
        let b_val = SyntheticSource::sample_at(AudioPattern::Ramp, b_src, 0);
        let a_val = SyntheticSource::sample_at(AudioPattern::Ramp, f, 0);
        let t = (f - w0) as f64 / (w1 - w0) as f64;
        let (wa, wb) = (((1.0 - t).max(0.0)).sqrt() as f32, (t.max(0.0)).sqrt() as f32);
        let expect = a_val * wa + b_val * wb;
        assert_eq!(out[(f as usize) * CH], expect, "frame {f}");
    }
}

/// Префетч греет ручки: wanted покрывает материал за краями тел.
#[test]
fn t136_wanted_covers_handles() {
    let mut p = project(vec![clip(1, 0, 10_240, 0, 1), clip(2, 10_240, 10_240, 2_048, 2)]);
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick::from_audio_frames(2_048),
        out_offset: Tick::from_audio_frames(2_048),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
    });
    p.validate().unwrap();
    let r = AudioRenderer::new(&p);
    let wanted = r.wanted_audio_bursts(8_192..12_288);
    // A: тело [8192,10240) + правая ручка [10240,12288) → src до 12288
    let a = wanted.iter().find(|(m, _)| *m == MediaId(1)).unwrap();
    assert!(a.1.end * 1024 >= 12_288, "outgoing handle must be prefetched: {:?}", a.1);
    // B: левая ручка src от 2048−2048 = 0
    let b = wanted.iter().find(|(m, _)| *m == MediaId(2)).unwrap();
    assert_eq!(b.1.start, 0, "incoming handle must be prefetched: {:?}", b.1);
}

// ---- тест 156: аудио-фейд у края (Р.25) --------------------------------------

/// Фейд-ин звука: тот же equal-power вес, что у кроссфейда, только напротив
/// окна пустота — звук поднимается из тишины (и симметрично гаснет в неё).
#[test]
fn t156_audio_fade_ramps_from_silence() {
    let mut p = project(vec![clip(1, 0, 4800, 0, 1)]);
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: None,
        b: Some(ClipId(1)),
        in_offset: Tick::ZERO,
        out_offset: Tick::from_audio_frames(2400), // фейд-ин на 2400 кадров
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
    });
    p.validate().unwrap();
    let (out, underruns) = render(&p, &[1], 4800);
    assert_eq!(underruns, 0);

    // dry-эталон того же клипа без перехода
    let dry = render(&project(vec![clip(1, 0, 4800, 0, 1)]), &[1], 4800).0;
    assert_eq!(out[0], 0.0, "в начале окна — тишина");
    for f in [600usize, 1200, 2399] {
        let w = (f as f64 / 2400.0).sqrt() as f32; // equal-power, как у кроссфейда
        let expect = dry[f * CH] * w;
        assert!(
            (out[f * CH] - expect).abs() < 1e-6,
            "кадр {f}: ожидался вес {w}, вышло {} против {expect}",
            out[f * CH]
        );
    }
    for f in [2400usize, 4000] {
        assert_eq!(out[f * CH], dry[f * CH], "за окном фейда клип звучит как есть");
    }
}

/// Фейд-аут: к концу клипа звук уходит в тишину.
#[test]
fn t156_audio_fade_out_ends_in_silence() {
    let mut p = project(vec![clip(1, 0, 4800, 0, 1)]);
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: None,
        in_offset: Tick::from_audio_frames(2400),
        out_offset: Tick::ZERO,
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
    });
    p.validate().unwrap();
    let (out, _) = render(&p, &[1], 4800);
    let dry = render(&project(vec![clip(1, 0, 4800, 0, 1)]), &[1], 4800).0;
    assert_eq!(out[100 * CH], dry[100 * CH], "до окна — без изменений");
    let f = 3600usize; // середина окна [2400, 4800)
    let w = (1.0 - (f - 2400) as f64 / 2400.0).sqrt() as f32;
    assert!((out[f * CH] - dry[f * CH] * w).abs() < 1e-6);
    assert!(out[4799 * CH].abs() < 1e-3, "к концу окна почти тишина");
}
