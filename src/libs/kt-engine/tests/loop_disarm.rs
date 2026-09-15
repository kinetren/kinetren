//! Тесты 132-133 (п.132-133 porting-notes, Р.23): «вооружённость» loop-заворота.
//!
//! Семантика — транспортная (advance_frames_wrapped): заворот возможен только
//! из позиции ВНУТРИ региона. Seek правее конца (при реверсе — левее начала)
//! развооружает loop до следующего прыжка: звук играет линейно то, что под
//! курсором, транспорт линеен, wraps молчит. Seek внутрь/левее — луп снова
//! подхватывается. До фикса рендер/префетч/показ фолдили в регион ЛЮБУЮ
//! позицию правее его начала — курсор ехал линейно, а звук крутил регион.

use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, FxStateStore, Player, reverse_frames};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick, TickRange};
use std::sync::atomic::Ordering;

/// п.8.1 (Р.28): рендер с ПОЛНОСТЬЮ доступным источником обязан идти без
/// единого underrun'а — иначе рендер спрашивает материал за пределами
/// собственного окна, а на живом кэше это тишина и растущий счётчик.
#[track_caller]
fn no_underruns(s: kt_engine::RenderStats) {
    assert_eq!(s.underruns, 0, "underrun при полностью доступном источнике");
}

const CH: usize = AUDIO_CHANNELS;
const RS: i64 = 48_000 + 696; // регион не по границе burst'а (как t14)
const RE: i64 = 2 * 48_000 + 264;
const SPAN: i64 = RE - RS;

fn project_with_audio_clips(clips: Vec<ClipEvent>) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    let mut media_ids: Vec<MediaId> = clips.iter().map(|c| c.media).collect();
    media_ids.sort();
    media_ids.dedup();
    for id in media_ids {
        p.media.push(MediaRef { id, path: format!("synth:{}", id.0) });
    }
    p.clips = clips;
    p.validate().unwrap();
    p
}

fn audio_clip(
    id: u64,
    lane: u32,
    start_f: i64,
    dur_f: i64,
    src_in_f: i64,
    media: u64,
) -> ClipEvent {
    ClipEvent {
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Audio,
        lane,
        start: Tick::from_audio_frames(start_f),
        duration: Tick::from_audio_frames(dur_f),
        source_in: Tick::from_audio_frames(src_in_f),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn direct(media_patterns: &[(u64, AudioPattern)]) -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    for &(id, pat) in media_patterns {
        f.add_source(MediaId(id), Box::new(SyntheticSource::new(Tick(90_000 * 60), pat)));
    }
    f
}

/// Плеер с одним ramp-клипом 8 с и loop-регионом [RS, RE).
fn ramp_player() -> (Player, rtrb::Consumer<f32>, Project) {
    let d = 8 * 48_000i64;
    let p = project_with_audio_clips(vec![audio_clip(1, 0, 0, d, 0, 1)]);
    let (player, ring) = Player::new(
        p.clone(),
        vec![(
            MediaId(1),
            Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp)) as _,
        )],
    );
    player.set_loop_region(Some(TickRange::new(
        Tick::from_audio_frames(RS),
        Tick::from_audio_frames(RE),
    )));
    (player, ring, p)
}

/// Детерминированный прогрев кэша плеера (по образцу t14, без поллинга).
fn warm_frames(player: &Player, frames: std::ops::Range<i64>) {
    let mut warm = DirectFetcher::new(Some(player.cache.clone()));
    warm.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp)),
    );
    let b0 = frames.start.div_euclid(AUDIO_BURST_FRAMES as i64).max(0);
    let b1 = (frames.end - 1).div_euclid(AUDIO_BURST_FRAMES as i64) + 1;
    for b in b0..b1 {
        warm.audio_burst(MediaId(1), b);
    }
}

/// Продвинуть consumed_frames и дождаться, пока render-поток применит его к
/// транспорту (swap в 0). Тест качает кольцо быстрее реального времени, а
/// wraps считает СОБЫТИЯ заворота (1 на advance): без ожидания кванты
/// слипаются в один гигантский advance и wraps перестаёт отражать круги.
fn bump_consumed(player: &Player, frames: i64) {
    player.shared.consumed_frames.fetch_add(frames, Ordering::AcqRel);
    let start = std::time::Instant::now();
    while player.shared.consumed_frames.load(Ordering::Acquire) != 0 {
        std::thread::sleep(std::time::Duration::from_millis(1));
        assert!(start.elapsed().as_secs() < 10, "render-поток не применяет consumed");
    }
}

