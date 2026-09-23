//! Тест 26 (docs/porting-notes.md, п.25): golden-тесты 10 видео-FX.
//! Синтетический кадр детерминирован; хэш фиксирует поведение, свойства
//! ловят смысловые поломки (агенты обязаны гонять при правках kt-fx).

use kt_fx::video::{Frame, VideoFxCtx, apply_video_chain};
use kt_model::{CamKeyframe, FxInstance, PanMode};
use xxhash_rust::xxh3::xxh3_64;

const W: u32 = 64;
const H: u32 = 36;

fn ctx() -> VideoFxCtx {
    VideoFxCtx {
        out_w: W,
        out_h: H,
        u: 0.5,
        burst_index: 90_000,
        cam3d: None,
        in_scale: 1,
        lead: None,
    }
}

/// Детерминированный кадр: градиенты + «зелёный экран» в правом нижнем углу.
fn test_frame() -> Frame {
    let mut rgba = vec![0u8; (W * H * 4) as usize];
    for y in 0..H {
        for x in 0..W {
            let i = ((y * W + x) * 4) as usize;
            if x > W / 2 && y > H / 2 {
                rgba[i] = 0;
                rgba[i + 1] = 255;
                rgba[i + 2] = 0;
            } else {
                rgba[i] = (x * 4) as u8;
                rgba[i + 1] = (y * 7) as u8;
                rgba[i + 2] = ((x + y) * 3) as u8;
            }
            rgba[i + 3] = 255;
        }
    }
    Frame { w: W, h: H, rgba, has_alpha: false }
}

fn run(fx: FxInstance) -> Frame {
    apply_video_chain(&[fx], test_frame(), &ctx())
}

fn mean_rgb(f: &Frame) -> f64 {
    let s: u64 = f.rgba.chunks_exact(4).map(|p| p[0] as u64 + p[1] as u64 + p[2] as u64).sum();
    s as f64 / (f.rgba.len() as f64 * 3.0 / 4.0)
}

/// Golden-хэши: фиксируют текущее поведение всех 10 эффектов ПОИМЁННО
/// (Р.29: одна сумма на всех прятала, какой именно эффект уехал). Менять
/// эталон можно ТОЛЬКО при осознанном изменении алгоритма конкретного
/// эффекта. Последняя санкционированная правка: noise → counter-based hash
/// (Р.29, параллелизуемый RNG для GPU); остальные 9 не менялись с v1.
const GOLDEN: [(&str, u64); 10] = [
    ("pan", 0x40755635c15a2c4a),
    ("cammotion", 0x796fb338df86d795),
    ("mirror", 0x6e6b11df22c1c072),
    ("blur", 0xec8d2f412f21f555),
    ("greenscreen", 0x18dd7db23eb92552),
    ("edges", 0x35521b30cd97497e),
    ("noise", 0x2c1bd320d47c00c6), // Р.29: counter-based hash
    ("vhs", 0xd82252c584915e16),
    ("glow", 0x32647b93a6a84eb2),
    ("eventgrid", 0x0f78a8817bcf1bbe),
];

