//! Тест 190 (Р.30): автоматизация параметров видео-FX.
//! curve_value (кламп краёв, линейность, ease), set/get_param по ВСЕМ
//! спецификациям каждого варианта, команды SetFxAuto/SetFx (подрезка),
//! валидация битых кривых, байт-стабильность файлов без автоматизации.

use kt_model::*;
use kt_time::Tick;

fn key(t: f64, v: f64) -> AutoKey {
    AutoKey { t, v }
}

#[test]
fn t190_curve_value_semantics() {
    let keys = vec![key(0.2, 10.0), key(0.8, 40.0)];
    // кламп краёв: до первой точки и после последней — крайние значения
    assert_eq!(curve_value(&keys, false, 0.0), 10.0);
    assert_eq!(curve_value(&keys, false, 0.2), 10.0);
    assert_eq!(curve_value(&keys, false, 1.0), 40.0);
    assert_eq!(curve_value(&keys, false, 5.0), 40.0, "u клампится в 0..1");
    // линейная середина сегмента
    assert!((curve_value(&keys, false, 0.5) - 25.0).abs() < 1e-9);
    // ease: середина совпадает (smoothstep(0.5)=0.5), четверть — нет
    assert!((curve_value(&keys, true, 0.5) - 25.0).abs() < 1e-9);
    let quarter_lin = curve_value(&keys, false, 0.35);
    let quarter_ease = curve_value(&keys, true, 0.35);
    assert!(quarter_ease < quarter_lin, "ease поджимает начало сегмента");
    // одна точка — константа
    assert_eq!(curve_value(&[key(0.5, 7.0)], true, 0.0), 7.0);
    assert_eq!(curve_value(&[key(0.5, 7.0)], true, 1.0), 7.0);
    // пустая кривая — ноль (не срабатывает через apply_fx_auto)
    assert_eq!(curve_value(&[], false, 0.5), 0.0);
}

/// Каждый параметр из param_specs обязан устанавливаться и читаться;
/// значения клампятся диапазоном; чужое имя — отказ (кривая-сирота).
#[test]
fn t190_set_get_param_all_specs() {
    let variants: Vec<FxInstance> = vec![
        FxInstance::Pan { mode: PanMode::Fit, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 },
        FxInstance::CamMotion { mode: PanMode::Fit, ease: true, time_scale: 1.0, keyframes: vec![] },
        FxInstance::Mirror { horizontal: true, vertical: false },
        FxInstance::Blur { radius: 8 },
        FxInstance::Greenscreen { key_r: 0, key_g: 255, key_b: 0, tolerance: 70, softness: 25 },
        FxInstance::Edges { threshold: 48 },
        FxInstance::Noise { amount: 0.1 },
        FxInstance::Vhs { rgb_shift: 4, bleed: 0.4 },
        FxInstance::Glow { strength: 0.65 },
        FxInstance::EventGrid { cols: 2, rows: 2, center: true },
        FxInstance::Pan3d {
            pos_x: 0.0,
            pos_y: 0.0,
            pos_z: 0.0,
            rot_x: 0.0,
            rot_y: 0.0,
            rot_z: 0.0,
            anchor_x: 0.5,
            anchor_y: 0.5,
            scale: 1.0,
            camera: None,
        },
        FxInstance::Gain { db: 0.0 },
    ];
    let mut specs_total = 0;
    for mut f in variants {
        for spec in f.param_specs() {
            specs_total += 1;
            let (name, lo, hi) = (spec.name, spec.min, spec.max);
            // дефолт спеки обязан лежать в её же диапазоне
            assert!(
                (lo..=hi).contains(&spec.default),
                "{name}: дефолт {} вне [{lo}, {hi}]",
                spec.default
            );
            assert!(spec.step > 0.0, "{name}: шаг обязан быть положительным");
            // середина диапазона ставится и читается (с точностью до
            // округления целочисленных полей)
            let mid = (lo + hi) / 2.0;
            assert!(f.set_param(name, mid), "{name} не установился");
            let got = f.get_param(name).unwrap();
            assert!((got - mid).abs() <= 0.5, "{name}: {got} vs {mid}");
            // выход за диапазон клампится
            assert!(f.set_param(name, hi + 1e6));
            assert!(f.get_param(name).unwrap() <= hi + 1e-9, "{name} не клампится");
            assert!(f.set_param(name, lo - 1e6));
            assert!(f.get_param(name).unwrap() >= lo - 1e-9);
        }
        // чужое имя — отказ, значение не тронуто
        assert!(!f.set_param("no_such_param", 1.0));
        assert_eq!(f.get_param("no_such_param"), None);
    }
    assert!(specs_total >= 20, "спеки покрывают все видео-варианты: {specs_total}");
    // у зеркала числовых параметров нет вовсе
    assert!(FxInstance::Mirror { horizontal: true, vertical: false }.param_specs().is_empty());
    // Р.33: аудио получило спеки — автоматизируется тем же механизмом
    let gain = FxInstance::Gain { db: 0.0 };
    assert_eq!(gain.auto_params().count(), 1, "громкость обязана автоматизироваться");
    let slowed = FxInstance::slowed_reverb(1.0, 0.3, 60.0);
    assert!(
        slowed.auto_params().any(|s| s.name == "rate"),
        "скорость slowed&reverb обязана быть автоматизируемой"
    );
    assert_eq!(
        slowed.param_spec("reverb_delay_ms").map(|s| s.kind),
        Some(ParamKind::Structural),
        "длина линий реверба — структурный параметр, кривой не подлежит"
    );
    assert!(
        !slowed.auto_params().any(|s| s.name == "reverb_delay_ms"),
        "структурный параметр не предлагается для автоматизации"
    );
}

