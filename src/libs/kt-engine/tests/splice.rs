//! Тест 19 (docs/porting-notes.md, п.22 пользователя): копирование КУСКА
//! события регионом + вставка в другое место — рендер вставленного блока
//! sample-exact равен рендеру исходного диапазона (склейка без швов).

use kt_engine::{AudioRenderer, DirectFetcher};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::ops::{copy_region_block, paste_block};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

/// п.8.1 (Р.28): рендер с ПОЛНОСТЬЮ доступным источником обязан идти без
/// единого underrun'а — иначе рендер спрашивает материал за пределами
/// собственного окна, а на живом кэше это тишина и растущий счётчик.
#[track_caller]
fn no_underruns(s: kt_engine::RenderStats) {
    assert_eq!(s.underruns, 0, "underrun при полностью доступном источнике");
}

const CH: usize = AUDIO_CHANNELS;

fn direct() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp)));
    f
}

#[test]
fn t19_copy_region_fragment_paste_renders_identically() {
    // клип 0..4c, регион вырезает [1c, 2.5c) из его СЕРЕДИНЫ
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000 * 4),
        source_in: Tick::from_audio_frames(48_000), // источник со смещением
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx: vec![],
    });
    p.validate().unwrap();

    let region = Region { start: Tick(90_000), end: Tick(90_000 * 5 / 2) };
    let block = copy_region_block(&p, region);
    assert_eq!(block.clips.len(), 1, "регион пересекает один клип");

    // вставка на 10c (пустое место)
    let paste_at = Tick(90_000 * 10);
    let (mut next_clip, mut next_group, mut next_tr) = (100, 100, 100);
    let (cmds, _end) = paste_block(&block, paste_at, &mut next_clip, &mut next_group, &mut next_tr);
    let mut model = kt_model::TimelineModel::new(p);
    model.apply_batch(cmds).expect("paste_block применяется");
    let p2 = model.project().clone();

    // рендер исходного диапазона и вставленного блока
    let renderer = AudioRenderer::new(&p2);
    let frames = region.end.to_audio_frames() - region.start.to_audio_frames();
    let mut orig = vec![0f32; frames as usize * CH];
    no_underruns(renderer.render_frames(&mut direct(), region.start.to_audio_frames(), &mut orig));
    let mut pasted = vec![0f32; frames as usize * CH];
    no_underruns(renderer.render_frames(&mut direct(), paste_at.to_audio_frames(), &mut pasted));

    assert_eq!(orig, pasted, "вставленный кусок должен звучать sample-exact как оригинал");
    assert!(orig.iter().any(|s| *s != 0.0), "рендер не должен быть тишиной");
}

#[test]
fn t19_two_fragments_pasted_back_to_back_are_seamless() {
    // склейка двух кусков ОДНОГО источника встык: [1c,2c) + [3c,4c) → на 10c
    // ожидание: выход на стыке равен прямому чтению источника без щелчка,
    // т.е. рендер == конкатенация двух эталонных кусков.
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 640, height: 360, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000 * 6),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx: vec![],
    });
    p.validate().unwrap();

    let r1 = Region { start: Tick(90_000), end: Tick(90_000 * 2) };
    let r2 = Region { start: Tick(90_000 * 3), end: Tick(90_000 * 4) };
    let b1 = copy_region_block(&p, r1);
    let b2 = copy_region_block(&p, r2);

    let mut model = kt_model::TimelineModel::new(p);
    let (mut nc, mut ng, mut nt) = (100, 100, 100);
    let at1 = Tick(90_000 * 10);
    let (cmds1, end1) = paste_block(&b1, at1, &mut nc, &mut ng, &mut nt);
    model.apply_batch(cmds1).unwrap();
    let (cmds2, _) = paste_block(&b2, end1, &mut nc, &mut ng, &mut nt);
    model.apply_batch(cmds2).unwrap();
    let p2 = model.project().clone();

    let renderer = AudioRenderer::new(&p2);
    let seg = 48_000usize; // 1 c каждый кусок
    let mut spliced = vec![0f32; seg * 2 * CH];
    no_underruns(renderer.render_frames(&mut direct(), at1.to_audio_frames(), &mut spliced));

    let mut expected = vec![0f32; seg * 2 * CH];
    no_underruns(renderer.render_frames(&mut direct(), r1.start.to_audio_frames(), &mut expected[..seg * CH]));
    no_underruns(renderer.render_frames(&mut direct(), r2.start.to_audio_frames(), &mut expected[seg * CH..]));

    assert_eq!(spliced, expected, "склейка кусков должна быть sample-exact без швов");
}