#[test]
fn t26_golden_hashes_all_effects() {
    let cases: Vec<(&str, FxInstance)> = vec![
        ("pan", FxInstance::Pan { mode: PanMode::Fit, pan_x: 0.3, pan_y: -0.2, zoom: 1.5 }),
        (
            "cammotion",
            FxInstance::CamMotion {
                ease: true,
                mode: PanMode::Fit,
                time_scale: 1.0,
                keyframes: vec![
                    CamKeyframe { t: 0.0, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 },
                    CamKeyframe { t: 1.0, pan_x: 0.5, pan_y: 0.0, zoom: 2.0 },
                ],
            },
        ),
        ("mirror", FxInstance::Mirror { horizontal: true, vertical: false }),
        ("blur", FxInstance::blur(8)),
        ("greenscreen", FxInstance::greenscreen([0, 255, 0], 70, 25)),
        ("edges", FxInstance::edges(48)),
        ("noise", FxInstance::noise(0.3)),
        ("vhs", FxInstance::vhs(4, 0.4)),
        ("glow", FxInstance::glow(0.65)),
        ("eventgrid", FxInstance::EventGrid { cols: 2, rows: 2, center: true }),
    ];
    let mut failures = Vec::new();
    // zip молча обрезал бы лишний кейс — 11-й эффект остался бы без
    // голдена и никто бы не заметил (ловушка Р.47, Кв1)
    assert_eq!(cases.len(), GOLDEN.len(), "cases and GOLDEN must be the same length");
    for ((name, fx), (gname, golden)) in cases.into_iter().zip(GOLDEN) {
        assert_eq!(name, gname, "порядок cases и GOLDEN обязан совпадать");
        let f = run(fx);
        let h = xxh3_64(&f.rgba);
        // при осознанной смене алгоритма: cargo test -- --nocapture, взять
        // новый хэш из вывода и обновить ТОЛЬКО строку этого эффекта
        println!("(\"{name}\", 0x{h:016x}), // {}x{}", f.w, f.h);
        if golden != 0 && h != golden {
            failures.push(format!("{name}: 0x{h:016x} != golden 0x{golden:016x}"));
        }
    }
    assert!(failures.is_empty(), "видео-FX изменили поведение:\n{}", failures.join("\n"));
}

/// Тест 299 (п.306): сетка крупнее кадра не паникует и не смещается.
/// До правки `center: true` при `cols > out_w` уводил `ow - grid_w` в
/// u32-underflow (паника в debug); гард — зеркало kt-gpu `chain_ops`.
#[test]
fn t299_eventgrid_larger_than_frame_is_guarded() {
    for center in [false, true] {
        let f = run(FxInstance::EventGrid { cols: 1000, rows: 1000, center });
        assert_eq!((f.w, f.h), (W, H));
        let again = run(FxInstance::EventGrid { cols: 1000, rows: 1000, center });
        assert_eq!(f.rgba, again.rgba, "детерминизм (center={center})");
    }
    // сетка шире кадра по X (cols→кламп 512 > W): center с гардом не смещает,
    // по Y grid_h == oh даёт offy = 0 — кадры обязаны совпасть байт-в-байт
    let a = run(FxInstance::EventGrid { cols: 1000, rows: 4, center: true });
    let b = run(FxInstance::EventGrid { cols: 1000, rows: 4, center: false });
    assert_eq!(a.rgba, b.rgba, "гард обязан зеркалить GPU: вне кадра — от нуля");
}

#[test]
fn t26_mirror_is_involution() {
    let m = FxInstance::Mirror { horizontal: true, vertical: true };
    let once = apply_video_chain(std::slice::from_ref(&m), test_frame(), &ctx());
    let twice = apply_video_chain(&[m], once, &ctx());
    assert_eq!(twice.rgba, test_frame().rgba, "двойное зеркало = исходник");
}

#[test]
fn t26_blur_preserves_mean_and_reduces_variance() {
    let src = test_frame();
    let out = run(FxInstance::blur(12));
    assert!((mean_rgb(&src) - mean_rgb(&out)).abs() < 8.0, "blur сохраняет среднюю яркость");
    let var = |f: &Frame| {
        let m = mean_rgb(f);
        f.rgba
            .chunks_exact(4)
            .map(|p| {
                let v = (p[0] as f64 + p[1] as f64 + p[2] as f64) / 3.0;
                (v - m) * (v - m)
            })
            .sum::<f64>()
    };
    assert!(var(&out) < var(&src), "blur снижает дисперсию");
}

#[test]
fn t26_greenscreen_keys_out_green_corner() {
    let out = run(FxInstance::greenscreen([0, 255, 0], 70, 25));
    assert!(out.has_alpha);
    // центр зелёного угла прозрачен
    let (x, y) = (W * 3 / 4, H * 3 / 4);
    assert_eq!(out.rgba[((y * W + x) * 4 + 3) as usize], 0, "зелёный угол выключен");
    // не-зелёный угол непрозрачен
    assert_eq!(out.rgba[3], 255, "обычные пиксели не тронуты");
}

