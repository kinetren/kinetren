//! Тест 550 (docs/porting-notes.md, п.433, Р.54): смешивание двух
//! состояний одного вида — операция под ОСЬ СИЛЫ в пикере.
//!
//! Главное свойство, ради которого она вообще есть: концы отдаются
//! БАЙТ-В-БАЙТ. На этом стоит правило «клик по ячейке даёт именно её
//! вариант»: если бы `t == 1` возвращал пересобранное значение, клик по
//! «Плотно» после проводки мог бы дать что-то на пол-единицы другое, и
//! никакой тест вёрстки этого бы не увидел.

use kt_model::{FxInstance, PanMode};

fn dbg(f: &FxInstance) -> String {
    format!("{f:?}")
}

#[test]
fn t550_lerp_keeps_the_ends_and_moves_the_numbers() {
    let a = FxInstance::blur(4);
    let b = FxInstance::blur(24);

    // концы — байт-в-байт, без пересборки
    assert_eq!(dbg(&a.lerp(&b, 0.0)), dbg(&a));
    assert_eq!(dbg(&a.lerp(&b, 1.0)), dbg(&b));
    // и за границами 0..1 тоже (значение зажимается)
    assert_eq!(dbg(&a.lerp(&b, -5.0)), dbg(&a));
    assert_eq!(dbg(&a.lerp(&b, 17.0)), dbg(&b));

    // середина — посередине; целое поле округляется, а не усекается
    assert_eq!(a.lerp(&b, 0.5).get_param("radius"), Some(14.0));
    assert_eq!(a.lerp(&b, 0.25).get_param("radius"), Some(9.0));
    // ход монотонный
    let mut prev = 0.0;
    for i in 0..=10 {
        let v = a.lerp(&b, i as f64 / 10.0).get_param("radius").unwrap();
        assert!(v >= prev, "ход не монотонен: {v} после {prev}");
        prev = v;
    }
}

/// Нечисловое интерполировать нельзя — оно берётся от БЛИЖАЙШЕГО конца и
/// переключается ровно в середине. Иначе «режим кадрирования» или «галка
/// вращения» меняли бы значение в случайной точке проводки.
#[test]
fn t550b_non_numeric_comes_from_the_nearer_end() {
    let fit = FxInstance::Pan { mode: PanMode::Fit, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 };
    let fill = FxInstance::Pan { mode: PanMode::Fill, pan_x: 0.0, pan_y: 0.0, zoom: 3.0 };
    for (t, want) in [(0.1, PanMode::Fit), (0.49, PanMode::Fit), (0.5, PanMode::Fill)] {
        match fit.lerp(&fill, t) {
            FxInstance::Pan { mode, .. } => assert_eq!(mode, want, "t = {t}"),
            other => panic!("вид подменился: {other:?}"),
        }
    }
    // а число при этом едет непрерывно через середину
    let mid = fit.lerp(&fill, 0.5).get_param("zoom").unwrap();
    assert!((mid - 2.0).abs() < 1e-9, "zoom в середине: {mid}");

    // у зеркала числовых параметров нет вовсе: остаётся ступенька, и это
    // единственное осмысленное поведение
    let h = FxInstance::Mirror { horizontal: true, vertical: false };
    let v = FxInstance::Mirror { horizontal: false, vertical: true };
    assert_eq!(dbg(&h.lerp(&v, 0.3)), dbg(&h));
    assert_eq!(dbg(&h.lerp(&v, 0.7)), dbg(&v));
}

/// Разные виды не смешиваются: получится ближайший конец как есть, а не
/// химера с чужими полями.
#[test]
fn t550c_different_kinds_do_not_blend() {
    let blur = FxInstance::blur(8);
    let glow = FxInstance::glow(0.9);
    assert_eq!(dbg(&blur.lerp(&glow, 0.2)), dbg(&blur));
    assert_eq!(dbg(&blur.lerp(&glow, 0.8)), dbg(&glow));
}

/// Смесь остаётся В ГРАНИЦАХ СПЕК по всем видам реестра: `set_param`
/// клампит, но проверка нужна отдельная — она же ловит параметр, у
/// которого `get_param` молчит (тогда поле осталось бы от ближайшего
/// конца, и ход по оси был бы рваным).
#[test]
fn t550d_blend_stays_in_spec_for_every_kind() {
    for f in FxInstance::all() {
        let specs = f.param_specs();
        if specs.is_empty() {
            continue;
        }
        // Второй конец — 99 % диапазона, а НЕ ровный максимум: у f32-поля
        // ровный потолок после `set_param` читается обратно чуть выше
        // потолка (кламп считается в f64, запись в f32 округляет вверх —
        // п.427). Это свойство модели, а не смешивания, и подсовывать его
        // сюда значило бы проверять не то.
        let mut hi = f.clone();
        for s in specs {
            hi.set_param(s.name, s.min + (s.max - s.min) * 0.99);
        }
        for i in 0..=8 {
            let t = i as f64 / 8.0;
            let mid = f.lerp(&hi, t);
            assert_eq!(
                std::mem::discriminant(&mid),
                std::mem::discriminant(f),
                "{}: вид подменился на t = {t}",
                f.tag()
            );
            for s in specs {
                let v = mid.get_param(s.name).unwrap_or(f64::NAN);
                assert!(
                    v.is_finite() && v >= s.min && v <= s.max,
                    "{} / {}: {v} вне [{}, {}] на t = {t}",
                    f.tag(),
                    s.name,
                    s.min,
                    s.max
                );
            }
        }
    }
}
