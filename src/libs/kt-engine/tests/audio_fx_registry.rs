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
    let p = project(fx, auto, reversed);
    let r = AudioRenderer::new(&p);
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Clicks)));
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
        let base_for = |param: &str| -> FxInstance {
            match (f.tag(), param) {
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
            let base_out = render(vec![base.clone()], vec![], false, 0);
            let heard = probe_values(spec).into_iter().find(|probe| {
                let mut probed = base.clone();
                assert!(probed.set_param(spec.name, *probe));
                let p = render(vec![probed], vec![], false, 0);
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
            let curved = render(fx.clone(), curve_for(spec, probe), false, 0);
            let mut probed = (*f).clone();
            probed.set_param(spec.name, probe);
            let const_probe = render(vec![probed], vec![], false, 0);
            assert!(
                max_delta(&curved, &a) > 1e-4,
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
            let rev_plain = render(fx.clone(), vec![], true, 0);
            let rev_curved = render(fx.clone(), curve_for(spec, probe), true, 0);
            let rev_curved2 = render(fx.clone(), curve_for(spec, probe), true, 0);
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