#[test]
fn t26_noise_is_deterministic_per_position() {
    let a = run(FxInstance::noise(0.3));
    let b = run(FxInstance::noise(0.3));
    assert_eq!(a.rgba, b.rgba, "одна позиция таймлайна = одно зерно");
    let other = apply_video_chain(
        &[FxInstance::noise(0.3)],
        test_frame(),
        &VideoFxCtx { burst_index: 180_000, ..ctx() },
    );
    assert_ne!(a.rgba, other.rgba, "другая позиция = другое зерно");
}

#[test]
fn t26_eventgrid_tiles_are_identical() {
    let out = run(FxInstance::EventGrid { cols: 2, rows: 2, center: true });
    assert_eq!((out.w, out.h), (W, H));
    // левая верхняя и правая верхняя ячейки идентичны
    let cell_w = (W / 2) as usize;
    let cell_h = (H / 2) as usize;
    for y in 0..cell_h {
        let a0 = (y * W as usize) * 4;
        let b0 = (y * W as usize + cell_w) * 4;
        assert_eq!(
            &out.rgba[a0..a0 + cell_w * 4],
            &out.rgba[b0..b0 + cell_w * 4],
            "строка {y}: плитки мозаики должны совпадать"
        );
    }
}

#[test]
fn t26_pan_stretch_identity_when_no_zoom() {
    // stretch + zoom 1 + pan 0 на кадре размеров out = почти тождество
    let out = run(FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 });
    let src = test_frame();
    let diff: u64 = out
        .rgba
        .iter()
        .zip(src.rgba.iter())
        .map(|(a, b)| (*a as i64 - *b as i64).unsigned_abs())
        .sum();
    let per_px = diff as f64 / (W * H * 4) as f64;
    assert!(per_px < 1.5, "stretch 1:1 почти без искажений, avg diff {per_px:.3}");
}

/// Тест 68 (п.2 р.7): pooled-цепочка с ГРЯЗНЫМ переиспользованным пулом
/// байт-идентична свежему прогону — для каждого эффекта и для цепочки
/// Mirror+EventGrid+Glow+VHS (пул не протекает между кадрами).
#[test]
fn t68_pooled_chain_identical_with_dirty_scratch() {
    use kt_fx::video::{FxScratch, apply_video_chain_pooled};
    let all: Vec<Vec<FxInstance>> = vec![
        vec![FxInstance::Pan { mode: PanMode::Fill, pan_x: 0.3, pan_y: -0.2, zoom: 1.4 }],
        vec![FxInstance::CamMotion {
            ease: true,
            mode: PanMode::Fill,
            time_scale: 1.0,
            keyframes: vec![],
        }],
        vec![FxInstance::Mirror { horizontal: true, vertical: false }],
        vec![FxInstance::Mirror { horizontal: true, vertical: true }],
        vec![FxInstance::blur(9)],
        vec![FxInstance::greenscreen([0, 255, 0], 90, 30)],
        vec![FxInstance::edges(40)],
        vec![FxInstance::noise(0.5)],
        vec![FxInstance::vhs(3, 0.4)],
        vec![FxInstance::glow(1.2)],
        vec![FxInstance::EventGrid { cols: 3, rows: 2, center: true }],
        // цепочка из жалобы пользователя (п.2 р.7)
        vec![
            FxInstance::Mirror { horizontal: true, vertical: false },
            FxInstance::EventGrid { cols: 2, rows: 2, center: false },
            FxInstance::glow(1.0),
            FxInstance::vhs(2, 0.5),
        ],
    ];
    let mut dirty = FxScratch::default();
    // испачкать пул чужими данными разных размеров
    let junk = apply_video_chain_pooled(&[FxInstance::blur(5)], test_frame(), &ctx(), &mut dirty);
    drop(junk);
    for fx in &all {
        let fresh = apply_video_chain(fx, test_frame(), &ctx());
        let pooled = apply_video_chain_pooled(fx, test_frame(), &ctx(), &mut dirty);
        assert_eq!(fresh.rgba, pooled.rgba, "грязный пул изменил результат: {fx:?}");
        assert_eq!((fresh.w, fresh.h, fresh.has_alpha), (pooled.w, pooled.h, pooled.has_alpha));
        // второй прогон тем же пулом — тоже идентичен
        let again = apply_video_chain_pooled(fx, test_frame(), &ctx(), &mut dirty);
        assert_eq!(fresh.rgba, again.rgba);
    }
}

