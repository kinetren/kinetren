//! Тест 138 (п.138 porting-notes, Р.24): «ручки» перехода у FX-клипа.
//!
//! ClipProcessor получает material-домен [−head_ext, clip_len + tail_ext):
//! dry-гейт режет за концом МАТЕРИАЛА (не тела), отрицательный local — левая
//! ручка, pre-roll может заходить в неё; нормировка автоматизации остаётся
//! по clip_len (материал ручек её НЕ сдвигает). chain_source_window_ext для
//! material = 0..len байт-совместим со старым chain_source_window.

use kt_fx::{
    ClipProcessor, chain_source_window, chain_source_window_ext, output_time_for_source,
    source_progress,
};
use kt_model::FxInstance;

const CH: usize = 2;

/// Источник-«линейка»: значение кадра = его АБСОЛЮТНЫЙ номер (определён и
/// для отрицательных — материал левее source_in существует).
fn ramp_pull(_kind: kt_fx::PullKind, src: i64, buf: &mut [f32]) -> bool {
    for (i, frame) in buf.chunks_mut(CH).enumerate() {
        let v = (src + i as i64) as f32 * 1e-6;
        frame.fill(v);
    }
    true
}

fn read_span(p: &mut ClipProcessor, from: i64, to: i64) -> Vec<f32> {
    let mut out = vec![0f32; ((to - from) as usize) * CH];
    let mut done = 0i64;
    while done < to - from {
        let n = (to - from - done).min(512) as usize;
        let s = (done as usize) * CH;
        p.read(from + done, &mut out[s..s + n * CH], &mut ramp_pull);
        done += n as i64;
    }
    out
}


#[test]
fn t138_plain_chain_reads_handles_and_gates_at_material_end() {
    let fx = [FxInstance::Gain { db: 0.0 }];
    let (src_in, len, head, tail) = (10_000i64, 4_096i64, 1_024i64, 2_048i64);
    let mut p = ClipProcessor::new(&fx, &Default::default(), src_in, len);
    p.set_material_ext(head, tail);

    let out = read_span(&mut p, -head, len + tail + 512);
    for i in 0..(head + len + tail + 512) {
        let local = -head + i;
        let got = out[(i as usize) * CH];
        let expect =
            if local < len + tail { (src_in + local) as f32 * 1e-6 } else { 0.0 };
        assert_eq!(got, expect, "local {local}: handles read source, gate at material end");
    }
}

#[test]
fn t138_default_material_keeps_old_gate() {
    // без set_material_ext поведение прежнее: dry-гейт ровно на clip_len
    let fx = [FxInstance::Gain { db: 0.0 }];
    let (src_in, len) = (5_000i64, 2_000i64);
    let mut p = ClipProcessor::new(&fx, &Default::default(), src_in, len);
    let out = read_span(&mut p, 0, len + 512);
    assert_eq!(out[((len - 1) as usize) * CH], (src_in + len - 1) as f32 * 1e-6);
    assert_eq!(out[(len as usize) * CH], 0.0, "beyond clip_len dry must be zero");
}

#[test]
fn t138_source_map_extrapolates_left_linearly() {
    // без автоматизации: S(t) = t·rate и слева
    assert_eq!(source_progress(0.5, &[], -0.25), -0.125);
    assert_eq!(output_time_for_source(0.5, &[], -0.125), -0.25);
    // с автоматизацией: слева — скорость ПЕРВОГО сегмента (за краями крайние)
    let kfs = [
        (0.0, 0.5),
        (1.0, 2.0),
    ];
    assert_eq!(source_progress(1.0, &kfs, -0.1), -0.05);
    assert_eq!(output_time_for_source(1.0, &kfs, -0.05), -0.1);
    // нормировка тела не тронута: S(0) = 0, якорь автоматизации на месте
    assert_eq!(source_progress(1.0, &kfs, 0.0), 0.0);
}

#[test]
fn t138_warp_reads_left_handle_without_reset_glitches() {
    // варп 0.5×, mix 0 (реверб прозрачен): чтение с отрицательного local
    // не паникует, не даёт underrun'ов и продолжается непрерывно в тело
    let fx = [FxInstance::slowed_reverb(0.5, 0.0, 85.0)];
    let (src_in, len, head) = (48_000i64, 8_192i64, 2_048i64);
    let mut p = ClipProcessor::new(&fx, &Default::default(), src_in, len);
    p.set_material_ext(head, 0);
    let mut out = vec![0f32; ((head + 4_096) as usize) * CH];
    let mut underruns = 0u64;
    let mut done = 0i64;
    while done < head + 4_096 {
        let n = ((head + 4_096) - done).min(512) as usize;
        let s = (done as usize) * CH;
        underruns += p.read(-head + done, &mut out[s..s + n * CH], &mut ramp_pull);
        done += n as i64;
    }
    assert_eq!(underruns, 0, "handles must not underrun (source is total)");
    // материал ручки — настоящий источник, не тишина (после прайминга sinc)
    let probe = ((head / 2) as usize) * CH;
    assert!(out[probe] != 0.0, "left handle carries real source material");
}

#[test]
fn t138_source_window_ext_and_compat() {
    // делегат: material = 0..len байт-совместим со старой сигнатурой
    let warp = [FxInstance::slowed_reverb(0.82, 0.1, 85.0)];
    let plain = [FxInstance::Gain { db: 0.0 }];
    for fx in [&warp[..], &plain[..]] {
        for local in [0..1024i64, 3_000..9_000, 7_000..8_000] {
            assert_eq!(
                chain_source_window(fx, &Default::default(), 8_192, local.clone()),
                chain_source_window_ext(fx, &Default::default(), 8_192, 0..8_192, local),
            );
        }
    }
    // ручки: окно источника уходит в отрицательные смещения (левее source_in);
    // pre-roll клампится началом материала, не нулём
    let w = chain_source_window_ext(&plain, &Default::default(), 8_192, -2_048..10_240, -2_048..-1_000);
    assert_eq!(w, -2_048..-1_000, "material start bounds the window, no zero clamp");
    let ww = chain_source_window_ext(&warp, &Default::default(), 8_192, -2_048..10_240, -2_048..0);
    assert!(ww.start < 0, "warp window may reach left of source_in for handles");
}
