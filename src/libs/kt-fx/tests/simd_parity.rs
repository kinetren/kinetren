//! Тест 198 (Р.32): SIMD-ядра видео-FX БАЙТ-В-БАЙТ равны скалярным.
//!
//! CPU-путь — эталон детерминизма (инвариант №11): golden'ы t26/t11/t67
//! не перегенерировались, значит любое расхождение SIMD со скаляром —
//! регрессия рендера. Тест зовёт ОБА пути явно (env не трогаем: KT_SIMD
//! читается раз на процесс) и сравнивает буферы целиком.
//!
//! Без AVX2 — skip с печатью; `KT_REQUIRE_SIMD=1` превращает пропуск в
//! провал (урок т.194: вакуумные skip'ы прячут сломанный код).

#![cfg(target_arch = "x86_64")]

use kt_fx::simd_testing as st;
use kt_fx::video::{Frame, noise_hash};

/// AVX2 доступен? Иначе skip (или провал при KT_REQUIRE_SIMD=1).
fn avx2() -> bool {
    if std::is_x86_feature_detected!("avx2") {
        return true;
    }
    if std::env::var("KT_REQUIRE_SIMD").as_deref() == Ok("1") {
        panic!("KT_REQUIRE_SIMD=1, а AVX2 на этом CPU нет");
    }
    eprintln!("skipped: у CPU нет AVX2 — SIMD-путь недостижим");
    false
}

/// Детерминированный «злой» кадр: counter-hash-шум, градиенты, крайние
/// 0/255 и шахматка — по одному паттерну на `kind`.
fn frame(w: u32, h: u32, kind: u32) -> Frame {
    let mut rgba = vec![0u8; (w as usize) * (h as usize) * 4];
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let px: [u8; 4] = match kind {
                0 => [(x * 4) as u8, (y * 7) as u8, ((x + y) * 3) as u8, 255],
                1 => {
                    let hsh = noise_hash(x ^ noise_hash(y ^ 0x9e37_79b9));
                    hsh.to_le_bytes()
                }
                2 => [0, 0, 0, 0],
                3 => [255, 255, 255, 255],
                _ => {
                    let v = if (x + y) % 2 == 0 { 0 } else { 255 };
                    [v, v, v, if (x + y) % 3 == 0 { 0 } else { 255 }]
                }
            };
            rgba[i..i + 4].copy_from_slice(&px);
        }
    }
    Frame { w, h, rgba, has_alpha: true }
}

/// Микро-доказательство: эмуляция округления совпадает с `f64::round`
/// на всей «опасной» части домена (точные половины, соседи половин,
/// классический 0.49999999999999994 и hash-рандом).
#[test]
fn t198_round_half_away_matches_scalar() {
    if !avx2() {
        return;
    }
    let mut cases: Vec<f64> = Vec::new();
    // все k*0.5 и их ближайшие соседи по ulp — здесь half-even ошибается
    for k in 0..=511u32 {
        let base = k as f64 * 0.5;
        cases.push(base);
        for n in 1..=4 {
            let mut up = base;
            let mut dn = base;
            for _ in 0..n {
                up = f64::from_bits(up.to_bits() + 1);
                dn = if dn > 0.0 { f64::from_bits(dn.to_bits() - 1) } else { dn };
            }
            cases.push(up);
            cases.push(dn);
        }
    }
    cases.push(0.499_999_999_999_999_94); // fl(x+0.5) == 1.0, а round(x) == 0
    cases.push(0.0);
    cases.push(255.0);
    // hash-рандом в [0, 256)
    for i in 0..1_000_000u32 {
        let h = noise_hash(i);
        cases.push((h as f64 / u32::MAX as f64) * 256.0);
    }
    for chunk in cases.chunks(4) {
        let mut v = [0f64; 4];
        v[..chunk.len()].copy_from_slice(chunk);
        let got = st::round_half_away_avx2(v);
        for (i, x) in v.iter().enumerate() {
            assert_eq!(
                got[i],
                x.round(),
                "round({x:?}) = {} у скаляра, {} у AVX2 (бит {:#x})",
                x.round(),
                got[i],
                x.to_bits()
            );
        }
    }
}