/// Тест 228 (Р.33 С5): новые параметры эффектов. Правило раунда — «дефолт
/// нового параметра = прежнее поведение БАЙТ-В-БАЙТ» (его держат golden'ы
/// t26 выше), поэтому здесь проверяется вторая половина: параметр,
/// сдвинутый с умолчания, обязан РЕАЛЬНО менять картинку и менять её
/// осмысленно.
#[test]
fn t228_new_fx_params_change_the_picture() {
    let hash = |f: Frame| xxh3_64(&f.rgba);
    // ---- Blur: раздельные оси -------------------------------------------
    let base = run(FxInstance::blur(12));
    let plain = hash(run(FxInstance::blur(12)));
    assert_eq!(
        plain,
        hash(run(FxInstance::Blur { radius: 12, radius_y: 0 })),
        "radius_y = 0 обязан быть тождеством прежнему размытию"
    );
    let wide = run(FxInstance::Blur { radius: 12, radius_y: 1 });
    assert_ne!(plain, hash(wide), "радиус по Y обязан менять картинку");
    // размытие только по X: вертикальные соседи различаются СИЛЬНЕЕ, чем
    // при размытии по обеим осям (по Y кадр остался резким)
    // метрика — по ЗЕЛЁНОМУ каналу: в тестовом кадре по вертикали меняется
    // именно он (R = x*4 от y не зависит вовсе, и на нём метрика слепа)
    let var_y = |f: &Frame| {
        let mut s = 0u64;
        for y in 1..f.h as usize {
            for x in 0..f.w as usize {
                let a = f.rgba[(y * f.w as usize + x) * 4 + 1] as i64;
                let b = f.rgba[((y - 1) * f.w as usize + x) * 4 + 1] as i64;
                s += (a - b).unsigned_abs();
            }
        }
        s
    };
    let x_only = run(FxInstance::Blur { radius: 48, radius_y: 1 });
    let both = run(FxInstance::Blur { radius: 48, radius_y: 0 });
    assert!(
        var_y(&x_only) > var_y(&both),
        "смаз по одной оси обязан оставить вертикальный контраст: {} vs {}",
        var_y(&x_only),
        var_y(&both)
    );
    drop(base);

    // ---- Edges: толщина и цвет ------------------------------------------
    let thin = run(FxInstance::edges(48));
    let lit = |f: &Frame| f.rgba.chunks_exact(4).filter(|p| p[0] > 0 || p[1] > 0).count();
    let thick = run(FxInstance::Edges {
        threshold: 48,
        thickness: 3,
        color_r: 255,
        color_g: 255,
        color_b: 255,
    });
    assert!(
        lit(&thick) > lit(&thin),
        "толстая линия обязана зажигать больше пикселей: {} vs {}",
        lit(&thick),
        lit(&thin)
    );
    let red = run(FxInstance::Edges {
        threshold: 48,
        thickness: 1,
        color_r: 255,
        color_g: 0,
        color_b: 0,
    });
    let (mut any_r, mut green) = (false, 0usize);
    for p in red.rgba.chunks_exact(4) {
        any_r |= p[0] > 0;
        green += (p[1] > 0) as usize;
    }
    assert!(any_r && green == 0, "красные линии: зелёного канала быть не должно ({green})");

    // ---- Noise: размер зерна и цвет -------------------------------------
    let fine = run(FxInstance::noise(0.5));
    assert_eq!(
        hash(run(FxInstance::noise(0.5))),
        hash(run(FxInstance::Noise { amount: 0.5, size: 1, color: false })),
        "size = 1, mono — прежняя формула"
    );
    let coarse = run(FxInstance::Noise { amount: 0.5, size: 8, color: false });
    assert_ne!(hash(fine), xxh3_64(&coarse.rgba), "крупное зерно обязано отличаться");
    // крупное зерно: внутри блока 8×8 сдвиг ОДИН, значит соседние пиксели
    // блока отличаются ровно как в исходнике
    let src = test_frame();
    let d_at = |f: &Frame, x: usize, y: usize| {
        let i = (y * f.w as usize + x) * 4;
        f.rgba[i] as i32 - src.rgba[i] as i32
    };
    assert_eq!(d_at(&coarse, 0, 0), d_at(&coarse, 3, 3), "внутри зерна сдвиг один");
    assert_ne!(d_at(&coarse, 0, 0), d_at(&coarse, 9, 9), "у соседнего зерна он свой");
    // цветной шум: у каналов свои сдвиги
    let col = run(FxInstance::Noise { amount: 0.5, size: 1, color: true });
    let mut differs = 0;
    for (p, s) in col.rgba.chunks_exact(4).zip(src.rgba.chunks_exact(4)) {
        let dr = p[0] as i32 - s[0] as i32;
        let dg = p[1] as i32 - s[1] as i32;
        if dr != dg {
            differs += 1;
        }
    }
    assert!(differs > 100, "у цветного шума каналы обязаны расходиться: {differs}");
}

