//! Tests 132-133 (items 132-133, R.23): the “armed” state of the loop wrap.
//!
//! The semantics are transport-level (advance_frames_wrapped): a wrap is possible only
//! from a position INSIDE the region. A seek to the right of the end (on reverse — to the left of the start)
//! disarms the loop until the next jump: the sound plays linearly what is under the cursor,
//! the transport is linear, wraps stays silent. A seek inside/left — the loop is picked up again.
//! Before the fix the render, prefetch and display folded ANY position to the right of the region's start
//! into the region — the cursor moved linearly while the sound looped the region.

use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, FxStateStore, Player, reverse_frames};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick, TickRange};
use std::sync::atomic::Ordering;

/// item 8.1 (R.28): a render with a FULLY available source must run without a
/// single underrun — otherwise the render asks for material beyond its own window,
/// and on a live cache that is silence plus a growing counter.
#[track_caller]
fn no_underruns(s: kt_engine::RenderStats) {
    assert_eq!(s.underruns, 0, "underrun with a fully available source");
}

const CH: usize = AUDIO_CHANNELS;
const RS: i64 = 48_000 + 696; // a region not on a burst boundary (as in t14)
const RE: i64 = 2 * 48_000 + 264;
const SPAN: i64 = RE - RS;

fn project_with_audio_clips(clips: Vec<ClipEvent>) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    let mut media_ids: Vec<MediaId> = clips.iter().map(|c| c.media).collect();
    media_ids.sort();
    media_ids.dedup();
    for id in media_ids {
        p.media.push(MediaRef::new(id, format!("synth:{}", id.0)));
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
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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

/// A player with a single ramp clip of 8 s and a loop region [RS, RE).
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

/// Deterministic warm-up of the player's cache (following t14, without polling).
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

/// Advance consumed_frames and wait until the render thread applies it to the
/// transport (swap to 0). The test pumps the ring faster than real time, and
/// wraps counts the wrap EVENTS (1 per advance): without waiting the quanta
/// merge into one giant advance and wraps stops reflecting the cycles.
fn bump_consumed(player: &Player, frames: i64) {
    player.shared.consumed_frames.fetch_add(frames, Ordering::AcqRel);
    let start = std::time::Instant::now();
    while player.shared.consumed_frames.load(Ordering::Acquire) != 0 {
        std::thread::sleep(std::time::Duration::from_millis(1));
        assert!(start.elapsed().as_secs() < 10, "the render stream does not apply consumed");
    }
}

/// “Callback”: take `frames` frames from the ring, advancing consumed_frames
/// by callback quanta (2048) by exactly what was taken — the playhead is exact at the end.
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
        assert!(start.elapsed().as_secs() < 60, "ring does not fill");
    }
    let rest = frames as i64 - bumped;
    if rest > 0 {
        bump_consumed(player, rest);
    }
    got
}

/// Pause → seek → drain the ring (determinism: the build will start exactly at the target).
fn jump_paused(player: &Player, pop: &mut dyn FnMut() -> Option<f32>, to_frames: i64) {
    player.pause();
    std::thread::sleep(std::time::Duration::from_millis(300));
    player.seek(Tick::from_audio_frames(to_frames));
    std::thread::sleep(std::time::Duration::from_millis(300));
    while pop().is_some() {}
    let _ = player.shared.flush.swap(false, Ordering::AcqRel);
}

// let the render thread apply the last consumed to the transport
fn settle(_player: &Player) {
    std::thread::sleep(std::time::Duration::from_millis(300));
}

fn assert_stream_eq(got: &[f32], expect: &[f32], what: &str) {
    if let Some(i) = (0..got.len()).find(|&i| got[i] != expect[i]) {
        panic!(
            "{what}: divergence at sample {} (frame {}): got {:?} expect {:?}",
            i,
            i / 2,
            &got[i.saturating_sub(2)..(i + 6).min(got.len())],
            &expect[i.saturating_sub(2)..(i + 6).min(expect.len())],
        );
    }
}