/// Билинейка: злые кадры × злые рамки × нечётные размеры.
#[test]
fn t198_bilinear_scalar_eq_avx2() {
    if !avx2() {
        return;
    }
    // (sw, sh, ow, oh) — хвосты, апскейл, даунскейл, вырожденные
    let sizes = [
        (1u32, 1u32, 1u32, 1u32),
        (3, 3, 7, 5),
        (7, 5, 3, 3),
        (64, 36, 64, 36),
        (64, 36, 128, 72), // апскейл ×2: fx/fy == 0.5 РОВНО (ловит half-even)
        (128, 72, 64, 36),
        (641, 359, 640, 360),
        (37, 23, 641, 359),
    ];
    for kind in 0..5u32 {
        for &(sw, sh, ow, oh) in &sizes {
            let src = frame(sw, sh, kind);
            // рамки: тождество, кроп зумом, выход за границы (клампы),
            // дробные размеры
            let frames = [
                (0.0, 0.0, sw as f64, sh as f64),
                (0.0, 0.0, sw as f64 / 32.0, sh as f64 / 32.0), // zoom 32
                (sw as f64 * 0.25, sh as f64 * 0.25, sw as f64 * 0.5, sh as f64 * 0.5),
                (-3.5, -2.25, sw as f64 + 7.0, sh as f64 + 5.0), // клампы краёв
                (0.3, 0.7, sw as f64 * 0.999, sh as f64 * 1.001),
            ];
            for (fi, &(x0, y0, cw, ch)) in frames.iter().enumerate() {
                let n = (ow as usize) * (oh as usize) * 4;
                let mut a = vec![0u8; n];
                let mut b = vec![0u8; n];
                st::bilinear_scalar(&src, x0, y0, cw, ch, ow, oh, &mut a);
                st::bilinear_avx2(&src, x0, y0, cw, ch, ow, oh, &mut b);
                if a != b {
                    let i = a.iter().zip(&b).position(|(x, y)| x != y).unwrap();
                    panic!(
                        "kind={kind} {sw}x{sh}->{ow}x{oh} рамка#{fi}: байт {i} \
                         (пиксель {}, канал {}) скаляр {} != avx2 {}",
                        i / 4,
                        i % 4,
                        a[i],
                        b[i]
                    );
                }
            }
        }
    }
}

