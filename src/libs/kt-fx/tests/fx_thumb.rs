//! Тест 544 (docs/porting-notes.md, п.428, Р.54): миниатюра эффекта.
//!
//! Миниатюра — то, по чему человек ВЫБИРАЕТ эффект, поэтому у неё две
//! обязанности, и обе проверяются здесь: быть детерминированной (иконка
//! не имеет права мигать от прогона к прогону) и что-то ПОКАЗЫВАТЬ —
//! образец обязан давать каждому виду эффекта материал, на котором тот
//! виден. Эталон образца заморожен хэшем: правка `test_pattern` меняет
//! все иконки программы сразу, и делать это можно только осознанно.

use kt_fx::video::{Frame, apply_video_chain, fit_into, render_fx_thumb, test_pattern};
use kt_fx::video::{FxScratch, VideoFxCtx};
use kt_model::{FxInstance, PanMode};
use xxhash_rust::xxh3::xxh3_64;

/// Опорный размер — размер тумбы источника (kt-app: 320×180): именно на
/// нём сервис считает цепочку, прежде чем ужать результат в ячейку.
const RW: u32 = 320;
const RH: u32 = 180;

fn hash(f: &Frame) -> u64 {
    xxh3_64(&f.rgba)
}

fn thumb(base: &Frame, fx: &[FxInstance]) -> Frame {
    let mut scratch = FxScratch::default();
    render_fx_thumb(base, fx, RW, RH, 0.5, 90_000, None, &mut scratch)
}

/// Голдены образца. Меняются ТОЛЬКО вместе с осознанной правкой
/// `test_pattern` — это эталон, по которому выглядят иконки всех видов.
const GOLDEN_PATTERN: [(&str, u32, u32, bool, u64); 3] = [
    ("ref", RW, RH, false, 0x923630d77778ab3f),
    ("ref-alpha", RW, RH, true, 0x36a58a5ecdcc259a),
    ("icon", 96, 54, false, 0x4621960666a971f2),
];

#[test]
fn t544_thumb_pattern_is_frozen_and_deterministic() {
    for (name, w, h, alpha, want) in GOLDEN_PATTERN {
        let a = test_pattern(w, h, alpha);
        let b = test_pattern(w, h, alpha);
        assert_eq!(hash(&a), hash(&b), "{name}: pattern is not deterministic");
        assert_eq!(a.w, w.max(4));
        assert_eq!(a.h, h.max(4));
        assert_eq!(a.rgba.len(), (a.w as usize) * (a.h as usize) * 4);
        assert_eq!(a.has_alpha, alpha);
        assert_eq!(hash(&a), want, "{name}: pattern hash moved (0x{:016x})", hash(&a));
    }
}

/// Образец обязан давать КАЖДОМУ классу эффектов материал: иначе иконка
/// честно покажет «ничего не изменилось» и соврёт про эффект.
#[test]
fn t544b_pattern_feeds_every_class_of_effect() {
    let p = test_pattern(RW, RH, false);
    let base = hash(&p);

    // асимметрия: зеркало по ЛЮБОЙ оси видно
    for (h, v) in [(true, false), (false, true), (true, true)] {
        let m = thumb(&p, &[FxInstance::Mirror { horizontal: h, vertical: v }]);
        assert_ne!(hash(&m), base, "mirror {h}/{v}: pattern is symmetric");
    }
    // кеинг: зелёная плашка даёт прозрачность
    let g = thumb(&p, &[FxInstance::greenscreen([0, 255, 0], 70, 25)]);
    assert!(g.has_alpha, "greenscreen: no alpha in the result");
    assert!(
        g.rgba.chunks_exact(4).any(|px| px[3] == 0),
        "greenscreen: nothing was keyed out (pattern has no green patch?)"
    );
    // высокие частоты: размытие и контур меняют кадр
    for fx in [FxInstance::blur(8), FxInstance::edges(48), FxInstance::sharpen()] {
        let f = thumb(&p, std::slice::from_ref(&fx));
        assert_ne!(hash(&f), base, "{}: pattern has no detail to work on", fx.tag());
    }
    // цвет: рампа и полосы
    for fx in [FxInstance::posterize(), FxInstance::duotone(), FxInstance::vhs(4, 0.4)] {
        let f = thumb(&p, std::slice::from_ref(&fx));
        assert_ne!(hash(&f), base, "{}: pattern has no colour to work on", fx.tag());
    }
}