#[test]
fn t190_apply_fx_auto() {
    let mut f = FxInstance::Blur { radius: 8 };
    let curves = vec![
        ParamCurve { param: "radius".into(), ease: false, keys: vec![key(0.0, 1.0), key(1.0, 47.0)] },
        ParamCurve { param: "orphan".into(), ease: false, keys: vec![key(0.0, 5.0)] },
        ParamCurve { param: "empty".into(), ease: false, keys: vec![] },
    ];
    assert_eq!(apply_fx_auto(&mut f, &curves, 0.5), 1, "сироты и пустые не считаются");
    assert_eq!(f.get_param("radius"), Some(24.0), "лерп 1..47 в середине");
}

fn clip_with_fx() -> ClipEvent {
    ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::Blur { radius: 8 }, FxInstance::Glow { strength: 0.5 }],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
    }
}

fn model_with_clip() -> TimelineModel {
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.clips.push(clip_with_fx());
    p.validate().unwrap();
    TimelineModel::new(p)
}

#[test]
fn t190_commands_undo_and_truncation() {
    let mut m = model_with_clip();
    let auto = vec![
        vec![ParamCurve {
            param: "radius".into(),
            ease: false,
            keys: vec![key(0.0, 1.0), key(1.0, 48.0)],
        }],
        vec![ParamCurve {
            param: "strength".into(),
            ease: true,
            keys: vec![key(0.0, 0.0), key(1.0, 2.0)],
        }],
    ];
    assert!(m.apply(Command::SetFxAuto { id: ClipId(1), auto: auto.clone() }).is_ok());
    assert_eq!(m.project().clips[0].fx_auto, auto);
    // битые кривые не проходят: не отсортированы / t вне 0..1 / не конечны
    for bad_keys in [
        vec![key(0.9, 1.0), key(0.1, 2.0)],
        vec![key(-0.1, 1.0)],
        vec![key(0.5, f64::NAN)],
    ] {
        let bad = vec![vec![ParamCurve { param: "radius".into(), ease: false, keys: bad_keys }]];
        assert!(
            m.apply(Command::SetFxAuto { id: ClipId(1), auto: bad }).is_err(),
            "битая кривая обязана отклоняться"
        );
    }
    // SetFx с укороченной цепочкой подрезает кривые лишних слотов
    assert!(m
        .apply(Command::SetFx { id: ClipId(1), fx: vec![FxInstance::Blur { radius: 8 }] })
        .is_ok());
    assert_eq!(m.project().clips[0].fx_auto.len(), 1, "кривые второго слота отпали");
    assert_eq!(m.project().clips[0].fx_auto[0][0].param, "radius");
    // undo возвращает и цепочку, и кривые
    m.undo().unwrap();
    assert_eq!(m.project().clips[0].fx.len(), 2);
    assert_eq!(m.project().clips[0].fx_auto, auto);
}

#[test]
fn t190_files_without_auto_are_byte_stable() {
    let m = model_with_clip();
    let json = serde_json::to_string(m.project()).unwrap();
    assert!(!json.contains("fx_auto"), "пустая автоматизация не сериализуется: {json}");
    // active_fx_auto выравнивается с active_fx при bypass
    let mut c = clip_with_fx();
    c.fx_auto = vec![
        vec![ParamCurve { param: "radius".into(), ease: false, keys: vec![key(0.0, 1.0)] }],
        vec![ParamCurve { param: "strength".into(), ease: false, keys: vec![key(0.0, 1.0)] }],
    ];
    c.fx_bypass = vec![true]; // первый слот выключен
    let fx = c.active_fx();
    let auto = c.active_fx_auto();
    assert_eq!(fx.len(), 1);
    assert_eq!(auto.len(), 1);
    assert_eq!(auto[0][0].param, "strength", "кривая уехала ВМЕСТЕ со своим слотом");
}
