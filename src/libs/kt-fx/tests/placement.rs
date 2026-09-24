//! Тест 761 (Р.64): размещение ведущего узла «Геометрия» и обратные
//! пересчёты экранного жеста.
//!
//! Владелец (Р.63 п.6): «КАК МИНИМУМ у нас должно быть редактирование прям
//! в превью удобное». Экранная рамка обязана считаться ТОЙ ЖЕ функцией, что
//! строит гео-программу рендера, а жест — возвращать значения, дающие ровно
//! запрошенное смещение. Отдельная проверка у ЯКОРЯ: он двигается, картинка
//! стоит — и на повёрнутом слое тоже, где у After Effects это сломано
//! (цитаты в `docs/features/ui/viewer/on-screen-controls.md`).

use kt_fx::geom::{
    Handle, Placement, anchor_move, crop_for_handle, geometry_placement, pos_for_drag,
    rot_for_point, zoom_for_handle,
};
use kt_fx::conform::{ConformRule, LeadCtx};
use kt_model::{EventGeometry, FxInstance};

const CANVAS: (f64, f64) = (1920.0, 1080.0);
/// Вид источника — вертикальный: у «вписать» появляются поля, и рамка
/// перестаёт совпадать с холстом (иначе тест зелен ни на чём).
const VIEW: (f64, f64) = (1080.0, 1920.0);

fn node(g: EventGeometry) -> FxInstance {
    g.node()
}

fn lead() -> Option<LeadCtx> {
    Some(LeadCtx::new(Some(VIEW), ConformRule::Fit))
}

fn place(g: &EventGeometry) -> Placement {
    geometry_placement(&node(*g), lead(), VIEW, CANVAS).expect("узел геометрии")
}

fn near(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} против {b} (допуск {tol})");
}

fn corners_near(a: [(f64, f64); 4], b: [(f64, f64); 4], tol: f64, what: &str) {
    for i in 0..4 {
        assert!(
            (a[i].0 - b[i].0).abs() <= tol && (a[i].1 - b[i].1).abs() <= tol,
            "{what}: угол {i} уехал — {:?} против {:?}",
            a[i],
            b[i]
        );
    }
}