/// «Колбэк»: снять `frames` кадров из кольца, продвигая consumed_frames
/// квантами колбэка (2048) ровно на снятое — playhead в конце точен.
fn collect_frames(
    player: &Player,
    pop: &mut dyn FnMut() -> Option<f32>,
    frames: usize,
) -> Vec<f32> {
    let total = frames * CH;
    let mut got = Vec::with_capacity(total);
    let mut bumped = 0i64;
    let start = std::time::Instant::now();
    while got.len() < total {
        match pop() {
            Some(v) => {
                got.push(v);
                if got.len() % 4096 == 0 {
                    bump_consumed(player, 2048);
                    bumped += 2048;
                }
            }
            None => std::thread::sleep(std::time::Duration::from_millis(1)),
        }
        assert!(start.elapsed().as_secs() < 60, "кольцо не наполняется");
    }
    let rest = frames as i64 - bumped;
    if rest > 0 {
        bump_consumed(player, rest);
    }
    got
}

/// Пауза → seek → дренаж кольца (детерминизм: сборка начнётся ровно с цели).
fn jump_paused(player: &Player, pop: &mut dyn FnMut() -> Option<f32>, to_frames: i64) {
    player.pause();
    std::thread::sleep(std::time::Duration::from_millis(300));
    player.seek(Tick::from_audio_frames(to_frames));
    std::thread::sleep(std::time::Duration::from_millis(300));
    while pop().is_some() {}
    let _ = player.shared.flush.swap(false, Ordering::AcqRel);
}

// дать render-потоку применить последний consumed к транспорту
fn settle(_player: &Player) {
    std::thread::sleep(std::time::Duration::from_millis(300));
}

fn assert_stream_eq(got: &[f32], expect: &[f32], what: &str) {
    if let Some(i) = (0..got.len()).find(|&i| got[i] != expect[i]) {
        panic!(
            "{what}: расхождение на сэмпле {} (кадр {}): got {:?} expect {:?}",
            i,
            i / 2,
            &got[i.saturating_sub(2)..(i + 6).min(got.len())],
            &expect[i.saturating_sub(2)..(i + 6).min(expect.len())],
        );
    }
}

/// Тест 132: seek ПРАВЕЕ конца региона при loop-плее развооружает loop
/// (звук линейный, транспорт линейный, wraps молчит) до следующего seek;
/// seek внутрь снова вооружает; цель ровно на конце региона — внутри
/// (инклюзивность как у advance_frames_wrapped).
#[test]
fn t132_seek_right_of_region_disarms_loop_until_next_jump() {
    let (player, mut ring, p) = ramp_player();
    let mut pop = move || ring.pop().ok();
    let r = AudioRenderer::new(&p);

    // Фаза A: луп жив (как t14) — минимум два оборота.
    warm_frames(&player, RS - 4096..RE + 4096);
    player.seek(Tick::from_audio_frames(RS));
    std::thread::sleep(std::time::Duration::from_millis(300));
    player.play();
    let _ = collect_frames(&player, &mut pop, (3 * SPAN) as usize);
    settle(&player);
    assert!(player.transport.wraps() >= 2, "луп не завёлся: wraps={}", player.transport.wraps());

    // Фаза B: seek правее конца — линейная игра того, что под курсором.
    let target = RE + 10_000;
    let total_b = 72_000usize; // 1.5 c
    warm_frames(&player, target - 8_192..target + total_b as i64 + 32_768);
    jump_paused(&player, &mut pop, target);
    let wraps_b = player.transport.wraps();
    player.play();
    let got = collect_frames(&player, &mut pop, total_b);
    settle(&player);
    assert_eq!(player.transport.wraps(), wraps_b, "disarm: транспорт не должен wrap'аться");
    assert_eq!(
        player.transport.playhead_frames(),
        target + total_b as i64,
        "disarm: playhead обязан идти линейно от точки seek"
    );
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut f = direct(&[(1, AudioPattern::Ramp)]);
    let mut expect = vec![0f32; got.len()];
    no_underruns(r.render_frames_with(&mut f, &mut store, target, &mut expect));
    assert_stream_eq(&got, &expect, "фаза B (линейный рендер правее региона)");

    // Фаза C: seek внутрь региона — луп снова вооружён.
    let target_c = RS + 1_000;
    jump_paused(&player, &mut pop, target_c);
    let wraps_c = player.transport.wraps();
    player.play();
    let got_c = collect_frames(&player, &mut pop, (2 * SPAN) as usize);
    settle(&player);
    assert!(player.transport.wraps() > wraps_c, "re-arm: wraps обязан расти");
    let mut store_c = FxStateStore::new();
    r.sync_fx(&mut store_c);
    let mut fc = direct(&[(1, AudioPattern::Ramp)]);
    let mut expect_c = vec![0f32; got_c.len()];
    no_underruns(r.render_looped_with(&mut fc, &mut store_c, RS..RE, target_c, &mut expect_c));
    assert_stream_eq(&got_c, &expect_c, "фаза C (луп после re-arm)");

    // Фаза D: цель ровно на конце региона — считается внутри, лупится.
    jump_paused(&player, &mut pop, RE);
    let wraps_d = player.transport.wraps();
    player.play();
    let got_d = collect_frames(&player, &mut pop, 24_000);
    settle(&player);
    assert!(player.transport.wraps() > wraps_d, "цель == конец региона обязана лупиться");
    let mut store_d = FxStateStore::new();
    r.sync_fx(&mut store_d);
    let mut fd = direct(&[(1, AudioPattern::Ramp)]);
    let mut expect_d = vec![0f32; got_d.len()];
    no_underruns(r.render_looped_with(&mut fd, &mut store_d, RS..RE, RE, &mut expect_d));
    assert_stream_eq(&got_d, &expect_d, "фаза D (инклюзивная граница)");
    player.pause();
}