/// Тест 228b (Р.33 С5): вторая пачка параметров — свечение, VHS, хромакей.
#[test]
fn t228_glow_vhs_greenscreen_params() {
    let hash = |f: &Frame| xxh3_64(&f.rgba);
    let mean = |f: &Frame| {
        f.rgba.chunks_exact(4).map(|p| p[0] as u64 + p[1] as u64 + p[2] as u64).sum::<u64>()
    };
    // ---- Glow: радиус, порог, оттенок -----------------------------------
    let plain = run(FxInstance::glow(1.0));
    assert_eq!(
        hash(&plain),
        hash(&run(FxInstance::Glow {
            strength: 1.0,
            radius: 4,
            threshold: 0,
            tint_r: 255,
            tint_g: 255,
            tint_b: 255
        })),
        "радиус 4, порог 0 и белый оттенок — прежнее свечение"
    );
    let wide = run(FxInstance::Glow {
        strength: 1.0,
        radius: 16,
        threshold: 0,
        tint_r: 255,
        tint_g: 255,
        tint_b: 255,
    });
    assert_ne!(hash(&plain), hash(&wide), "радиус свечения обязан менять картинку");
    let gated = run(FxInstance::Glow {
        strength: 1.0,
        radius: 4,
        threshold: 200,
        tint_r: 255,
        tint_g: 255,
        tint_b: 255,
    });
    assert!(
        mean(&gated) < mean(&plain),
        "порог обязан убрать свечение с тёмных мест: {} vs {}",
        mean(&gated),
        mean(&plain)
    );
    let tinted = run(FxInstance::Glow {
        strength: 1.0,
        radius: 4,
        threshold: 0,
        tint_r: 255,
        tint_g: 0,
        tint_b: 0,
    });
    let src = test_frame();
    let (mut dr, mut dg) = (0i64, 0i64);
    for (p, s) in tinted.rgba.chunks_exact(4).zip(src.rgba.chunks_exact(4)) {
        dr += p[0] as i64 - s[0] as i64;
        dg += p[1] as i64 - s[1] as i64;
    }
    assert!(dr > 0 && dg == 0, "красный оттенок светит только в красный: {dr}/{dg}");

    // ---- VHS: шаг скан-линий и дрожание ---------------------------------
    let vplain = run(FxInstance::vhs(4, 0.4));
    assert_eq!(
        hash(&vplain),
        hash(&run(FxInstance::Vhs { rgb_shift: 4, bleed: 0.4, scan_step: 4, jitter: 0.0 })),
        "шаг 4 и нулевое дрожание — прежний VHS"
    );
    // строк со скан-линией: считаем ряды, где зелёный просел
    let dark_rows = |f: &Frame| {
        (0..f.h as usize)
            .filter(|&y| {
                let i = (y * f.w as usize + 5) * 4 + 1;
                let j = (y * src.w as usize + 5) * 4 + 1;
                f.rgba[i] < src.rgba[j]
            })
            .count()
    };
    let sparse = run(FxInstance::Vhs { rgb_shift: 4, bleed: 0.4, scan_step: 12, jitter: 0.0 });
    let none = run(FxInstance::Vhs { rgb_shift: 4, bleed: 0.4, scan_step: 0, jitter: 0.0 });
    assert!(
        dark_rows(&sparse) < dark_rows(&vplain),
        "редкие скан-линии: {} против {}",
        dark_rows(&sparse),
        dark_rows(&vplain)
    );
    assert_eq!(dark_rows(&none), 0, "шаг 0 — скан-линий нет вовсе");
    let jit = run(FxInstance::Vhs { rgb_shift: 0, bleed: 0.0, scan_step: 0, jitter: 0.8 });
    assert_ne!(hash(&jit), hash(&src), "дрожание обязано сдвигать строки");

    // ---- Greenscreen: подавление засветки --------------------------------
    let key = [0u8, 255, 0];
    let keyed = run(FxInstance::greenscreen(key, 70, 25));
    let despilled = run(FxInstance::Greenscreen {
        key_r: 0,
        key_g: 255,
        key_b: 0,
        tolerance: 70,
        softness: 25,
        spill: 1.0,
    });
    // у видимых пикселей зелёный обязан просесть, альфа — не измениться
    let (mut lower, mut alpha_same) = (0usize, true);
    for (a, b) in keyed.rgba.chunks_exact(4).zip(despilled.rgba.chunks_exact(4)) {
        alpha_same &= a[3] == b[3];
        if b[1] < a[1] {
            lower += 1;
        }
    }
    assert!(lower > 50, "подавление обязано убрать зелёную засветку: {lower}");
    assert!(alpha_same, "подавление засветки не трогает альфу");
}

