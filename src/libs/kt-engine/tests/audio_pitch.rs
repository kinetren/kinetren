//! Тесты 337-340 (Р.48, пп.339-340): смена высоты без смены длительности.
//!
//! Что доказывается: нейтральный питч не трогает байты вовсе; результат
//! не зависит от нарезки буферов (гриды зёрен и хопов абсолютны);
//! высота действительно едет на заданное число полутонов ОБОИМИ
//! движками; движок — настройка проекта, он меняет звук, переживает
//! сохранение и откатывается undo.

use kt_engine::{AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const DUR: i64 = 48_000 * 2;
const TONE: f64 = 440.0;

fn project(fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>, engine: PitchEngine) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: engine,
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:tone".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick::from_audio_frames(DUR),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx,
        fx_bypass: vec![],
        fx_auto: auto,
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn source() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: TONE })),
    );
    f
}

fn render(fx: Vec<FxInstance>, engine: PitchEngine) -> Vec<f32> {
    render_auto(fx, vec![], engine)
}

fn render_auto(
    fx: Vec<FxInstance>,
    auto: Vec<Vec<ParamCurve>>,
    engine: PitchEngine,
) -> Vec<f32> {
    let p = project(fx, auto, engine);
    let r = AudioRenderer::new(&p);
    let mut f = source();
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; DUR as usize * CH];
    let stats = r.render_frames_with(&mut f, &mut store, 0, &mut out);
    assert_eq!(stats.underruns, 0, "источник доступен целиком");
    out
}

/// Тот же рендер несколькими кусками подряд (нарезка не должна влиять).
fn render_chunked(fx: Vec<FxInstance>, engine: PitchEngine, chunks: &[i64]) -> Vec<f32> {
    let p = project(fx, vec![], engine);
    let r = AudioRenderer::new(&p);
    let mut f = source();
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = Vec::new();
    let mut pos = 0i64;
    for &n in chunks {
        let mut buf = vec![0f32; n as usize * CH];
        r.render_frames_with(&mut f, &mut store, pos, &mut buf);
        out.extend_from_slice(&buf);
        pos += n;
    }
    out
}

/// Доминирующая частота левого канала в окне [from, from+N) — пик
/// спектра по Ханну.
fn dominant_hz(buf: &[f32], from: usize) -> f64 {
    use realfft::RealFftPlanner;
    const N: usize = 16384;
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(N);
    let mut time: Vec<f32> = (0..N)
        .map(|i| {
            let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / N as f32).cos();
            buf[(from + i) * CH] * w
        })
        .collect();
    let mut spec = fft.make_output_vec();
    fft.process(&mut time, &mut spec).unwrap();
    let (mut best, mut best_mag) = (0usize, 0f32);
    for (k, c) in spec.iter().enumerate().skip(1) {
        let m = c.re * c.re + c.im * c.im;
        if m > best_mag {
            best_mag = m;
            best = k;
        }
    }
    best as f64 * 48_000.0 / N as f64
}

/// Энергия полосы ±20 Гц вокруг частоты (для сравнения доминирования).
fn band_energy(buf: &[f32], from: usize, hz: f64) -> f64 {
    use realfft::RealFftPlanner;
    const N: usize = 16384;
    let mut planner = RealFftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(N);
    let mut time: Vec<f32> = (0..N)
        .map(|i| {
            let w = 0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / N as f32).cos();
            buf[(from + i) * CH] * w
        })
        .collect();
    let mut spec = fft.make_output_vec();
    fft.process(&mut time, &mut spec).unwrap();
    let bin = |f: f64| (f * N as f64 / 48_000.0).round() as usize;
    let (a, b) = (bin(hz - 20.0).max(1), bin(hz + 20.0).min(spec.len() - 1));
    spec[a..=b].iter().map(|c| (c.re * c.re + c.im * c.im) as f64).sum()
}

/// Тест 337: НЕЙТРАЛЬ — БАЙТ-В-БАЙТ.
///
/// Ноль полутонов и центов без кривых: узел не трогает буфер вовсе, и
/// упреждения у цепочки нет. Эффект, добавленный «на попробовать», не
/// смеет менять экспорт молча. (Red-proof: убрать ранний выход по
/// `la == 0` в `PitchNode::process`.)
#[test]
fn t337_neutral_pitch_is_byte_passthrough() {
    let dry = render(vec![], PitchEngine::Wsola);
    for engine in [PitchEngine::Wsola, PitchEngine::Vocoder] {
        let neutral = render(vec![FxInstance::pitch_shift()], engine);
        assert_eq!(dry, neutral, "нейтральный питч обязан быть тождеством ({engine:?})");
    }
    // и упреждения нейтраль не требует
    let auto = kt_fx::AutoProfiles::from_curves_rev(&[vec![]], 0, DUR, false);
    assert_eq!(kt_fx::chain_lookahead(&[FxInstance::pitch_shift()], &auto), 0);
    // а живая кривая нейтралью НЕ является, даже при нулевой базе
    let curve = vec![vec![ParamCurve {
        param: "semitones".into(),
        ease: false,
        keys: vec![
            AutoKey { t: Tick(0), v: 0.0 },
            AutoKey { t: Tick::from_audio_frames(DUR), v: 7.0 },
        ],
    }]];
    let auto_c = kt_fx::AutoProfiles::from_curves_rev(&curve, 0, DUR, false);
    assert!(
        kt_fx::chain_lookahead(&[FxInstance::pitch_shift()], &auto_c) > 0,
        "кривая поверх нулевой базы — не нейтраль"
    );
}