/// Эталон reverse-потока: зеркало блочного цикла движка (блок ПЕРЕД головой,
/// разворот покадрово; loop-ветка при head <= RE, линейная — выше).
fn reverse_reference(r: &AudioRenderer, from: i64, frames: usize, looped: bool) -> Vec<f32> {
    let mut f = direct(&[(1, AudioPattern::Ramp)]);
    let mut head = from;
    let mut out: Vec<f32> = Vec::with_capacity(frames * CH);
    let mut block = vec![0f32; AUDIO_BURST_FRAMES * CH];
    while out.len() < frames * CH {
        let start = head - AUDIO_BURST_FRAMES as i64;
        if looped && head <= RE {
            no_underruns(r.render_looped_raw(&mut f, RS..RE, start, &mut block));
        } else {
            no_underruns(r.render_frames_raw(&mut f, start, &mut block));
        }
        reverse_frames(&mut block);
        out.extend_from_slice(&block);
        head = start;
    }
    out.truncate(frames * CH);
    out
}

/// Тест 133: reverse-loop. (a) Устойчивый reverse-loop живёт дольше круга
/// (до фикса render_head, уйдя ниже начала региона, гасил guard `> rs` и
/// звук навсегда уходил в линейный рендер левее региона). (b) Seek правее
/// конца при реверсе — линейный ПОДХОД до региона, затем луп подхватывается.
#[test]
fn t133_reverse_loop_lives_and_right_approach_is_linear() {
    let (player, mut ring, p) = ramp_player();
    let mut pop = move || ring.pop().ok();
    let r = AudioRenderer::new(&p);

    // (a) устойчивый reverse-loop: старт из середины региона.
    let mid = RS + SPAN / 2; // 72_480, кратно 8 — тик↔кадр точен
    warm_frames(&player, RS - 4096..RE + 4096);
    player.seek(Tick::from_audio_frames(mid));
    player.set_playback_rate(-1.0);
    std::thread::sleep(std::time::Duration::from_millis(300));
    player.play();
    let got = collect_frames(&player, &mut pop, (3 * SPAN) as usize);
    settle(&player);
    assert!(
        player.transport.wraps() >= 2,
        "reverse-loop обязан жить дольше круга: wraps={}",
        player.transport.wraps()
    );
    let expect = reverse_reference(&r, mid, (3 * SPAN) as usize, true);
    assert_stream_eq(&got, &expect, "(a) устойчивый reverse-loop");

    // (b) seek правее региона при реверсе: линейный подход, затем вход в луп.
    let target = RE + 72_000; // 1.5 c правее конца
    warm_frames(&player, RE - 4096..target + 4096);
    jump_paused(&player, &mut pop, target);
    let wraps_b = player.transport.wraps();
    player.play();
    let approach = 48_000usize; // 1 c — регион ещё не достигнут
    let mut got_b = collect_frames(&player, &mut pop, approach);
    settle(&player);
    assert_eq!(player.transport.wraps(), wraps_b, "подход справа обязан быть линейным");
    // докручиваем до входа в регион и через круг — луп подхватился
    got_b.extend(collect_frames(&player, &mut pop, (72_000 - approach as i64 + 2 * SPAN) as usize));
    settle(&player);
    assert!(player.transport.wraps() > wraps_b, "после входа в регион wraps обязан расти");
    let expect_b = reverse_reference(&r, target, got_b.len() / CH, true);
    assert_stream_eq(&got_b, &expect_b, "(b) линейный подход и вход в луп");
    player.pause();
}