/// Тест 328 (Р.47, п.331): golden-цикл по ВИДЕО-подмножеству реестра
/// FxInstance::all() — хэш дефолт-шаблона каждого эффекта на test_frame.
/// Замок полноты в обе стороны: вид без строки в таблице и строка без
/// вида валят тест (t26 остаётся замороженным на 10 легаси-хэшах). Плюс
/// замок КЛАССИФИКАЦИИ: каждый видео-вид обязан быть гео, поточечным или
/// иметь арм барьера — «неклассифицированный» эффект молча проходит
/// насквозь, и в debug это ловится паникой debug_assert в apply_barrier.
/// (Дефолтные pan и pan3d — оба тождества, их совпадающие хэши легальны.)
#[test]
fn t328_golden_cycle_over_the_registry() {
    use kt_fx::geom::{is_geo, is_pointwise};
    const GOLDEN_ALL: &[(&str, u64)] = &[
        ("pan", 0x05719cf53332c61a),
        // Р.62: узел «Геометрия» с умолчаниями — тождество (identity-хэш)
        ("geometry", 0x05719cf53332c61a),
        ("cam_motion", 0x410e2d08915e02e9),
        ("mirror", 0x6e6b11df22c1c072),
        ("blur", 0xec8d2f412f21f555),
        ("greenscreen", 0x18dd7db23eb92552),
        ("edges", 0x35521b30cd97497e),
        ("noise", 0x2c1bd320d47c00c6),
        ("vhs", 0xd82252c584915e16),
        ("glow", 0x32647b93a6a84eb2),
        ("event_grid", 0x0f78a8817bcf1bbe),
        ("pan3d", 0x05719cf53332c61a),
        // Р.47 Кв2: дефолтные color_grade и corner_pin — тождества,
        // их совпадение с identity-хэшем pan легально и НАМЕРЕННО
        ("color_grade", 0x05719cf53332c61a),
        ("duotone", 0xb24b68c2dc8d352e),
        ("posterize", 0x067921e26e16007c),
        ("vignette", 0xa815992243883318),
        ("corner_pin", 0x05719cf53332c61a),
        // Р.47 Кв3: drop_shadow на непрозрачном тест-кадре — passthrough
        // (identity-хэш) — вход без альфы тень не рисует НАМЕРЕННО
        ("sharpen", 0x542512aaafe117ad),
        ("pixelate", 0xd856791cc8952d79),
        ("chromatic_aberration", 0x4513f7b18223cea3),
        ("radial_blur", 0xcf33730d85dc9c70),
        ("drop_shadow", 0x05719cf53332c61a),
        ("halftone", 0x7a640931274d6e24),
        // Р.47 Кв4: у всех пяти дефолт видим (нейтральных нет)
        ("lens_distort", 0x580537c8e0f3badf),
        ("twirl", 0x0955aa3873066955),
        ("ripple", 0xaf7efc91f739ccf0),
        ("kaleidoscope", 0x9d601aeee14f4062),
        ("cylinder3d", 0x9178d747ce3f9976),
    ];
    let video: Vec<&FxInstance> =
        FxInstance::all().iter().filter(|f| !f.is_audio() && !f.is_layer_mix()).collect();
    let table: std::collections::BTreeMap<&str, u64> = GOLDEN_ALL.iter().copied().collect();
    let tags: std::collections::BTreeSet<&str> = video.iter().map(|f| f.tag()).collect();
    assert_eq!(
        table.keys().copied().collect::<std::collections::BTreeSet<_>>(),
        tags,
        "GOLDEN_ALL and the video registry must cover each other"
    );
    for f in video {
        if !is_geo(f) && !is_pointwise(f) && !matches!(f, FxInstance::SlowedReverb { .. }) {
            let _ = kt_fx::video::apply_barrier(
                f,
                test_frame(),
                &ctx(),
                &mut kt_fx::video::FxScratch::default(),
                1,
            );
        }
        let need_cam = matches!(f, FxInstance::Pan3d { .. });
        let c = if need_cam {
            VideoFxCtx {
                cam3d: Some(kt_fx::geom::Cam3d {
                    cam: kt_model::CameraParams::default(),
                    world_scale: 1.0,
                }),
                ..ctx()
            }
        } else {
            ctx()
        };
        let out = apply_video_chain(std::slice::from_ref(f), test_frame(), &c);
        let hash = xxhash_rust::xxh3::xxh3_64(&out.rgba);
        println!("t328 {}: 0x{hash:016x}", f.tag());
        assert_eq!(
            hash,
            table[f.tag()],
            "{}: default-template bytes moved (update GOLDEN_ALL deliberately)",
            f.tag()
        );
    }
}