/// Тождественная цепочка не трогает байты, а ВАРИАНТ трогает. Это условие
/// иконки: у Pan/Pan 3D/Color Grade/Corner Pin/Drop Shadow шаблон —
/// тождество, и иконкой такому виду служит первый вариант (kt-app,
/// t543). Если бы вариант оказался копией шаблона, иконка показала бы
/// исходный кадр — и тест на стороне kt-app этого бы не увидел, он
/// сравнивает ДАННЫЕ, а не картинку.
#[test]
fn t544c_identity_is_identity_and_a_variant_is_not() {
    let p = test_pattern(RW, RH, false);
    let base = hash(&p);
    assert_eq!(hash(&thumb(&p, &[])), base, "empty chain moved the bytes");

    let identity = FxInstance::Pan { mode: PanMode::Fit, pan_x: 0.0, pan_y: 0.0, zoom: 1.0 };
    assert_eq!(hash(&thumb(&p, &[identity])), base, "Pan defaults are not identity any more");
    let zoomed = FxInstance::Pan { mode: PanMode::Fit, pan_x: 0.0, pan_y: 0.0, zoom: 1.25 };
    assert_ne!(hash(&thumb(&p, &[zoomed])), base, "a zoom variant shows nothing");

    // Color Grade и Corner Pin — те же тождества по шаблону
    assert_eq!(hash(&thumb(&p, &[FxInstance::color_grade()])), base);
    assert_eq!(hash(&thumb(&p, &[FxInstance::corner_pin()])), base);
}

/// Тень существует только там, где есть прозрачность: на непрозрачном
/// кадре `DropShadow` тождествен ПО ПОСТРОЕНИЮ (тень падает из-под
/// альфы). Поэтому у образца два вида, и иконке теневых эффектов нужен
/// именно альфа-вариант.
#[test]
fn t544d_drop_shadow_needs_the_alpha_pattern() {
    let opaque = test_pattern(RW, RH, false);
    let alpha = test_pattern(RW, RH, true);
    let fx = [FxInstance::drop_shadow()];
    assert_eq!(
        hash(&thumb(&opaque, &fx)),
        hash(&opaque),
        "drop shadow on an opaque frame is no longer a passthrough"
    );
    assert_ne!(
        hash(&thumb(&alpha, &fx)),
        hash(&alpha),
        "drop shadow shows nothing even on the alpha pattern"
    );
}

/// Миниатюра — та же цепочка, что у движка: `render_fx_thumb` обязан быть
/// ровно «вписать + применить», без своей арифметики. Иначе картинка в
/// пикере разойдётся с тем, что получится после клика (инвариант №7 по
/// духу: код композиции один, размер — параметр исполнения).
#[test]
fn t544e_thumb_is_fit_plus_chain_and_nothing_else() {
    let p = test_pattern(RW, RH, false);
    let fx = [FxInstance::blur(6), FxInstance::vhs(4, 0.4)];
    let (w, h) = (160, 90);

    let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
    fit_into(&p, w, h, &mut buf);
    let fitted = Frame { w, h, rgba: buf, has_alpha: p.has_alpha };
    let want = apply_video_chain(
        &fx,
        fitted,
        &VideoFxCtx { out_w: w, out_h: h, u: 0.5, burst_index: 90_000, cam3d: None, in_scale: 1 },
    );

    let mut scratch = FxScratch::default();
    let got = render_fx_thumb(&p, &fx, w, h, 0.5, 90_000, None, &mut scratch);
    assert_eq!(hash(&got), hash(&want), "render_fx_thumb is not fit_into + apply_video_chain");

    // тот же пул, второй прогон: «грязный» пул байты не меняет (t68)
    let again = render_fx_thumb(&p, &fx, w, h, 0.5, 90_000, None, &mut scratch);
    assert_eq!(hash(&again), hash(&want), "a reused scratch pool changed the bytes");
}

/// Зерно шума прибито к `burst_index`: при неподвижном курсоре ячейка
/// обязана быть одной и той же картинкой, иначе полоска мерцает на каждом
/// перерисованном кадре интерфейса.
#[test]
fn t544f_noise_is_pinned_to_the_tick() {
    let p = test_pattern(RW, RH, false);
    let fx = [FxInstance::noise(0.25)];
    let mut s = FxScratch::default();
    let a = render_fx_thumb(&p, &fx, RW, RH, 0.5, 90_000, None, &mut s);
    let b = render_fx_thumb(&p, &fx, RW, RH, 0.5, 90_000, None, &mut s);
    let other = render_fx_thumb(&p, &fx, RW, RH, 0.5, 270_000, None, &mut s);
    assert_eq!(hash(&a), hash(&b), "same tick gave two different thumbs");
    assert_ne!(hash(&a), hash(&other), "noise ignores the tick");
}
