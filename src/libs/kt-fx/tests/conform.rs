//! Тест 712 (Р.62, change frame-conform-and-event-geometry): правило
//! приведения кадра — одна функция на размер декода и рамку размещения.
//! Четыре правила × четыре источника (вертикальный, горизонтальный,
//! меньше и больше холста) плюс SAR 2:1; чётность и потолок 4096;
//! «как есть» крупнее холста — за край.

use kt_fx::conform::{ConformRule, conform_dest, conform_geometry};
use kt_fx::plan::MAX_INTER_DIM;

const CANVAS: (u32, u32) = (1920, 1080);

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn t712_four_rules_four_sources() {
    let c = (CANVAS.0 as f64, CANVAS.1 as f64);
    // вертикальный 1080×1920: вписать — поля по бокам, заполнить — обрезка сверху/снизу
    let v = Some((1080.0, 1920.0));
    let d = conform_dest(v, c, ConformRule::Fit);
    assert!(close(d.0, 607.5) && close(d.1, 1080.0), "fit vertical {d:?}");
    let d = conform_dest(v, c, ConformRule::Fill);
    assert!(close(d.0, 1920.0) && close(d.1, 3413.333333333333), "fill vertical {d:?}");
    let d = conform_dest(v, c, ConformRule::Stretch);
    assert!(close(d.0, 1920.0) && close(d.1, 1080.0), "stretch {d:?}");
    let d = conform_dest(v, c, ConformRule::AsIs);
    assert!(close(d.0, 1080.0) && close(d.1, 1920.0), "as-is keeps native {d:?}");
    // горизонтальный 4:3: вписать — поля по бокам (1440×1080)
    let h = Some((640.0, 480.0));
    let d = conform_dest(h, c, ConformRule::Fit);
    assert!(close(d.0, 1440.0) && close(d.1, 1080.0), "fit 4:3 {d:?}");
    let d = conform_dest(h, c, ConformRule::Fill);
    assert!(close(d.0, 1920.0) && close(d.1, 1440.0), "fill 4:3 {d:?}");
    // меньше холста: вписать МАСШТАБИРУЕТ ВВЕРХ, как есть — остаётся мелким
    let small = Some((200.0, 200.0));
    let d = conform_dest(small, c, ConformRule::Fit);
    assert!(close(d.0, 1080.0) && close(d.1, 1080.0), "fit scales up {d:?}");
    let d = conform_dest(small, c, ConformRule::AsIs);
    assert!(close(d.0, 200.0) && close(d.1, 200.0), "as-is small {d:?}");
    // больше холста: как есть торчит за край
    let big = Some((4000.0, 3000.0));
    let d = conform_dest(big, c, ConformRule::AsIs);
    assert!(d.0 > c.0 && d.1 > c.1, "as-is big sticks out {d:?}");
    // неизвестный вид — растянуть, детерминированно
    for r in [ConformRule::Fit, ConformRule::Fill, ConformRule::AsIs, ConformRule::Stretch] {
        assert_eq!(conform_dest(None, c, r), (c.0, c.1), "{r:?}");
    }
}

#[test]
fn t712b_sar_is_part_of_the_view() {
    // два файла 640×480, SAR 1:1 и 2:1: виды 4:3 и 8:3, рамки разные
    let c = (1920.0, 1080.0);
    let a = conform_dest(Some((640.0, 480.0)), c, ConformRule::Fit);
    let b = conform_dest(Some((1280.0, 480.0)), c, ConformRule::Fit);
    assert!(close(a.0, 1440.0) && close(b.0, 1920.0) && close(b.1, 720.0), "{a:?} {b:?}");
}

#[test]
fn t712c_decode_size_even_capped_and_crop_aware() {
    // вертикальный, вписать: декод = рамка (чётный)
    let g = conform_geometry(Some((1080.0, 1920.0)), CANVAS, ConformRule::Fit, [0.0; 4]);
    assert_eq!(g.decode, (608, 1080), "{:?}", g.decode);
    assert!(g.decode.0.is_multiple_of(2) && g.decode.1.is_multiple_of(2));
    // заполнить 9:16 в 4K — рамка выше потолка, декод режется пропорционально
    let g = conform_geometry(Some((1080.0, 1920.0)), (3840, 2160), ConformRule::Fill, [0.0; 4]);
    assert!(g.dest.1 > MAX_INTER_DIM as f64, "dest above cap {:?}", g.dest);
    assert!(g.decode.0 <= MAX_INTER_DIM && g.decode.1 <= MAX_INTER_DIM, "{:?}", g.decode);
    let ratio = g.decode.0 as f64 / g.decode.1 as f64;
    assert!((ratio - 1080.0 / 1920.0).abs() < 0.01, "cap keeps proportions {ratio}");
    // кроп половины ширины: рамка считается от кропнутого вида, декод —
    // рамка, растянутая обратно на весь кадр
    let g =
        conform_geometry(Some((1920.0, 1080.0)), CANVAS, ConformRule::Fit, [0.25, 0.0, 0.25, 0.0]);
    assert!(close(g.dest.0, 960.0) && close(g.dest.1, 1080.0), "cropped fit {:?}", g.dest);
    assert_eq!(g.decode, (1920, 1080), "decode is the full frame {:?}", g.decode);
    // как есть 4000×3000 — натив, с потолком
    let g = conform_geometry(Some((4000.0, 3000.0)), CANVAS, ConformRule::AsIs, [0.0; 4]);
    assert_eq!(g.decode, (4000, 3000));
    let g = conform_geometry(Some((8000.0, 6000.0)), CANVAS, ConformRule::AsIs, [0.0; 4]);
    assert_eq!(g.decode, (4096, 3072));
    // минимум 2
    let g = conform_geometry(Some((1.0, 1.0)), CANVAS, ConformRule::AsIs, [0.0; 4]);
    assert_eq!(g.decode, (2, 2));
}