/// Тест 338: НАРЕЗКА НЕ ВЛИЯЕТ.
///
/// Гриды зёрен (WSOLA) и хопов (вокодер) абсолютны по позиции клипа,
/// поэтому рендер одним куском обязан совпасть с рендером кусками
/// произвольной длины — иначе экспорт и playback звучали бы по-разному.
#[test]
fn t338_pitch_is_independent_of_chunking() {
    for engine in [PitchEngine::Wsola, PitchEngine::Vocoder] {
        let fx = vec![FxInstance::PitchShift { semitones: 5.0, cents: 0.0 }];
        let one = render(fx.clone(), engine);
        let many = render_chunked(fx, engine, &[1000, 17, 48_000, DUR - 49_017]);
        assert_eq!(one.len(), many.len());
        assert_eq!(one, many, "{engine:?}: нарезка буферов изменила результат");
    }
}

/// Тест 339: ВЫСОТА ЕДЕТ НА ЗАДАННЫЙ ИНТЕРВАЛ.
///
/// Синус 440 Гц, подъём на 7 полутонов: пик обязан встать на
/// 440·2^(7/12) ≈ 659.3 Гц у обоих движков, и он обязан доминировать
/// над исходной частотой (а не просто присутствовать).
#[test]
fn t339_pitch_shifts_the_frequency() {
    let from = 48_000usize / 2;
    for engine in [PitchEngine::Wsola, PitchEngine::Vocoder] {
        for st in [7.0f32, -5.0] {
            let out = render(vec![FxInstance::PitchShift { semitones: st, cents: 0.0 }], engine);
            let want = TONE * 2f64.powf(st as f64 / 12.0);
            let got = dominant_hz(&out, from);
            println!("t339 {engine:?} {st:+} st: пик {got:.1} Гц (ждём {want:.1})");
            assert!(
                (got - want).abs() / want < 0.02,
                "{engine:?} {st:+} st: пик {got:.1} Гц вместо {want:.1}"
            );
            let moved = band_energy(&out, from, want);
            let stayed = band_energy(&out, from, TONE);
            assert!(
                moved > stayed * 100.0,
                "{engine:?} {st:+} st: исходная частота не ушла ({moved:.3e} против {stayed:.3e})"
            );
        }
        // детерминизм между прогонами
        let a = render(vec![FxInstance::PitchShift { semitones: 3.0, cents: 20.0 }], engine);
        let b = render(vec![FxInstance::PitchShift { semitones: 3.0, cents: 20.0 }], engine);
        assert_eq!(a, b, "{engine:?}: рендер обязан быть детерминирован");
    }
}

/// Тест 340: ДВИЖОК — НАСТРОЙКА ПРОЕКТА.
///
/// Два движка звучат по-разному (иначе выбор бессмысленен), выбор
/// переживает сохранение и чтение файла, а смена откатывается undo —
/// это обычная правка модели, а не скрытая настройка приложения.
#[test]
fn t340_pitch_engine_is_a_project_setting() {
    let fx = vec![FxInstance::PitchShift { semitones: 7.0, cents: 0.0 }];
    let w = render(fx.clone(), PitchEngine::Wsola);
    let v = render(fx.clone(), PitchEngine::Vocoder);
    assert_ne!(w, v, "движки обязаны звучать по-разному");

    // смена — обычная команда модели: undo возвращает как было
    // (round-trip файла сторожит kt-otio t144/t147: поле едет секцией
    // настроек и по дефолту в файл не пишется)
    let mut m = TimelineModel::new(project(vec![], vec![], PitchEngine::Wsola));
    let mut settings = m.project().settings.clone();
    settings.pitch_engine = PitchEngine::Vocoder;
    m.apply(Command::SetSettings { settings }).expect("set settings");
    assert_eq!(m.project().settings.pitch_engine, PitchEngine::Vocoder);
    m.undo().expect("undo");
    assert_eq!(
        m.project().settings.pitch_engine,
        PitchEngine::Wsola,
        "undo обязан вернуть прежний движок"
    );
}