// ---- Р.62: узел «Геометрия» ------------------------------------------------

/// Вертикальный кадр 36×64: градиент + непрозрачная альфа.
fn vertical_frame() -> Frame {
    let (w, h) = (36u32, 64u32);
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            rgba[i] = (x * 7) as u8;
            rgba[i + 1] = (y * 4) as u8;
            rgba[i + 2] = 200;
            rgba[i + 3] = 255;
        }
    }
    Frame { w, h, rgba, has_alpha: false }
}

fn lead_ctx(view: (f64, f64), rule: kt_fx::conform::ConformRule) -> VideoFxCtx {
    VideoFxCtx { lead: Some(kt_fx::conform::LeadCtx::new(Some(view), rule)), ..ctx() }
}

/// Тест 713 (Р.62): ведущий узел кладёт вертикальный кадр в 16:9 с
/// ПРОЗРАЧНЫМИ полями; на совпадающих пропорциях узел с умолчаниями —
/// тождество байт-в-байт (прежний путь без единого ресемпла).
#[test]
fn t713_lead_geometry_places_with_transparent_fields() {
    use kt_fx::conform::ConformRule;
    let lead = vec![FxInstance::geometry()];
    // совпадающие пропорции: узел прозрачен, кадр не тронут
    let same = apply_video_chain(&lead, test_frame(), &lead_ctx((64.0, 36.0), ConformRule::Fit));
    assert_eq!(same.rgba, test_frame().rgba, "identity lead must be byte-identical");
    assert!(!same.has_alpha);
    // вертикальный источник, «вписать»: рамка 20.25×36 по центру, поля прозрачны
    let out = apply_video_chain(&lead, vertical_frame(), &lead_ctx((36.0, 64.0), ConformRule::Fit));
    assert_eq!((out.w, out.h), (W, H));
    assert!(out.has_alpha, "fields are transparent, the node produces alpha");
    let alpha = |x: u32, y: u32| out.rgba[((y * W + x) * 4 + 3) as usize];
    for y in 0..H {
        assert_eq!(alpha(0, y), 0, "left field transparent at y={y}");
        assert_eq!(alpha(W - 1, y), 0, "right field transparent at y={y}");
        assert_eq!(alpha(W / 2, y), 255, "centre opaque at y={y}");
    }
    let opaque = (0..W).filter(|&x| alpha(x, H / 2) == 255).count();
    assert!((19..=22).contains(&opaque), "frame width ≈ 20.25 px, got {opaque}");
    // «заполнить»: полей нет, кадр обрезан сверху и снизу
    let fill =
        apply_video_chain(&lead, vertical_frame(), &lead_ctx((36.0, 64.0), ConformRule::Fill));
    assert!(fill.rgba.chunks(4).all(|p| p[3] == 255), "fill has no transparent pixel");
    // «как есть»: 36×64 по центру холста 64×36 — торчит за край сверху и снизу
    let asis =
        apply_video_chain(&lead, vertical_frame(), &lead_ctx((36.0, 64.0), ConformRule::AsIs));
    let a2 = |x: u32, y: u32| asis.rgba[((y * W + x) * 4 + 3) as usize];
    assert_eq!(a2(0, 0), 0);
    assert_eq!(a2(W / 2, 0), 255, "as-is sticks out: column opaque to the edge");
    let opaque = (0..W).filter(|&x| a2(x, H / 2) == 255).count();
    assert!((35..=37).contains(&opaque), "as-is keeps 36 px, got {opaque}");
    // «растянуть»: весь холст, непрозрачно
    let st =
        apply_video_chain(&lead, vertical_frame(), &lead_ctx((36.0, 64.0), ConformRule::Stretch));
    assert!(st.rgba.chunks(4).all(|p| p[3] == 255));
}