/// Test 132: a seek RIGHT of the region's end during loop playback disarms the loop
/// (the sound linear, the transport linear, wraps silent) until the next seek;
/// a seek inside arms it again; a target exactly at the region's end is inside
/// (inclusivity as in advance_frames_wrapped).
#[test]
fn t132_seek_right_of_region_disarms_loop_until_next_jump() {
    let (player, mut ring, p) = ramp_player();
    let mut pop = move || ring.pop().ok();
    let r = AudioRenderer::new(&p);

    // Phase A: the loop is alive (as t14) — at least two turns.
    warm_frames(&player, RS - 4096..RE + 4096);
    player.seek(Tick::from_audio_frames(RS));
    std::thread::sleep(std::time::Duration::from_millis(300));
    player.play();
    let _ = collect_frames(&player, &mut pop, (3 * SPAN) as usize);
    settle(&player);
    assert!(player.transport.wraps() >= 2, "loop did not start: wraps={}", player.transport.wraps());

    // Phase B: a seek right of the end — the linear play of what is under the cursor.
    let target = RE + 10_000;
    let total_b = 72_000usize; // 1.5 c
    warm_frames(&player, target - 8_192..target + total_b as i64 + 32_768);
    jump_paused(&player, &mut pop, target);
    let wraps_b = player.transport.wraps();
    player.play();
    let got = collect_frames(&player, &mut pop, total_b);
    settle(&player);
    assert_eq!(player.transport.wraps(), wraps_b, "disarm: transport must not wrap");
    assert_eq!(
        player.transport.playhead_frames(),
        target + total_b as i64,
        "disarm: playhead must run linearly from the seek point"
    );
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut f = direct(&[(1, AudioPattern::Ramp)]);
    let mut expect = vec![0f32; got.len()];
    no_underruns(r.render_frames_with(&mut f, &mut store, target, &mut expect));
    assert_stream_eq(&got, &expect, "phase B (linear render to the right of the region)");

    // Phase C: a seek inside the region — the loop is armed again.
    let target_c = RS + 1_000;
    jump_paused(&player, &mut pop, target_c);
    let wraps_c = player.transport.wraps();
    player.play();
    let got_c = collect_frames(&player, &mut pop, (2 * SPAN) as usize);
    settle(&player);
    assert!(player.transport.wraps() > wraps_c, "re-arm: wraps must grow");
    let mut store_c = FxStateStore::new();
    r.sync_fx(&mut store_c);
    let mut fc = direct(&[(1, AudioPattern::Ramp)]);
    let mut expect_c = vec![0f32; got_c.len()];
    no_underruns(r.render_looped_with(&mut fc, &mut store_c, RS..RE, target_c, &mut expect_c));
    assert_stream_eq(&got_c, &expect_c, "phase C (loop after re-arm)");

    // Phase D: a target exactly at the region's end — counted as inside, loops.
    jump_paused(&player, &mut pop, RE);
    let wraps_d = player.transport.wraps();
    player.play();
    let got_d = collect_frames(&player, &mut pop, 24_000);
    settle(&player);
    assert!(player.transport.wraps() > wraps_d, "a target == the end of the region must loop");
    let mut store_d = FxStateStore::new();
    r.sync_fx(&mut store_d);
    let mut fd = direct(&[(1, AudioPattern::Ramp)]);
    let mut expect_d = vec![0f32; got_d.len()];
    no_underruns(r.render_looped_with(&mut fd, &mut store_d, RS..RE, RE, &mut expect_d));
    assert_stream_eq(&got_d, &expect_d, "phase D (inclusive boundary)");
    player.pause();
}

/// Reference of the reverse stream: a mirror of the engine's block cycle (the block BEFORE the head,
/// reversed frame by frame; the loop branch when head <= RE, linear above it).
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

/// Test 133: reverse-loop. (a) A stable reverse loop lives longer than a cycle
/// (before the fix render_head, having gone below the region's start, switched off the guard `> rs` and
/// the sound went for good into the linear render to the left of the region). (b) A seek right of the
/// end on reverse — a linear APPROACH to the region, then the loop is picked up.
#[test]
fn t133_reverse_loop_lives_and_right_approach_is_linear() {
    let (player, mut ring, p) = ramp_player();
    let mut pop = move || ring.pop().ok();
    let r = AudioRenderer::new(&p);

    // (a) stable reverse loop: start from the middle of the region.
    let mid = RS + SPAN / 2; // 72_480, a multiple of 8 — the tick↔frame mapping is exact
    warm_frames(&player, RS - 4096..RE + 4096);
    player.seek(Tick::from_audio_frames(mid));
    player.set_playback_rate(-1.0);
    std::thread::sleep(std::time::Duration::from_millis(300));
    player.play();
    let got = collect_frames(&player, &mut pop, (3 * SPAN) as usize);
    settle(&player);
    assert!(
        player.transport.wraps() >= 2,
        "reverse loop must live longer than a lap: wraps={}",
        player.transport.wraps()
    );
    let expect = reverse_reference(&r, mid, (3 * SPAN) as usize, true);
    assert_stream_eq(&got, &expect, "(a) a stable reverse loop");

    // (b) seek right of the region on reverse: a linear approach, then entry into the loop.
    let target = RE + 72_000; // 1.5 s right of the end
    warm_frames(&player, RE - 4096..target + 4096);
    jump_paused(&player, &mut pop, target);
    let wraps_b = player.transport.wraps();
    player.play();
    let approach = 48_000usize; // 1 s — the region is not yet reached
    let mut got_b = collect_frames(&player, &mut pop, approach);
    settle(&player);
    assert_eq!(player.transport.wraps(), wraps_b, "the approach from the right must be linear");
    // wind on to the entry into the region and through one cycle — the loop was picked up
    got_b.extend(collect_frames(&player, &mut pop, (72_000 - approach as i64 + 2 * SPAN) as usize));
    settle(&player);
    assert!(player.transport.wraps() > wraps_b, "after entering the region wraps must grow");
    let expect_b = reverse_reference(&r, target, got_b.len() / CH, true);
    assert_stream_eq(&got_b, &expect_b, "(b) linear approach and entry into the loop");
    player.pause();
}
