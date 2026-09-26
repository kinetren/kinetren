//! Тест 701 (Р.61): вычислитель мини-формулы — замкнутый, детерминированный
//! и объясняющий свои отказы.
//!
//! Решение владельца (Р.59): «Выражения — своя мини-формула, считается
//! вживую на каждом кадре; внешних интерпретаторов нет». Из инварианта
//! детерминизма следует остальное: набор операций закрыт, окружения в языке
//! нет, а отказ НАЗЫВАЕТСЯ — молчаливый ноль показал бы человеку картинку,
//! которой он поверит.

use kt_model::{Formula, FormulaErrorKind};
use std::collections::BTreeMap;

fn vars(pairs: &[(&str, f64)]) -> BTreeMap<String, f64> {
    pairs.iter().map(|(k, v)| ((*k).to_string(), *v)).collect()
}

#[test]
fn t701_formula_is_closed_deterministic_and_names_its_refusals() {
    let v = vars(&[("t", 2.0), ("x", 3.0), ("blur.radius", 8.0)]);

    // --- НАБОР операций считается и считается ОДИНАКОВО ---------------
    let cases: &[(&str, f64)] = &[
        ("1 + 2 * 3", 7.0),
        ("(1 + 2) * 3", 9.0),
        ("-x", -3.0),
        ("x - -x", 6.0),
        ("7 % 4", 3.0),
        ("t < x", 1.0),
        ("t > x", 0.0),
        ("t <= 2", 1.0),
        ("x != 3", 0.0),
        ("if(t < x, 10, 20)", 10.0),
        ("if(t > x, 10, 20)", 20.0),
        ("min(t, x)", 2.0),
        ("max(t, x)", 3.0),
        ("clamp(99, 0, 10)", 10.0),
        ("abs(0 - x)", 3.0),
        ("floor(2.7)", 2.0),
        ("ceil(2.1)", 3.0),
        ("round(2.5)", 3.0),
        ("sqrt(9)", 3.0),
        ("lerp(0, 10, 0.25)", 2.5),
        // адрес свойства — обычное имя с точкой
        ("blur.radius / 2", 4.0),
    ];
    for (src, want) in cases {
        let f = Formula::parse(src).unwrap_or_else(|e| panic!("{src:?} не разобралось: {e}"));
        let got = f.eval(&v).unwrap_or_else(|e| panic!("{src:?} не посчиталось: {e}"));
        assert!((got - want).abs() < 1e-12, "{src:?} дало {got}, ждали {want}");
        // ДВА прогона байт-в-байт
        assert_eq!(f.eval(&v).unwrap().to_bits(), got.to_bits(), "{src:?} не детерминировано");
    }

    // шум детерминирован и ограничен, и зависит от ЦЕЛОГО зерна
    let n = Formula::parse("noise(t)").expect("шум разбирается");
    let a = n.eval(&v).expect("шум считается");
    assert_eq!(a.to_bits(), n.eval(&v).unwrap().to_bits(), "шум не детерминирован");
    assert!((-1.0..=1.0).contains(&a), "шум вне диапазона: {a}");
    let b = n.eval(&vars(&[("t", 9.0)])).unwrap();
    assert_ne!(a.to_bits(), b.to_bits(), "шум не зависит от зерна");

    // --- ЧЕТЫРЕ отказа, и у каждого СВОЯ причина ----------------------
    let unknown = Formula::parse("нет_такого + 1")
        .err()
        .or_else(|| Formula::parse("qqq").ok().and_then(|f| f.eval(&v).err()))
        .expect("неизвестное имя обязано быть отказом");
    assert!(
        matches!(unknown.kind, FormulaErrorKind::UnknownName(_) | FormulaErrorKind::BadChar(_)),
        "неизвестное имя отвергнуто не тем: {unknown:?}"
    );

    let div = Formula::parse("x / (t - 2)").expect("разбирается");
    let e = div.eval(&v).expect_err("деление на ноль обязано быть ОТКАЗОМ, а не бесконечностью");
    assert_eq!(e.kind, FormulaErrorKind::DivZero, "{e:?}");

    let open = Formula::parse("(1 + 2").expect_err("незакрытая скобка обязана быть отказом");
    assert_eq!(open.kind, FormulaErrorKind::Unclosed, "{open:?}");

    // окружения в языке НЕТ сознательно: ни часов, ни случайности
    for src in ["now()", "now", "rand()", "time()"] {
        let bad = Formula::parse(src)
            .err()
            .or_else(|| Formula::parse(src).ok().and_then(|f| f.eval(&v).err()))
            .unwrap_or_else(|| panic!("{src:?} прошло — окружение просочилось в язык"));
        assert!(
            matches!(bad.kind, FormulaErrorKind::UnknownName(_)),
            "{src:?} отвергнуто не как неизвестное имя: {bad:?}"
        );
    }

    // --- ни один отказ не даёт МОЛЧАЛИВОГО НУЛЯ -----------------------
    for src in ["1/0", "(2", "wat(1)", "min(1)"] {
        let r = Formula::parse(src).and_then(|f| f.eval(&v));
        assert!(r.is_err(), "{src:?} посчиталось вместо отказа: {r:?}");
        let text = r.unwrap_err().to_string();
        assert!(text.len() > 12 && text.contains("formula"), "отказ без причины: {text}");
    }

    // позиция в строке названа: поле ввода подсветит её
    let e = Formula::parse("1 + $").expect_err("мусор обязан быть отказом");
    assert!(e.at >= 4, "позиция ошибки не указана: {e:?} ({})", e.at);

    // --- какие ИМЕНА читает выражение, известно ДО счёта ---------------
    let f = Formula::parse("blur.radius * 2 + t").expect("разбирается");
    assert_eq!(f.reads, vec!["blur.radius".to_string(), "t".to_string()], "{:?}", f.reads);
    let none = Formula::parse("1 + 2").expect("разбирается");
    assert!(none.reads.is_empty(), "константа зря объявила зависимости: {:?}", none.reads);
}