/// Тест 717 (Р.62): эффекты ДО ведущего узла работают в пространстве
/// источника, ПОСЛЕ — на холсте; поворот на 90° с зумом заполняет холст.
#[test]
fn t717_before_lead_is_source_space_after_is_canvas() {
    use kt_fx::conform::ConformRule;
    let vig = FxInstance::Vignette { strength: 1.0, radius: 0.5, softness: 0.5 };
    let c = lead_ctx((36.0, 64.0), ConformRule::Fit);
    let before = apply_video_chain(&[vig.clone(), FxInstance::geometry()], vertical_frame(), &c);
    let after = apply_video_chain(&[FxInstance::geometry(), vig.clone()], vertical_frame(), &c);
    assert_eq!((before.w, before.h), (W, H));
    assert_eq!((after.w, after.h), (W, H));
    assert_ne!(before.rgba, after.rgba, "vignette in source space differs from canvas space");
    // виньетка В ИСТОЧНИКЕ темнит его собственные боковые кромки (узкий
    // кадр, край далеко от центра); НА ХОЛСТЕ тот же столбец стоит у
    // центра холста и почти не темнеет — у «after» он светлее
    let px = |f: &Frame, x: u32, y: u32| {
        f.rgba[((y * W + x) * 4) as usize..((y * W + x) * 4 + 3) as usize].to_vec()
    };
    let lum = |p: Vec<u8>| p.iter().map(|&v| v as u32).sum::<u32>();
    let edge = W / 2 - 9; // левая кромка рамки 20.25 px по центру холста
    assert!(
        lum(px(&after, edge, H / 2)) > lum(px(&before, edge, H / 2)),
        "canvas-space vignette leaves the source edge brighter"
    );
    // поворот 90° + зум: вертикальный источник накрывает холст целиком
    let mut g = kt_model::EventGeometry { rot: 90.0, zoom: 1.8, ..Default::default() };
    g.rule = kt_model::Conform::Fit;
    let rot = apply_video_chain(&[g.node()], vertical_frame(), &c);
    assert!(rot.rgba.chunks(4).all(|p| p[3] == 255), "rotated + zoomed source covers the canvas");
    let corner = &rot.rgba[0..3];
    assert_ne!(corner, &[0u8, 0, 0], "corner carries source pixels");
}
