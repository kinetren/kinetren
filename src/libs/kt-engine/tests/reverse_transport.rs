//! Тест 42 (docs/porting-notes.md, J4 v1.4): reverse-транспорт.
//! Кольцо при dir<0 наполняется блоками [head−N, head) с покадровым
//! разворотом — конкатенация обратных блоков обязана быть ТОЧНЫМ
//! покадровым зеркалом форвард-прогона того же диапазона (детерминизм,
//! инвариант №11). Плюс loop-вариант: rem_euclid-wrap назад.

use kt_engine::{AudioRenderer, DirectFetcher, reverse_frames};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const N: usize = 1024; // кадров в блоке (как AUDIO_BURST_FRAMES)

fn project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000 * 4),
        source_in: Tick(9_000),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn direct() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp)));
    f
}

/// Покадровое зеркало interleaved-буфера (эталон для сверки).
fn mirror(buf: &[f32]) -> Vec<f32> {
    let n = buf.len() / CH;
    let mut out = vec![0f32; buf.len()];
    for i in 0..n {
        let j = n - 1 - i;
        for ch in 0..CH {
            out[i * CH + ch] = buf[j * CH + ch];
        }
    }
    out
}

#[test]
fn t42_reverse_blocks_are_exact_mirror_of_forward() {
    let renderer = AudioRenderer::new(&project());
    let blocks = 3usize;
    let total = blocks * N;
    let a = 48_000i64; // произвольная позиция

    // форвард: один прогон [a, a+total)
    let mut forward = vec![0f32; total * CH];
    renderer.render_frames_raw(&mut direct(), a, &mut forward);

    // реверс: как render-поток — блоки [head-N, head) с разворотом
    let mut head = a + total as i64;
    let mut rev = Vec::with_capacity(total * CH);
    let mut chunk = vec![0f32; N * CH];
    for _ in 0..blocks {
        let start = head - N as i64;
        renderer.render_frames_raw(&mut direct(), start, &mut chunk);
        reverse_frames(&mut chunk);
        rev.extend_from_slice(&chunk);
        head = start;
    }

    assert_eq!(rev, mirror(&forward), "конкатенация реверс-блоков == зеркало форварда, exact");
}

#[test]
fn t42_reverse_looped_wraps_backwards() {
    let renderer = AudioRenderer::new(&project());
    // регион [rs, re): 2 блока шириной
    let rs = 96_000i64;
    let re = rs + 2 * N as i64;

    // эталон: форвард-материал региона
    let mut region_buf = vec![0f32; 2 * N * CH];
    renderer.render_frames_raw(&mut direct(), rs, &mut region_buf);

    // реверс из линейной позиции head=rs (блок [rs-N, rs) должен
    // замотаться в ХВОСТ региона [re-N, re))
    let mut chunk = vec![0f32; N * CH];
    renderer.render_looped_raw(&mut direct(), rs..re, rs - N as i64, &mut chunk);
    reverse_frames(&mut chunk);
    let tail_mirror = mirror(&region_buf[(N * CH)..]);
    assert_eq!(chunk, tail_mirror, "wrap назад читает хвост региона, exact");
}
