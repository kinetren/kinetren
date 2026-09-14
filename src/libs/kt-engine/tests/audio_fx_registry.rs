//! Тест 322 (Р.47, п.326): ПАРАМЕТРИЗОВАННЫЙ каркас по реестру аудио-FX.
//! Каждый аудио-вид из `FxInstance::all()` автоматически проходит один и
//! тот же набор проверок: дефолт звучит и детерминирован, каждый
//! Smooth-параметр слышен, кривая отрабатывает, реверс с кривой
//! детерминирован, повторный seek байт-в-байт, хвост согласован между
//! `chain_tail` (без state) и `ClipProcessor::tail()` (со state).
//! Замок полноты: новый эффект, добавленный в `all()`, попадает сюда сам;
//! не добавленный — не соберётся из-за исчерпывающего `tag()`.

use kt_engine::{AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const DUR: i64 = 48_000 * 2;

fn project(fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>, reversed: bool) -> Project {
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 64, height: 36, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:clicks".into() });
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
        reversed,
        fx,
        fx_bypass: vec![],
        fx_auto: auto,
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

/// Рендер клипа; стимул — широкополосные СТЕРЕО-РАЗЛИЧНЫЕ щелчки: у
/// фильтров есть что резать, у стерео-параметров — s-компонента, у
/// кривых — события по всей длине клипа.
fn render(fx: Vec<FxInstance>, auto: Vec<Vec<ParamCurve>>, reversed: bool, from: i64) -> Vec<f32> {
    render_on(fx, auto, reversed, from, AudioPattern::Clicks)
}

/// Тот же рендер на выбранном стимуле: динамика (гейт/компрессор) меряется
/// на щелчках С ФОНОМ — на нулевом фоне hold/release принципиально
/// неслышимы (нечего глушить между событиями).
fn render_on(
    fx: Vec<FxInstance>,
    auto: Vec<Vec<ParamCurve>>,
    reversed: bool,
    from: i64,
    pattern: AudioPattern,
) -> Vec<f32> {
    let p = project(fx, auto, reversed);
    let r = AudioRenderer::new(&p);
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), pattern)));
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let frames = (DUR - from) as usize;
    let mut out = vec![0f32; frames * CH];
    let stats = r.render_frames_with(&mut f, &mut store, from, &mut out);
    assert_eq!(stats.underruns, 0, "underrun on a fully available source");
    out
}

fn max_delta(a: &[f32], b: &[f32]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y).abs() as f64).fold(0.0, f64::max)
}

fn rms(s: &[f32]) -> f64 {
    (s.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / s.len().max(1) as f64).sqrt()
}

/// Кривая одного параметра слота 0: default → пробное значение к концу.
fn curve_for(spec: &ParamSpec, probe: f64) -> Vec<Vec<ParamCurve>> {
    vec![vec![ParamCurve {
        param: spec.name.into(),
        ease: false,
        keys: vec![
            AutoKey { t: Tick(0), v: spec.default },
            AutoKey { t: Tick::from_audio_frames(DUR), v: probe },
        ],
    }]]
}

/// Кандидаты пробы: оба конца диапазона (дальний первым). У параметра
/// бывает «прозрачный» край (rate_hz=48000 у крашера — сетка в каждый
/// кадр), поэтому слышимость требуется ХОТЯ БЫ от одного конца, а кривая
/// строится на слышимом.
fn probe_values(spec: &ParamSpec) -> [f64; 2] {
    if (spec.default - spec.max).abs() > (spec.default - spec.min).abs() {
        [spec.max, spec.min]
    } else {
        [spec.min, spec.max]
    }
}

