//! Тест 198 (Р.32), часть композита: AVX2-ядра alpha_over/dissolve/
//! scale_alpha БАЙТ-В-БАЙТ равны скалярным. Композит целочисленный,
//! поэтому проверяются ровно две вещи: точность деления на 255 (весь
//! домен) и повторение ВЕТВЛЕНИЙ скаляра (sa == 0 оставляет dst
//! нетронутым, sa == 255 — копия).
//!
//! Без AVX2 — skip; `KT_REQUIRE_SIMD=1` превращает пропуск в провал.

#![cfg(target_arch = "x86_64")]

use kt_engine::simd_testing as st;
use kt_fx::video::noise_hash;

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

/// Исчерпывающе: `x / 255` для ВСЕГО домена, который может встретиться
/// (max 255·255·255 = 16 581 375 < 2^24).
#[test]
fn t198_div255_exhaustive() {
    if !avx2() {
        return;
    }
    let mut buf = [0u32; 8];
    let mut x = 0u32;
    while x < (1 << 24) {
        for (i, slot) in buf.iter_mut().enumerate() {
            *slot = x + i as u32;
        }
        let got = st::div255_avx2(buf);
        for i in 0..8 {
            let want = buf[i] / 255;
            assert_eq!(got[i], want, "div255({}) = {want}, а SIMD дал {}", buf[i], got[i]);
        }
        x += 8;
    }
}

/// Кадр: `kind` задаёт паттерн RGB, `alphas` — циклический набор альф
/// (в т.ч. крайние 0 и 255 — они выбирают ветки alpha_over).
fn frame(pixels: usize, kind: u32, alphas: &[u8]) -> Vec<u8> {
    let mut v = vec![0u8; pixels * 4];
    for p in 0..pixels {
        let i = p * 4;
        let rgb: [u8; 3] = match kind {
            0 => [(p * 7) as u8, (p * 13) as u8, (p * 29) as u8],
            1 => {
                let h = noise_hash(p as u32 ^ 0x1234_5678);
                [h as u8, (h >> 8) as u8, (h >> 16) as u8]
            }
            2 => [0, 0, 0],
            _ => [255, 255, 255],
        };
        v[i..i + 3].copy_from_slice(&rgb);
        v[i + 3] = alphas[p % alphas.len()];
    }
    v
}

#[test]
fn t198_alpha_over_scalar_eq_avx2() {
    if !avx2() {
        return;
    }
    // ПОЛНЫЙ перебор (sa, da): 256×256 пикселей — обе ветки и все
    // промежуточные значения, включая sa=255 (копия) и sa=0 (не трогать)
    let pixels = 256 * 256;
    let mut src = vec![0u8; pixels * 4];
    let mut dst0 = vec![0u8; pixels * 4];
    for sa in 0..256usize {
        for da in 0..256usize {
            let i = (sa * 256 + da) * 4;
            let h = noise_hash((sa * 256 + da) as u32);
            src[i] = h as u8;
            src[i + 1] = (h >> 8) as u8;
            src[i + 2] = (h >> 16) as u8;
            src[i + 3] = sa as u8;
            let g = noise_hash((sa * 256 + da) as u32 ^ 0xdead);
            dst0[i] = g as u8;
            dst0[i + 1] = (g >> 8) as u8;
            dst0[i + 2] = (g >> 16) as u8;
            dst0[i + 3] = da as u8;
        }
    }
    let mut a = dst0.clone();
    let mut b = dst0.clone();
    st::alpha_over_scalar(&mut a, &src, pixels);
    st::alpha_over_avx2(&mut b, &src, pixels);
    if a != b {
        let i = a.iter().zip(&b).position(|(x, y)| x != y).unwrap();
        let p = i / 4;
        panic!(
            "alpha_over: пиксель {p} (sa={}, da={}), канал {}: скаляр {} != avx2 {}",
            src[p * 4 + 3],
            dst0[p * 4 + 3],
            i % 4,
            a[i],
            b[i]
        );
    }

    // разные паттерны и НЕКРАТНЫЕ 8 длины (хвост)
    for kind in 0..4u32 {
        for pixels in [1usize, 7, 8, 9, 33, 1000] {
            let src = frame(pixels, kind, &[0, 1, 127, 128, 254, 255]);
            let base = frame(pixels, (kind + 1) % 4, &[255, 0, 200, 3, 128]);
            let mut a = base.clone();
            let mut b = base.clone();
            st::alpha_over_scalar(&mut a, &src, pixels);
            st::alpha_over_avx2(&mut b, &src, pixels & !7);
            // хвост у SIMD-пути дорисовывает скаляр — эмулируем это
            for p in (pixels & !7)..pixels {
                let one = p * 4;
                let mut tmp = b[one..one + 4].to_vec();
                st::alpha_over_scalar(&mut tmp, &src[one..one + 4], 1);
                b[one..one + 4].copy_from_slice(&tmp);
            }
            assert_eq!(a, b, "alpha_over kind={kind} pixels={pixels}");
        }
    }
}

