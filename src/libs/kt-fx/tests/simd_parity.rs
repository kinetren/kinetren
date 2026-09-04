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
                    let v = if (x / 1 + y / 1) % 2 == 0 { 0 } else { 255 };
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
}