#[test]
fn t322_audio_registry_contract() {
    let audio: Vec<&FxInstance> = FxInstance::all().iter().filter(|f| f.is_audio()).collect();
    assert!(audio.len() >= 2, "registry must know the audio effects");
    let mut covered: Vec<&'static str> = Vec::new();

    let dry = render(vec![], vec![], false, 0);
    for f in &audio {
        covered.push(f.tag());
        let fx = vec![(*f).clone()];

        // (а) дефолт: не тишина, детерминирован между прогонами
        let a = render(fx.clone(), vec![], false, 0);
        assert!(rms(&a) > 1e-4, "{}: default renders silence", f.tag());
        let b = render(fx.clone(), vec![], false, 0);
        assert_eq!(a, b, "{}: default render is not deterministic", f.tag());

        // (б) каждый Smooth-параметр слышен от пробного значения.
        // Энейблеры: параметр, неслышимый на дефолтах соседей (частота
        // полосы EQ при нулевом гейне), пробуется на базе, где сосед
        // включён, — таблица с комментарием на каждую строку.
        let stim = match f.tag() {
            "gate" | "compressor" => AudioPattern::ClicksBed,
            _ => AudioPattern::Clicks,
        };
        let base_for = |param: &str| -> FxInstance {
            match (f.tag(), param) {
                // детектор с дефолтной атакой 10 мс не успевает на
                // 4-кадровом щелчке — пробы компрессора идут с быстрой
                ("compressor", _) => {
                    let mut e = (*f).clone();
                    e.set_param("attack_ms", 0.1);
                    e
                }
                // дефолтный порог -40 ниже фона стимула (-34): гейт всегда
                // открыт и рампы/hold не участвуют — пробы идут при пороге
                // между фоном и щелчками
                ("gate", "hold_ms") | ("gate", "release_ms") | ("gate", "attack_ms") => {
                    let mut e = (*f).clone();
                    e.set_param("threshold_db", -25.0);
                    e
                }
                // частота/добротность полосы слышны только при ненулевом
                // гейне этой полосы
                ("eq", "low_freq_hz") => {
                    let mut e = (*f).clone();
                    e.set_param("low_gain_db", -12.0);
                    e
                }
                ("eq", "mid_freq_hz") | ("eq", "mid_q") => {
                    let mut e = (*f).clone();
                    e.set_param("mid_gain_db", -12.0);
                    e
                }
                ("eq", "high_freq_hz") => {
                    let mut e = (*f).clone();
                    e.set_param("high_gain_db", -12.0);
                    e
                }
                _ => (*f).clone(),
            }
        };
        let mut audible_probe: Vec<(&'static str, f64)> = Vec::new();
        for spec in f.auto_params() {
            let base = base_for(spec.name);
            let base_out = render_on(vec![base.clone()], vec![], false, 0, stim);
            let heard = probe_values(spec).into_iter().find(|probe| {
                let mut probed = base.clone();
                assert!(probed.set_param(spec.name, *probe));
                let p = render_on(vec![probed], vec![], false, 0, stim);
                max_delta(&base_out, &p) > 1e-4
            });
            let heard = heard.unwrap_or_else(|| {
                panic!("{}.{}: no end of the range is audible", f.tag(), spec.name)
            });
            audible_probe.push((spec.name, heard));
        }

        // (в) кривая первого Smooth-параметра отрабатывает: выход не равен
        // ни константному дефолту, ни константной пробе
        if let Some(spec) = f.auto_params().next() {
            let probe = audible_probe
                .iter()
                .find(|(n, _)| *n == spec.name)
                .map(|(_, v)| *v)
                .unwrap();
            let curved = render_on(fx.clone(), curve_for(spec, probe), false, 0, stim);
            let mut probed = (*f).clone();
            probed.set_param(spec.name, probe);
            let const_probe = render_on(vec![probed], vec![], false, 0, stim);
            let default_on_stim = render_on(fx.clone(), vec![], false, 0, stim);
            assert!(
                max_delta(&curved, &default_on_stim) > 1e-4,
                "{}.{}: curve equals the default render",
                f.tag(),
                spec.name
            );
            assert!(
                max_delta(&curved, &const_probe) > 1e-4,
                "{}.{}: curve equals the constant probe",
                f.tag(),
                spec.name
            );

            // (г) реверс + кривая: детерминирован и отличается от реверса
            // без кривой (движковый t271-кейс: до Р.39 никто не сочетал
            // reversed с непустым fx_auto)
            let rev_plain = render_on(fx.clone(), vec![], true, 0, stim);
            let rev_curved = render_on(fx.clone(), curve_for(spec, probe), true, 0, stim);
            let rev_curved2 = render_on(fx.clone(), curve_for(spec, probe), true, 0, stim);
            assert_eq!(rev_curved, rev_curved2, "{}: reversed+curve not deterministic", f.tag());
            assert!(
                max_delta(&rev_curved, &rev_plain) > 1e-4,
                "{}.{}: curve is lost on a reversed clip",
                f.tag(),
                spec.name
            );
        }

        // (д) повторный seek в ту же позицию со свежим store — байт-в-байт
        let s1 = render(fx.clone(), vec![], false, DUR / 3);
        let s2 = render(fx.clone(), vec![], false, DUR / 3);
        assert_eq!(s1, s2, "{}: seek render is not reproducible", f.tag());

        // (е) хвост: формула без state == формула процессора (относительный
        // допуск 2% — целочисленные усечения длин линий)
        let auto = AutoProfiles::from_curves_rev(&[vec![]], 0, DUR, false);
        let ct = kt_fx::chain_tail(&fx, &auto).0 as f64;
        let proc = kt_fx::ClipProcessor::new(&fx, &auto, 0, DUR);
        let pt = proc.tail().0 as f64;
        let diff = (ct - pt).abs();
        assert!(
            diff <= (ct.max(pt) * 0.02).max(200.0),
            "{}: chain_tail {ct} vs processor tail {pt}",
            f.tag()
        );
    }

    // LayerMix — слот силы, в одиночной цепочке он смешивает вход с самим
    // собой: контракт «слышимости» к нему неприменим, но детерминизм и
    // нейтральность обязаны держаться
    let lm = render(vec![FxInstance::LayerMix { amount: 0.5, from: 0 }], vec![], false, 0);
    assert_eq!(lm, dry, "solo LayerMix must be a no-op (dry == wet)");

    // замок полноты: покрыт каждый аудио-вид реестра
    let all_tags: std::collections::BTreeSet<&str> =
        audio.iter().map(|f| f.tag()).collect();
    let covered: std::collections::BTreeSet<&str> = covered.into_iter().collect();
    assert_eq!(covered, all_tags, "every audio kind of all() must pass this contract");
}

/// Тест 325 (Р.47, п.328): детерминизм фазы LFO. Фаза — интеграл кривой
/// частоты по ПОЗИЦИИ клипа, не счётчик обработанного: рендер одним
/// куском равен рендеру кусками, seek равен холодному рендеру, а кривая
/// частоты ускоряет модуляцию к концу клипа.
#[test]
fn t325_lfo_phase_is_a_function_of_position() {
    let trem = vec![FxInstance::Tremolo { rate_hz: 5.0, depth: 1.0 }];

    // (а) куском == кусками: фаза не зависит от нарезки буферов
    let whole = render(trem.clone(), vec![], false, 0);
    let p = project(trem.clone(), vec![], false);
    let r = AudioRenderer::new(&p);
    let mut f = DirectFetcher::new(None);
    f.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Clicks)),
    );
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut pieces = vec![0f32; DUR as usize * CH];
    let third = (DUR / 3) as usize * CH;
    let n0 = r.render_frames_with(&mut f, &mut store, 0, &mut pieces[..third]);
    assert_eq!(n0.underruns, 0);
    let n1 =
        r.render_frames_with(&mut f, &mut store, DUR / 3, &mut pieces[third..2 * third]);
    assert_eq!(n1.underruns, 0);
    let n2 = r.render_frames_with(&mut f, &mut store, 2 * (DUR / 3), &mut pieces[2 * third..]);
    assert_eq!(n2.underruns, 0);
    assert_eq!(whole, pieces, "LFO phase must not depend on buffer slicing");

    // (б) синус под тремоло: кривая rate_hz 2 -> 8 Гц ускоряет модуляцию
    // (счёт минимумов огибающей во второй половине больше, чем в первой)
    let sine_render = |auto: Vec<Vec<ParamCurve>>| -> Vec<f32> {
        let mut p = project(vec![FxInstance::Tremolo { rate_hz: 2.0, depth: 1.0 }], auto, false);
        p.media[0].path = "synth:sine:440".into();
        let r = AudioRenderer::new(&p);
        let mut f = DirectFetcher::new(None);
        f.add_source(
            MediaId(1),
            Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: 440.0 })),
        );
        let mut store = FxStateStore::new();
        r.sync_fx(&mut store);
        let mut out = vec![0f32; DUR as usize * CH];
        assert_eq!(r.render_frames_with(&mut f, &mut store, 0, &mut out).underruns, 0);
        out
    };
    let curve = vec![vec![ParamCurve {
        param: "rate_hz".into(),
        ease: false,
        keys: vec![
            AutoKey { t: Tick(0), v: 2.0 },
            AutoKey { t: Tick::from_audio_frames(DUR), v: 8.0 },
        ],
    }]];
    let swept = sine_render(curve);
    // огибающая по окнам 4 мс; минимумы = провалы тремоло
    let env: Vec<f64> = swept
        .chunks(192 * CH)
        .map(|w| w.iter().map(|v| (*v as f64).abs()).fold(0.0, f64::max))
        .collect();
    let dips = |e: &[f64]| -> usize {
        let mut n = 0;
        for i in 1..e.len() - 1 {
            if e[i] < 0.2 && e[i] <= e[i - 1] && e[i] < e[i + 1] {
                n += 1;
            }
        }
        n
    };
    let half = env.len() / 2;
    let (d1, d2) = (dips(&env[..half]), dips(&env[half..]));
    assert!(
        d2 > d1 + 1,
        "rate curve 2->8 Hz must speed the modulation up: {d1} dips then {d2}"
    );
}