#[test]
fn t198_dissolve_and_scale_alpha_scalar_eq_avx2() {
    if !avx2() {
        return;
    }
    for kind in 0..4u32 {
        for pixels in [1usize, 3, 4, 5, 17, 1000] {
            let a_buf = frame(pixels, kind, &[0, 17, 128, 255]);
            let b_buf = frame(pixels, (kind + 2) % 4, &[255, 200, 1, 0]);
            for &k in &[0u32, 1, 2048, 4095, 4096] {
                for &(ah, bh) in &[(true, true), (true, false), (false, true), (false, false)] {
                    let mut sa_ = vec![0u8; pixels * 4];
                    let mut sb = vec![0u8; pixels * 4];
                    st::dissolve_scalar(&mut sa_, &a_buf, &b_buf, ah, bh, k, pixels);
                    st::dissolve_avx2(&mut sb, &a_buf, &b_buf, ah, bh, k, pixels);
                    // хвост (pixels % 4) SIMD не трогает — дорисовываем
                    for p in (pixels & !3)..pixels {
                        let i = p * 4;
                        let mut tmp = vec![0u8; 4];
                        st::dissolve_scalar(
                            &mut tmp,
                            &a_buf[i..i + 4],
                            &b_buf[i..i + 4],
                            ah,
                            bh,
                            k,
                            1,
                        );
                        sb[i..i + 4].copy_from_slice(&tmp);
                    }
                    assert_eq!(sa_, sb, "dissolve kind={kind} pixels={pixels} k={k} {ah}/{bh}");
                }
            }
            // scale_alpha
            for &k in &[0u32, 1, 2048, 4095, 4096] {
                for &has in &[true, false] {
                    let mut sa_ = vec![0u8; pixels * 4];
                    let mut sb = vec![0u8; pixels * 4];
                    st::scale_alpha_scalar(&mut sa_, &a_buf, has, k, pixels);
                    st::scale_alpha_avx2(&mut sb, &a_buf, has, k, pixels);
                    for p in (pixels & !7)..pixels {
                        let i = p * 4;
                        let mut tmp = vec![0u8; 4];
                        st::scale_alpha_scalar(&mut tmp, &a_buf[i..i + 4], has, k, 1);
                        sb[i..i + 4].copy_from_slice(&tmp);
                    }
                    assert_eq!(sa_, sb, "scale_alpha kind={kind} pixels={pixels} k={k} has={has}");
                }
            }
        }
    }
}

/// Бенч композита: `cargo test -p kt-engine --release --test simd_parity
/// -- --ignored --nocapture bench`.
#[test]
#[ignore = "бенч: cargo test --release -- --ignored --nocapture bench"]
fn bench_composite_1080p() {
    if !avx2() {
        return;
    }
    let pixels = 1920 * 1080;
    let src = frame(pixels, 1, &[0, 64, 128, 200, 255]);
    let base = frame(pixels, 0, &[255, 128, 7]);
    let iters = 20;
    let bench = |name: &str, mut f: Box<dyn FnMut()>| {
        let t = std::time::Instant::now();
        for _ in 0..iters {
            f();
        }
        let ms = t.elapsed().as_secs_f64() * 1000.0 / iters as f64;
        println!("  {name}: {ms:.2} ms");
        ms
    };
    let mut d1 = base.clone();
    let s1 = src.clone();
    let ms_s = bench(
        "alpha_over scalar",
        Box::new(move || st::alpha_over_scalar(&mut d1, &s1, pixels)),
    );
    let mut d2 = base.clone();
    let s2 = src.clone();
    let ms_a = bench(
        "alpha_over avx2  ",
        Box::new(move || st::alpha_over_avx2(&mut d2, &s2, pixels)),
    );
    println!("alpha_over 1080p ×{:.2}", ms_s / ms_a);

    let mut o1 = vec![0u8; pixels * 4];
    let (a1, b1) = (src.clone(), base.clone());
    let ds = bench(
        "dissolve scalar",
        Box::new(move || st::dissolve_scalar(&mut o1, &a1, &b1, true, true, 2048, pixels)),
    );
    let mut o2 = vec![0u8; pixels * 4];
    let (a2, b2) = (src.clone(), base.clone());
    let da = bench(
        "dissolve avx2  ",
        Box::new(move || st::dissolve_avx2(&mut o2, &a2, &b2, true, true, 2048, pixels)),
    );
    println!("dissolve 1080p ×{:.2}", ds / da);
}