/// Тест 761 (Р.64): размещение и обратные пересчёты.
#[test]
fn t761_placement_and_inverse_gestures() {
    // --- РАМКА равна аналитической ------------------------------------
    let g0 = EventGeometry::default();
    let p0 = place(&g0);
    // «вписать» вертикального вида в 16:9 — рамка по высоте, поля по бокам
    near(p0.dh, 1080.0, 1e-9, "высота рамки «вписать»");
    near(p0.dw, 1080.0 * VIEW.0 / VIEW.1, 1e-9, "ширина рамки «вписать»");
    near(p0.ox + p0.dw / 2.0, CANVAS.0 / 2.0, 1e-9, "рамка по центру холста");
    assert_eq!(p0.anchor, (p0.ox + p0.dw / 2.0, p0.oy + p0.dh / 2.0), "якорь по умолчанию — центр");

    // --- ПЕРЕНОС: сдвиг в px холста даёт ровно себя --------------------
    let want = (137.0, -42.0);
    let pos = pos_for_drag((0.0, 0.0), CANVAS, want);
    let moved = place(&EventGeometry { pos_x: pos.0, pos_y: pos.1, ..g0 });
    near(moved.ox - p0.ox, want.0, 1e-9, "перенос по X");
    near(moved.oy - p0.oy, want.1, 1e-9, "перенос по Y");

    // --- МАСШТАБ: противоположный угол остался на месте ----------------
    for h in [Handle::TopLeft, Handle::BottomRight, Handle::Right, Handle::Top] {
        let from = h.point(&p0);
        let fixed_before = {
            // та же точка, что вернёт `opposite` — берём через жест ниже
            let cs = p0.corners();
            match h {
                Handle::TopLeft => cs[2],
                Handle::BottomRight => cs[0],
                Handle::Right => ((cs[3].0 + cs[0].0) / 2.0, (cs[3].1 + cs[0].1) / 2.0),
                Handle::Top => ((cs[2].0 + cs[3].0) / 2.0, (cs[2].1 + cs[3].1) / 2.0),
                _ => unreachable!(),
            }
        };
        // тянем ручку в полтора раза дальше от неподвижной точки
        let target = (
            fixed_before.0 + (from.0 - fixed_before.0) * 1.5,
            fixed_before.1 + (from.1 - fixed_before.1) * 1.5,
        );
        let (z, pos) = zoom_for_handle(&p0, h, target, g0.zoom, (g0.pos_x, g0.pos_y), CANVAS)
            .expect("ручка масштаба");
        near(z, 1.5, 1e-9, "зум после тяги на 1.5");
        let after = place(&EventGeometry { zoom: z, pos_x: pos.0, pos_y: pos.1, ..g0 });
        let fixed_after = {
            let cs = after.corners();
            match h {
                Handle::TopLeft => cs[2],
                Handle::BottomRight => cs[0],
                Handle::Right => ((cs[3].0 + cs[0].0) / 2.0, (cs[3].1 + cs[0].1) / 2.0),
                Handle::Top => ((cs[2].0 + cs[3].0) / 2.0, (cs[2].1 + cs[3].1) / 2.0),
                _ => unreachable!(),
            }
        };
        near(fixed_after.0, fixed_before.0, 1e-6, "неподвижная точка уехала по X");
        near(fixed_after.1, fixed_before.1, 1e-6, "неподвижная точка уехала по Y");
        // и сама ручка пришла туда, куда тянули
        let now = h.point(&after);
        near(now.0, target.0, 1e-6, "ручка не дошла по X");
        near(now.1, target.1, 1e-6, "ручка не дошла по Y");
    }

    // --- ПОВОРОТ: угол от якоря, шаг по Shift --------------------------
    let a = p0.anchor;
    // точка строго справа от якоря — это поворот на +90° (ноль «вверх»)
    let deg = rot_for_point(&p0, (a.0 + 100.0, a.1), None);
    near(deg, 90.0, 1e-9, "угол по точке справа");
    let snapped = rot_for_point(&p0, (a.0 + 100.0, a.1 + 7.0), Some(15.0));
    near(snapped, 90.0, 1e-9, "шаг 15° округлил 94° к 90°");

    // --- ЯКОРЬ: значения меняются, КАРТИНКА СТОИТ ---------------------
    for rot in [0.0, 30.0, -137.0] {
        let g = EventGeometry { rot, ..g0 };
        let pl = place(&g);
        let before = pl.corners();
        // тянем якорь в левый верх рамки (в её собственной системе)
        let target = {
            let cs = before;
            cs[0]
        };
        let (na, npos) = anchor_move(&pl, target, (g.pos_x, g.pos_y), CANVAS);
        assert!(
            (na.0 - g.anchor_x).abs() > 0.1 || (na.1 - g.anchor_y).abs() > 0.1,
            "якорь не изменился — жест ничего не сделал: {na:?}"
        );
        let after = place(&EventGeometry {
            anchor_x: na.0,
            anchor_y: na.1,
            pos_x: npos.0,
            pos_y: npos.1,
            ..g
        });
        corners_near(after.corners(), before, 1e-6, &format!("поворот {rot}°: картинка уехала"));
        // …и якорь ТЕПЕРЬ в той точке, куда его тянули
        near(after.anchor.0, target.0, 1e-6, "якорь не там, куда тянули (X)");
        near(after.anchor.1, target.1, 1e-6, "якорь не там, куда тянули (Y)");
    }

    // --- КРОП: ручка режет свою сторону, рамка следует правилу ---------
    let pl = place(&g0);
    let left = Handle::CropLeft.point(&pl);
    let target = (left.0 + pl.dw * 0.25, left.1);
    let crop = crop_for_handle(&pl, Handle::CropLeft, target, [0.0; 4]).expect("кроп слева");
    near(crop[0], 0.25, 1e-9, "кроп слева");
    assert_eq!(&crop[1..], &[0.0, 0.0, 0.0], "кроп тронул чужие стороны");
    let after = place(&EventGeometry { crop_l: crop[0], ..g0 });
    assert!(after.dw < pl.dw, "рамка не сузилась после кропа: {} против {}", after.dw, pl.dw);
    // взаимный предел: съесть вид целиком нельзя
    assert!(
        crop_for_handle(&pl, Handle::CropLeft, (left.0 + pl.dw * 0.99, left.1), [0.0, 0.0, 0.9, 0.0])
            .is_none(),
        "кроп съел вид целиком — предел не сработал"
    );
}