/// Бенч (не в обычном прогоне): `cargo test -p kt-fx --release --test
/// simd_parity -- --ignored --nocapture bench`.
#[test]
#[ignore = "бенч: cargo test --release -- --ignored --nocapture bench"]
fn bench_simd_1080p() {
    if !avx2() {
        return;
    }
    let src = frame(1920, 1080, 1);
    let (ow, oh) = (1920u32, 1080u32);
    let n = (ow as usize) * (oh as usize) * 4;
    let mut out = vec![0u8; n];
    let iters = 20;
    let run = |f: &dyn Fn(&mut Vec<u8>), out: &mut Vec<u8>| {
        let t = std::time::Instant::now();
        for _ in 0..iters {
            f(out);
        }
        t.elapsed().as_secs_f64() * 1000.0 / iters as f64
    };
    // даунскейл 1080p→1080p с зумом (типичный Pan)
    let ms_s = run(
        &|o: &mut Vec<u8>| st::bilinear_scalar(&src, 10.0, 5.0, 1600.0, 900.0, ow, oh, o),
        &mut out,
    );
    let ms_a = run(
        &|o: &mut Vec<u8>| st::bilinear_avx2(&src, 10.0, 5.0, 1600.0, 900.0, ow, oh, o),
        &mut out,
    );
    println!("bilinear 1080p: scalar {ms_s:.2} ms, avx2 {ms_a:.2} ms, ×{:.2}", ms_s / ms_a);

    // noise (целочисленный) и glow-аддитив (f64)
    let pixels = 1920 * 1080;
    let mut nb = src.rgba.clone();
    let t = std::time::Instant::now();
    for _ in 0..iters {
        st::noise_scalar(&mut nb, 1920, 0xdead, 10);
    }
    let ns = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
    let mut nb2 = src.rgba.clone();
    let t = std::time::Instant::now();
    for _ in 0..iters {
        st::noise_avx2(&mut nb2, 1920, 1080, 0xdead, 10);
    }
    let na = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
    println!("noise 1080p: scalar {ns:.2} ms, avx2 {na:.2} ms, ×{:.2}", ns / na);

    let up = src.rgba.clone();
    let mut gb = src.rgba.clone();
    let t = std::time::Instant::now();
    for _ in 0..iters {
        st::glow_add_scalar(&mut gb, &up, 0.35);
    }
    let gs = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
    let mut gb2 = src.rgba.clone();
    let t = std::time::Instant::now();
    for _ in 0..iters {
        st::glow_add_avx2(&mut gb2, &up, 0.35, pixels);
    }
    let ga = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
    println!("glow-add 1080p: scalar {gs:.2} ms, avx2 {ga:.2} ms, ×{:.2}", gs / ga);

    // Сколько вообще стоят ядра, которые ещё НЕ векторизованы (решение
    // «делать или оставить скаляром» — по этим цифрам, Р.32 шаг 5).
    use kt_fx::video::{VideoFxCtx, apply_video_chain};
    use kt_model::FxInstance;
    let ctx = VideoFxCtx { out_w: 1920, out_h: 1080, u: 0.5, burst_index: 90_000, cam3d: None, in_scale: 1 };
    for (name, fx) in [
        (
            "greenscreen",
            FxInstance::greenscreen([0, 255, 0], 70, 25 ),
        ),
        ("edges", FxInstance::edges(48)),
        ("vhs", FxInstance::vhs(4, 0.4)),
        ("mirror", FxInstance::Mirror { horizontal: true, vertical: false }),
    ] {
        let t = std::time::Instant::now();
        for _ in 0..iters {
            let f = Frame {
                w: 1920,
                h: 1080,
                rgba: src.rgba.clone(),
                has_alpha: true,
            };
            std::hint::black_box(apply_video_chain(std::slice::from_ref(&fx), f, &ctx));
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
        // вычитаем стоимость клона кадра (он внутри цикла)
        let t = std::time::Instant::now();
        for _ in 0..iters {
            std::hint::black_box(src.rgba.clone());
        }
        let clone_ms = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
        println!("{name} 1080p (через цепочку, путь по детекту): {:.2} ms", ms - clone_ms);
    }
}

/// noise: целочисленное ядро — обязано совпасть БИТ-В-БИТ (включая
/// деление n*k/32767 с усечением к нулю у отрицательных n).
#[test]
fn t198_noise_scalar_eq_avx2() {
    if !avx2() {
        return;
    }
    for kind in 0..5u32 {
        for (w, h) in [(8u32, 4u32), (9, 3), (64, 36), (641, 7), (1, 1)] {
            for k in [1i64, 7, 16, 32] {
                for seed in [0u32, 1, 0xdead_beef, 0x9e37_79b9] {
                    let base = frame(w, h, kind);
                    let mut a = base.rgba.clone();
                    let mut b = base.rgba.clone();
                    st::noise_scalar(&mut a, w, seed, k);
                    if w >= 8 {
                        st::noise_avx2(&mut b, w, h, seed, k);
                    }
                    // хвост столбцов SIMD не трогает — дорисовываем скаляром
                    let vec_w = if w >= 8 { w & !7 } else { 0 };
                    for y in 0..h {
                        for x in vec_w..w {
                            let i = ((y * w + x) * 4) as usize;
                            let hh = kt_fx::video::noise_hash(
                                x ^ kt_fx::video::noise_hash(y ^ seed),
                            );
                            let n = ((hh >> 16) & 0x7fff) as i64 - 16384;
                            let off = (n * k / 32767) as i32;
                            for c in 0..3 {
                                b[i + c] = (b[i + c] as i32 + off).clamp(0, 255) as u8;
                            }
                        }
                    }
                    assert_eq!(a, b, "noise kind={kind} {w}x{h} k={k} seed={seed:#x}");
                }
            }
        }
    }
}

/// glow-аддитив: f64 mul+add с потолком и УСЕЧЕНИЕМ (не round).
#[test]
fn t198_glow_add_scalar_eq_avx2() {
    if !avx2() {
        return;
    }
    for kind in 0..5u32 {
        for pixels in [1usize, 7, 64, 1000] {
            let up = frame(pixels as u32, 1, (kind + 1) % 5).rgba;
            for s in [0.001f64, 0.35, 0.7, 2.0 * 0.35] {
                let base = frame(pixels as u32, 1, kind).rgba;
                let mut a = base.clone();
                let mut b = base.clone();
                st::glow_add_scalar(&mut a, &up, s);
                st::glow_add_avx2(&mut b, &up, s, pixels);
                assert_eq!(a, b, "glow kind={kind} pixels={pixels} s={s}");
            }
        }
    }
}

/// edges: люма + Собель + порог. Сверяется ПУБЛИЧНАЯ цепочка (идёт
/// SIMD-путём по детекту) со скалярным эталоном.
#[test]
fn t198_edges_scalar_eq_simd() {
    if !avx2() {
        return;
    }
    use kt_fx::video::{VideoFxCtx, apply_video_chain};
    use kt_model::FxInstance;
    for kind in 0..5u32 {
        for (w, h) in [(3u32, 3u32), (9, 5), (11, 4), (64, 36), (65, 37), (129, 9)] {
            for th in [1u32, 48, 255] {
                let src = frame(w, h, kind);
                let want = st::edges_scalar(&src.rgba, w as usize, h as usize, th);
                let ctx = VideoFxCtx { out_w: w, out_h: h, u: 0.5, burst_index: 90_000, cam3d: None, in_scale: 1 };
                let got = apply_video_chain(&[FxInstance::edges(th)], src, &ctx);
                if got.rgba != want {
                    let i = got.rgba.iter().zip(&want).position(|(a, b)| a != b).unwrap();
                    let p = i / 4;
                    panic!(
                        "edges kind={kind} {w}x{h} th={th}: пиксель ({}, {}) канал {}: \
                         скаляр {} != simd {}",
                        p % w as usize,
                        p / w as usize,
                        i % 4,
                        want[i],
                        got.rgba[i]
                    );
                }
            }
        }
    }
}

/// vhs: RGB-сдвиг с лерпом и скан-линии. Векторный интерьер + скалярные
/// кромки обязаны в сумме дать ровно скалярный кадр.
#[test]
fn t198_vhs_scalar_eq_simd() {
    if !avx2() {
        return;
    }
    use kt_fx::video::{VideoFxCtx, apply_video_chain};
    use kt_model::FxInstance;
    for kind in 0..5u32 {
        for (w, h) in [(4u32, 4u32), (13, 5), (33, 4), (64, 36), (65, 9)] {
            for shift in [0u32, 1, 4, 12] {
                for bleed in [0.0f64, 0.4, 1.0] {
                    for burst in [0i64, 90_000, 3000 * 5] {
                        let src = frame(w, h, kind);
                        let mut want = src.rgba.clone();
                        st::vhs_scalar(&mut want, w as usize, h as usize, shift, bleed, burst);
                        let ctx =
                            VideoFxCtx { out_w: w, out_h: h, u: 0.5, burst_index: burst, cam3d: None, in_scale: 1 };
                        let got = apply_video_chain(
                            &[FxInstance::vhs(shift, bleed)],
                            src,
                            &ctx,
                        );
                        if got.rgba != want {
                            let i =
                                got.rgba.iter().zip(&want).position(|(a, b)| a != b).unwrap();
                            let p = i / 4;
                            panic!(
                                "vhs kind={kind} {w}x{h} shift={shift} bleed={bleed} \
                                 burst={burst}: пиксель ({}, {}) канал {}: \
                                 скаляр {} != simd {}",
                                p % w as usize,
                                p / w as usize,
                                i % 4,
                                want[i],
                                got.rgba[i]
                            );
                        }
                    }
                }
            }
        }
    }
}
