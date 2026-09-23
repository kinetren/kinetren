//! Тест 696 (Р.61): у скорости варпа форма отрезка только ПРЯМАЯ.
//!
//! Замкнутая форма обратной карты растяжения стоит на кусочно-линейном
//! профиле: интеграл от третьего порядка был бы четвёртой степени, а от
//! зигзага не выражается вовсе. Правило существовало и до Р.61 (прежнее
//! булево сглаживание у `rate` молча выбрасывалось) — новая форма обязана
//! не выбрасываться молча, а ОТКАЗЫВАТЬ: тихое выбрасывание показало бы в
//! редакторе одно, а в звуке другое.

use kt_model::*;
use kt_time::Tick;

const LEN: Tick = Tick(90_000);

fn clip_with(rate_keys: Vec<AutoKey>) -> ClipEvent {
    ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick::ZERO,
        duration: LEN,
        source_in: Tick::ZERO,
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::slowed_reverb(0.8, 0.0, 85.0)],
        fx_bypass: vec![false],
        fx_auto: vec![vec![ParamCurve {
            param: "rate".into(),
            keys: rate_keys,
            source: CurveSource::Keys,
        }]],
        opacity: 1.0,
        fx_off: false,
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
    }
}

#[test]
fn t696_speed_curve_takes_the_linear_shape_only() {
    let straight = vec![
        AutoKey::shaped(Tick(0), 0.5, SegShape::Linear),
        AutoKey::shaped(LEN, 2.0, SegShape::Linear),
    ];

    // прямая законна
    assert!(
        check_fx_auto(ClipId(1), &clip_with(straight.clone()).fx_auto).is_ok(),
        "кусочно-линейная скорость обязана проходить"
    );

    // любая другая форма — ОТКАЗ, и он назван своим именем
    for shape in [
        SegShape::DEFAULT,
        SegShape::Hold,
        SegShape::EaseIn { tension: 1.0 },
        SegShape::EaseOut { tension: 1.0 },
        SegShape::Zigzag { periods: 3.0, amplitude: 0.2, profile: ZigProfile::Triangle },
    ] {
        let keys = vec![AutoKey::shaped(Tick(0), 0.5, shape), AutoKey::shaped(LEN, 2.0, shape)];
        let err = check_fx_auto(ClipId(1), &clip_with(keys).fx_auto).unwrap_err();
        assert!(
            matches!(err, ProjectError::RateShapeMustBeLinear(_)),
            "форма {shape:?} на скорости прошла молча или отвергнута не тем: {err:?}"
        );
        // текст отказа называет ПРИЧИНУ, а не «bad curve»
        let text = err.to_string();
        assert!(
            text.contains("linear") && text.contains("warp"),
            "отказ не объясняет, почему именно прямая: {text}"
        );
    }

    // форма ДРУГОГО параметра того же слота законна: правило про скорость,
    // а не про слот целиком
    let mut c = clip_with(straight);
    c.fx_auto[0].push(ParamCurve {
        param: "mix".into(),
        keys: vec![
            AutoKey::shaped(
                Tick(0),
                0.0,
                SegShape::Zigzag { periods: 4.0, amplitude: 0.1, profile: ZigProfile::Sine },
            ),
            AutoKey::at(LEN, 1.0),
        ],
        source: CurveSource::Keys,
    });
    assert!(
        check_fx_auto(ClipId(1), &c.fx_auto).is_ok(),
        "зигзаг на mix отвергнут — правило уехало со скорости на весь слот"
    );

    // и КАРТА варпа считает по прямой те же числа, что и прежде:
    // профиль скорости формы не видит
    let len = LEN.to_audio_frames();
    let prof = AutoProfiles::from_curves(&c.fx_auto, 0, len);
    let at = |s: f64| prof.value(0, "rate", 0.8, s);
    assert!((at(0.0) - 0.5).abs() < 1e-9, "начало профиля уехало: {}", at(0.0));
    assert!((at(1.0) - 2.0).abs() < 1e-9, "конец профиля уехал: {}", at(1.0));
    assert!(
        (at(0.5) - 1.25).abs() < 1e-9,
        "середина профиля скорости обязана быть ЛИНЕЙНОЙ: {}",
        at(0.5)
    );
}