/// Тест 326 (Р.47, п.329): эхо и динамика делают то, что написано на
/// ручках — позиция повтора, серия затуханий, ping-pong в противоканале,
/// закон компрессии, глушение гейтом.
#[test]
fn t326_delay_and_dynamics_behave() {
    // --- Delay: повтор ровно на time_ms, серия убывает, хвост честен ----
    let time_ms = 250.0f32;
    let one = render_on(
        vec![FxInstance::Delay { time_ms, feedback: 0.6, mix: 1.0, pingpong: false }],
        vec![],
        false,
        0,
        AudioPattern::Impulse,
    );
    let frames = |ms: f32| ((ms / 1000.0) * 48_000.0) as usize;
    let at = |out: &[f32], f: usize, ch: usize| out[f * CH + ch].abs();
    // повторы в левом канале: на 250/500/750 мс, убывают
    let (e1, e2, e3) =
        (at(&one, frames(250.0), 0), at(&one, frames(500.0), 0), at(&one, frames(750.0), 0));
    assert!(e1 > 0.2, "first echo at time_ms: {e1}");
    assert!(e2 > 0.05 && e2 < e1, "second echo decays: {e2} vs {e1}");
    assert!(e3 < e2, "series keeps decaying: {e3} vs {e2}");
    // между повторами тихо
    assert!(at(&one, frames(125.0), 0) < 1e-3, "silence between echoes");

    // ping-pong: щелчки Clicks чередуются по каналам — первый (n=0..3)
    // только ЛЕВЫЙ. Время 230 мс выбрано так, чтобы повторы (11040,
    // 22080) не совпадали с щелчками источника (сетка 4800).
    let pp = render_on(
        vec![FxInstance::Delay { time_ms: 230.0, feedback: 0.6, mix: 1.0, pingpong: true }],
        vec![],
        false,
        0,
        AudioPattern::Clicks,
    );
    let win = |out: &[f32], f: usize, ch: usize| -> f64 {
        (f.saturating_sub(4)..f + 8).map(|i| out[i * CH + ch].abs() as f64).sum()
    };
    // первый повтор левого щелчка — по-прежнему в левом (читается своя
    // линия), второй — перекрёстной связью уехал в правый
    let (l1, r1) = (win(&pp, frames(230.0), 0), win(&pp, frames(230.0), 1));
    assert!(l1 > 0.5 && r1 < l1 * 0.1, "first pp echo stays in its line: {l1}/{r1}");
    let (l2, r2) = (win(&pp, frames(460.0), 0), win(&pp, frames(460.0), 1));
    assert!(r2 > 0.1 && l2 < r2 * 0.2, "second pp echo crosses channels: {l2}/{r2}");

    // --- Compressor: выход ~ thr + (in-thr)/ratio ----------------------
    let comp = |fx: Vec<FxInstance>| -> f64 {
        let out = render_on(fx, vec![], false, 0, AudioPattern::Sine { hz: 440.0 });
        // стабильный участок после атаки: вторая половина
        let tail = &out[out.len() / 2..];
        let rms = (tail.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>()
            / tail.len() as f64)
            .sqrt();
        20.0 * (rms * std::f64::consts::SQRT_2).log10()
    };
    let dry_db = comp(vec![]);
    let comp_db = comp(vec![FxInstance::Compressor {
        threshold_db: -18.0,
        ratio: 4.0,
        attack_ms: 1.0,
        release_ms: 150.0,
        makeup_db: 0.0,
    }]);
    // синус 0.5 = -6 дБFS: ожидаем -18 + ( -6 - -18 )/4 = -15 дБFS
    let expect = -18.0 + (dry_db + 18.0) / 4.0;
    assert!(
        (comp_db - expect).abs() < 1.5,
        "compression law: got {comp_db:.1} dB, expected {expect:.1} dB (dry {dry_db:.1})"
    );

    // --- Gate: тихий синус глушится, громкий проходит ------------------
    let g = |amp_db: f32| -> f64 {
        let hz = 440.0;
        let out = render_on(
            vec![
                FxInstance::gain(amp_db),
                FxInstance::Gate {
                    threshold_db: -30.0,
                    attack_ms: 1.0,
                    hold_ms: 40.0,
                    release_ms: 50.0,
                },
            ],
            vec![],
            false,
            0,
            AudioPattern::Sine { hz },
        );
        let tail = &out[out.len() / 2..];
        (tail.iter().map(|v| (*v as f64) * (*v as f64)).sum::<f64>() / tail.len() as f64).sqrt()
    };
    let loud = g(0.0);
    let quiet = g(-40.0);
    assert!(loud > 0.2, "loud sine passes the gate: {loud}");
    assert!(quiet < 1e-4, "quiet sine is gated: {quiet}");
}