/// Тест 784 (Р.67): `t` формулы — секунды от НАЧАЛА МАТЕРИАЛА, и в
/// предпросмотре тот же: рендер на материале 12 с при точке входа 10 с
/// даёт 2.0, а предпросмотр для двух разных окон совпадает с рендером в
/// тех же тиках. На прежнем коде предпросмотр считал `t` от левого края
/// окна — линия врала и уезжала при прокрутке.
#[test]
fn t784_formula_time_counts_from_material_start_everywhere() {
    use kt_model::*;
    use kt_time::Tick;
    let curve = ParamCurve {
        param: "amount".into(),
        keys: vec![],
        source: CurveSource::Formula { expr: "t".into() },
    };
    let clip = ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(10 * 90_000),
        source_in: Tick(10 * 90_000),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
        fx_bypass: vec![],
        fx_auto: vec![vec![curve.clone()]],
        opacity: 1.0,
        fx_off: false,
    };
    let step = 3000;
    let out = resolve_derived(std::slice::from_ref(&clip), step);
    let rendered = &out[0].fx_auto[0][0].keys;
    let at = Tick(12 * 90_000);
    let v = curve_value(rendered, at);
    assert!((v - 2.0).abs() < 1e-9, "рендер: {v}, ждали 2.0 (не 12 и не 0)");
    // предпросмотр: два окна — одна линия, равная рендеру
    let material = (clip.source_in, Tick(clip.source_in.0 + clip.duration.0));
    let win_a = formula_preview(&curve, material, Tick(11 * 90_000), Tick(13 * 90_000), step);
    let win_b = formula_preview(&curve, material, Tick(5 * 90_000), Tick(20 * 90_000), step);
    for k in win_a.iter().chain(win_b.iter()) {
        let r = curve_value(rendered, k.t);
        assert!((k.v - r).abs() < 1e-9, "предпросмотр {} ≠ рендер {} на тике {}", k.v, r, k.t.0);
    }
    let a12 = win_a.iter().find(|k| k.t == at).map(|k| k.v).unwrap();
    let b12 = win_b.iter().find(|k| k.t == at).map(|k| k.v).unwrap();
    assert!((a12 - 2.0).abs() < 1e-9 && (b12 - 2.0).abs() < 1e-9, "окна разошлись: {a12} / {b12}");
    // `k` в предпросмотре тоже есть
    let with_keys = ParamCurve {
        keys: vec![AutoKey::at(Tick(0), 1.0), AutoKey::at(Tick(20 * 90_000), 1.0)],
        source: CurveSource::Formula { expr: "k + t".into() },
        ..curve
    };
    let p = formula_preview(&with_keys, material, at, at, step);
    assert!((p[0].v - 3.0).abs() < 1e-9, "k + t в предпросмотре: {}", p[0].v);
}