/// Тест 327 (Р.47, п.330): цена плотной цепочки — ПЕЧАТЬ, не ассерт
/// (о скорости судят по release-прогону: п.301). Семь событий, у каждого
/// цепочка из шести узлов.
#[test]
#[ignore]
fn t327_print_dense_chain_cost() {
    let chain = || {
        vec![
            FxInstance::filter(),
            FxInstance::eq(),
            FxInstance::chorus(),
            FxInstance::delay(),
            FxInstance::compressor(),
            FxInstance::gain(-3.0),
        ]
    };
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 64, height: 36, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:clicks".into() });
    for i in 0..7 {
        p.clips.push(ClipEvent {
            id: ClipId(i + 1),
            link_group: None,
            kind: ClipKind::Audio,
            lane: i as u32 % 3,
            start: Tick((i as i64) * 90_000 / 2),
            duration: Tick(90_000 * 8),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: chain(),
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.validate().unwrap();
    let r = AudioRenderer::new(&p);
    let mut f = DirectFetcher::new(None);
    f.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Clicks)),
    );
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let secs = 10i64;
    let mut out = vec![0f32; (48_000 * secs) as usize * CH];
    let t = std::time::Instant::now();
    let stats = r.render_frames_with(&mut f, &mut store, 0, &mut out);
    let ms = t.elapsed().as_secs_f64() * 1e3;
    assert_eq!(stats.underruns, 0);
    println!(
        "t327: 7 events x 6-node chains, {secs} s of audio in {ms:.1} ms ({:.1} ms per second, {:.1}% of realtime)",
        ms / secs as f64,
        ms / (secs as f64 * 1000.0) * 100.0
    );
}
