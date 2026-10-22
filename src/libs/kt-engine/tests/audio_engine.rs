//! Regression tests of the engine (docs/porting-notes.md):
//! 1 — audio-event concatenation sample-exact;
//! 2 — equal-power crossfade / 1-N;
//! 3 — gain and summing of tracks;
//! 7 — render determinism (the audio part);
//! 8 — cache/coverage;
//! 14 — loop region: continuity of wrap, head pin.

use kt_cache::{BurstCache, BurstKind, CacheConfig, SourceId};
use kt_engine::{AudioFetcher, AudioRenderer, CacheFetcher, DirectFetcher, db_to_linear};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};
use std::sync::Arc;

/// item 8.1 (R.28): a render with a FULLY available source must run without a
/// single underrun — otherwise the render asks for material beyond its own window,
/// and on a live cache that is silence and a growing counter.
#[track_caller]
fn no_underruns(s: kt_engine::RenderStats) {
    assert_eq!(s.underruns, 0, "underrun with a fully available source");
}

const CH: usize = AUDIO_CHANNELS;

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
    p.normalize(); // tracks to the content (R.62)
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

// ---- Test 1: concatenation ---------------------------------------------------

#[test]
fn t1_butt_joined_clips_are_sample_exact() {
    // Two clips back to back, the second continues the source from the same place:
    // the output must be INDISTINGUISHABLE from a continuous read of the source. Exact f32.
    let d = 3 * AUDIO_BURST_FRAMES as i64 + 136; // clip boundaries are not on bursts
    let p =
        project_with_audio_clips(vec![audio_clip(1, 0, 0, d, 0, 1), audio_clip(2, 0, d, d, d, 1)]);
    let r = AudioRenderer::new(&p);
    let mut f = direct(&[(1, AudioPattern::Ramp)]);
    let total = (2 * d) as usize;
    let mut out = vec![0f32; total * CH];
    let stats = r.render_frames(&mut f, 0, &mut out);
    assert_eq!(stats.underruns, 0);
    for s in 0..total as i64 {
        for ch in 0..CH {
            let expect = SyntheticSource::sample_at(AudioPattern::Ramp, s, ch);
            assert_eq!(out[s as usize * CH + ch], expect, "gap at sample {s} (seam at {d})");
        }
    }
}

#[test]
fn t1_second_clip_reads_its_own_source_window() {
    // The second clip with source_in=0: after the seam the output = the source from zero.
    let d = 2 * AUDIO_BURST_FRAMES as i64 + 40;
    let p =
        project_with_audio_clips(vec![audio_clip(1, 0, 0, d, 0, 1), audio_clip(2, 0, d, d, 0, 1)]);
    let r = AudioRenderer::new(&p);
    let mut f = direct(&[(1, AudioPattern::Ramp)]);
    let total = (2 * d) as usize;
    let mut out = vec![0f32; total * CH];
    no_underruns(r.render_frames(&mut f, 0, &mut out));
    for s in 0..d {
        assert_eq!(
            out[(d + s) as usize * CH],
            SyntheticSource::sample_at(AudioPattern::Ramp, s, 0)
        );
    }
}

// ---- Test 2: crossfade ------------------------------------------------------

#[test]
fn t2_crossfade_transition_is_equal_power() {
    // R.24: crossfade — an explicit transition at the joint (there are no overlaps any more,
    // test 135 rejects them). The window and formulas are the same as for the former
    // overlap: outgoing √(1−t), incoming √t.
    let d = 4 * AUDIO_BURST_FRAMES as i64; // 4096 — end of the window
    let ov = AUDIO_BURST_FRAMES as i64 + 336; // 1360 — window width
    let start2 = d - ov; // 2736 — window start
    let joint = start2 + ov / 2; // 3416 — joint (the middle of the former overlay)
    let mut p = project_with_audio_clips(vec![
        audio_clip(1, 0, 0, joint, 0, 1),
        audio_clip(2, 0, joint, d + start2 - joint, ov / 2, 2),
    ]);
    p.transitions.push(kt_model::TransitionEvent {
        id: kt_model::TransitionId(1),
        a: Some(kt_model::ClipId(1)),
        b: Some(kt_model::ClipId(2)),
        in_offset: kt_time::Tick::from_audio_frames(ov / 2),
        out_offset: kt_time::Tick::from_audio_frames(ov / 2),
        kind: kt_model::TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    let r = AudioRenderer::new(&p);
    let mut f =
        direct(&[(1, AudioPattern::Dc { value: 1.0 }), (2, AudioPattern::Dc { value: 1.0 })]);
    let total = (start2 + d) as usize;
    let mut out = vec![0f32; total * CH];
    assert_eq!(r.render_frames(&mut f, 0, &mut out).underruns, 0);
    for s in 0..total as i64 {
        let expect = if s < start2 {
            1.0f32 // only outgoing
        } else if s < d {
            let t = (s - start2) as f64 / ov as f64;
            ((1.0 - t).sqrt() + t.sqrt()) as f32
        } else {
            1.0f32 // only incoming
        };
        let got = out[s as usize * CH];
        assert!((got - expect).abs() < 1e-6, "sample {s}: got {got}, expect {expect}");
    }
    // unity power on equal inputs in the transition window: w1²+w2² == 1
    for s in start2..d {
        let t = (s - start2) as f64 / ov as f64;
        let pw = (1.0 - t) + t;
        assert!((pw - 1.0).abs() < 1e-12);
    }
}

#[test]
fn t2_different_lanes_sum_without_weights() {
    let d = AUDIO_BURST_FRAMES as i64;
    let p =
        project_with_audio_clips(vec![audio_clip(1, 0, 0, d, 0, 1), audio_clip(2, 1, 0, d, 0, 2)]);
    let r = AudioRenderer::new(&p);
    let mut f =
        direct(&[(1, AudioPattern::Dc { value: 0.25 }), (2, AudioPattern::Dc { value: 0.5 })]);
    let mut out = vec![0f32; d as usize * CH];
    no_underruns(r.render_frames(&mut f, 0, &mut out));
    assert_eq!(out[0], 0.75);
}

// ---- Test 3: gain -----------------------------------------------------------

#[test]
fn t3_gain_is_applied_exactly() {
    let d = AUDIO_BURST_FRAMES as i64;
    let mut clip = audio_clip(1, 0, 0, d, 0, 1);
    clip.gain_db = -6.0;
    let p = project_with_audio_clips(vec![clip]);
    let r = AudioRenderer::new(&p);
    let mut f = direct(&[(1, AudioPattern::Dc { value: 0.8 })]);
    let mut out = vec![0f32; d as usize * CH];
    no_underruns(r.render_frames(&mut f, 0, &mut out));
    assert_eq!(out[0], 0.8 * db_to_linear(-6.0));
}

// ---- Test 7 (audio): determinism -------------------------------------------

#[test]
fn t7_render_is_deterministic_regardless_of_history() {
    let d = 5 * AUDIO_BURST_FRAMES as i64;
    // R.24: the former 496 overlap converted into a joint + transition (the window is the same)
    let joint = d - 248;
    let mut p = project_with_audio_clips(vec![
        audio_clip(1, 0, 0, joint, 0, 1),
        audio_clip(2, 0, joint, d - 248, 248, 2),
    ]);
    p.transitions.push(kt_model::TransitionEvent {
        id: kt_model::TransitionId(1),
        a: Some(kt_model::ClipId(1)),
        b: Some(kt_model::ClipId(2)),
        in_offset: kt_time::Tick::from_audio_frames(248),
        out_offset: kt_time::Tick::from_audio_frames(248),
        kind: kt_model::TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    let r = AudioRenderer::new(&p);
    let target = d - 1000..d + 1000; // range through the seam/crossfade

    // cold render
    let mut f1 = direct(&[(1, AudioPattern::Sine { hz: 440.0 }), (2, AudioPattern::Ramp)]);
    let mut cold = vec![0f32; (target.end - target.start) as usize * CH];
    no_underruns(r.render_frames(&mut f1, target.start, &mut cold));

    // render after “jumps” along the timeline by the same fetcher
    let mut f2 = direct(&[(1, AudioPattern::Sine { hz: 440.0 }), (2, AudioPattern::Ramp)]);
    let mut scratch = vec![0f32; 512 * CH];
    for pos in [7 * d / 2, 0, 3 * d / 2, d, 42] {
        no_underruns(r.render_frames(&mut f2, pos, &mut scratch));
    }
    let mut warm = vec![0f32; cold.len()];
    no_underruns(r.render_frames(&mut f2, target.start, &mut warm));

    assert_eq!(cold, warm, "access history must not affect the result");
}

// ---- Test 8: cache and coverage -------------------------------------------------

#[test]
fn t8_eviction_shrinks_coverage_exactly() {
    // Budget for 32 audio bursts (8 KiB each)
    let burst_bytes = AUDIO_BURST_FRAMES * CH * 4;
    let cache = BurstCache::new(CacheConfig {
        max_audio_bytes: 32 * burst_bytes,
        max_video_bytes: usize::MAX,
    });
    let src = SourceId(1);
    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp);
    use kt_media::MediaSource;
    for idx in 0..64 {
        cache.insert_audio(src, Arc::new(synth.read_audio_burst(idx).unwrap()));
    }
    // exactly the last 32 remain
    assert_eq!(cache.audio_bytes(), 32 * burst_bytes);
    let cov = cache.coverage(src, BurstKind::Audio, 0..64);
    assert_eq!(cov, vec![32..64], "coverage must shrink exactly with eviction");
    assert!(cache.get_audio(src, 0).is_none());
    assert!(cache.get_audio(src, 63).is_some());
}

#[test]
fn t8_quality_does_not_cross_satisfy() {
    use kt_media::{Quality, VideoFrame};
    let cache = BurstCache::new(CacheConfig::default());
    let src = SourceId(1);
    let q1 = Some(Quality { width: 640, height: 360 });
    let q2 = Some(Quality { width: 1920, height: 1080 });
    let frame = Arc::new(VideoFrame {
        pts: Tick(0),
        width: 640,
        height: 360,
        rgba: vec![0u8; 16].into_boxed_slice(),
        has_alpha: false,
    });
    cache.insert_video(src, q1, 5, frame);
    assert!(cache.get_video(src, q1, 5).is_some());
    assert!(cache.get_video(src, q2, 5).is_none(), "other quality ≠ warm");
    assert!(cache.get_video(src, None, 5).is_none());
    assert_eq!(cache.coverage(src, BurstKind::video(q2), 0..10), vec![]);
    assert_eq!(cache.coverage(src, BurstKind::video(q1), 0..10), vec![5..6]);
}

#[test]
fn t8_epoch_changes_only_on_mutation() {
    let cache = BurstCache::new(CacheConfig::default());
    let e0 = cache.epoch();
    let _ = cache.coverage(SourceId(1), BurstKind::Audio, 0..100);
    let _ = cache.get_audio(SourceId(1), 0);
    assert_eq!(cache.epoch(), e0, "reads do not change epoch");
    cache.insert_audio(SourceId(1), Arc::new(kt_media::AudioBurst::silence(0)));
    assert_ne!(cache.epoch(), e0);
}

// ---- Test 14: loop region ---------------------------------------------------

#[test]
fn t14_loop_wrap_is_continuous_and_warm_cache_has_no_misses() {
    let d = 6 * AUDIO_BURST_FRAMES as i64;
    let p = project_with_audio_clips(vec![audio_clip(1, 0, 0, d, 0, 1)]);
    let r = AudioRenderer::new(&p);

    // The region is aligned neither to a burst nor to a video frame.
    let region = 696i64..(3 * AUDIO_BURST_FRAMES as i64 + 264);
    let region_len = region.end - region.start;

    // 1) Continuity: the loop output == the reference, glued from direct renders.
    let mut f = direct(&[(1, AudioPattern::Ramp)]);
    let total = (region_len * 3 + 777) as usize;
    let mut looped = vec![0f32; total * CH];
    let stats = r.render_looped(&mut f, region.clone(), region.start, &mut looped);
    assert_eq!(stats.underruns, 0);
    for o in 0..total as i64 {
        let tl = region.start + (o % region_len);
        let mut one = [0f32; 2];
        no_underruns(r.render_frames(&mut f, tl, &mut one));
        assert_eq!(looped[o as usize * CH], one[0], "wrap broke the stream at frame {o}");
    }

    // 2) Warm cache: prefetch of the region + head pin → CacheFetcher without misses,
    //    and the head survives eviction by garbage.
    let burst_bytes = AUDIO_BURST_FRAMES * CH * 4;
    let cache = Arc::new(BurstCache::new(CacheConfig {
        max_audio_bytes: 8 * burst_bytes,
        max_video_bytes: usize::MAX,
    }));
    let mut warm = DirectFetcher::new(Some(cache.clone()));
    warm.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp)),
    );
    // prefetch = the region's wanted keys (as the prefetch thread will do)
    for (media, bursts) in r.wanted_audio_bursts(region.clone()) {
        for b in bursts {
            warm.audio_burst(media, b);
        }
    }
    // head pin of the region (the first 2 bursts)
    let head0 = region.start.div_euclid(AUDIO_BURST_FRAMES as i64);
    cache.pin_range(kt_engine::source_id(MediaId(1)), BurstKind::Audio, head0..head0 + 2);

    // pressure: another source flushes the LRU
    let mut noise = DirectFetcher::new(Some(cache.clone()));
    noise.add_source(
        MediaId(2),
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence)),
    );
    for b in 100..112 {
        noise.audio_burst(MediaId(2), b);
    }

    // the head survived
    let cov = cache.coverage(kt_engine::source_id(MediaId(1)), BurstKind::Audio, head0..head0 + 2);
    assert_eq!(cov, vec![head0..head0 + 2], "pin must survive eviction");

    // a wrap on a warm head gives no misses
    let mut cf = CacheFetcher::new(cache.clone());
    let head_frames =
        2 * AUDIO_BURST_FRAMES as i64 - (region.start - head0 * AUDIO_BURST_FRAMES as i64);
    let mut out = vec![0f32; (head_frames.min(region_len)) as usize * CH];
    r.render_frames(&mut cf, region.start, &mut out);
    assert_eq!(cf.miss_count(), 0, "the warm head of a region must not produce Playback misses");
}

// ---- Loop in the Player domain (U3): transport wrap arithmetic --------------------

#[test]
fn t14_transport_wrap_math() {
    use kt_engine::Transport;
    let tr = Transport::new();
    tr.seek_frames(90);
    // without a region — linear
    tr.advance_frames_wrapped(10, None);
    assert_eq!(tr.playhead_frames(), 100);
    // wrap: region [50, 110)
    tr.advance_frames_wrapped(15, Some((50, 110)));
    assert_eq!(tr.playhead_frames(), 50 + (115 - 50) % 60, "wrap to the start of the region");
    // not yet reached the end of the region — linear
    tr.advance_frames_wrapped(5, Some((50, 110)));
    assert_eq!(tr.playhead_frames(), 60);
    // playhead BEFORE the region: no wrap (start before entering the region)
    tr.seek_frames(10);
    tr.advance_frames_wrapped(20, Some((50, 110)));
    assert_eq!(tr.playhead_frames(), 30);
}

#[test]
fn t14_player_loop_stream_equals_render_looped_reference() {
    use kt_engine::{FxStateStore, Player};
    use kt_time::TickRange;
    // Project: ramp clip 4 s; loop region not on a burst boundary.
    let d = 4 * 48_000i64;
    let p = project_with_audio_clips(vec![audio_clip(1, 0, 0, d, 0, 1)]);
    let (player, mut ring) = Player::new(
        p.clone(),
        vec![(
            MediaId(1),
            Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp)) as _,
        )],
    );
    let region_f = 48_000i64 + 696..2 * 48_000 + 264;
    player.set_loop_region(Some(TickRange::new(
        Tick::from_audio_frames(region_f.start),
        Tick::from_audio_frames(region_f.end),
    )));
    player.seek(Tick::from_audio_frames(region_f.start));
    // we wait for the cache to be warmed by the prefetch thread (otherwise the first chunks would be underrun silence).
    // We wait EXACTLY for the region's bursts: since item 167 prefetch does not go past the region's end
    // (there is nothing to wait for there — playback will wrap), the margin “+4 bursts”
    // past the end is now never warmed.
    let b0 = region_f.start.div_euclid(1024);
    let b1 = (region_f.end - 1).div_euclid(1024) + 1;
    let warm_start = std::time::Instant::now();
    loop {
        let cov = player.cache.coverage(
            kt_engine::source_id(MediaId(1)),
            kt_cache::BurstKind::Audio,
            b0..b1,
        );
        if cov.len() == 1 && cov[0] == (b0..b1) {
            break;
        }
        assert!(warm_start.elapsed().as_secs() < 20, "prefetch did not warm the region: {cov:?}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    player.play();

    // consume from the ring as a “callback”: 6 s of output (≈4 loops)
    let total = 6 * 48_000usize;
    let mut got = Vec::with_capacity(total * 2);
    let start = std::time::Instant::now();
    while got.len() < total * 2 {
        match ring.pop() {
            Ok(v) => {
                got.push(v);
                if got.len() % 4096 == 0 {
                    player
                        .shared
                        .consumed_frames
                        .fetch_add(2048, std::sync::atomic::Ordering::AcqRel);
                }
            }
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(1)),
        }
        assert!(start.elapsed().as_secs() < 30, "ring does not fill");
    }
    player.pause();

    // reference: render_looped from the same position
    let r = AudioRenderer::new(&p);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut f = direct(&[(1, AudioPattern::Ramp)]);
    let mut expect = vec![0f32; got.len()];
    no_underruns(r.render_looped_with(
        &mut f,
        &mut store,
        region_f.clone(),
        region_f.start,
        &mut expect,
    ));
    if let Some(i) = (0..got.len()).find(|&i| got[i] != expect[i]) {
        panic!(
            "divergence from reference at sample {} (frame {}): got {:?} expect {:?}",
            i,
            i / 2,
            &got[i.saturating_sub(2)..(i + 6).min(got.len())],
            &expect[i.saturating_sub(2)..(i + 6).min(expect.len())],
        );
    }
}

/// Test 84 (item 7 of round 9): the master volume is CLAMPED in the engine. A failure in the UI
/// (a diverging fader animation) once sent 10^20 here — a whole burst got multiplied by it and went
/// into the sound. The engine must protect itself.
#[test]
fn t84_master_gain_is_clamped() {
    let p = kt_model::Project::new(kt_model::ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    let (player, _rx) = kt_engine::Player::new(p, vec![]);
    player.set_master_gain(1e20);
    assert!(
        player.master_gain() <= kt_engine::player::MASTER_GAIN_MAX,
        "gain is not limited: {}",
        player.master_gain()
    );
    player.set_master_gain(f32::NAN);
    assert!(player.master_gain().is_finite(), "NaN leaked into master gain");
    player.set_master_gain(f32::INFINITY);
    assert!(player.master_gain().is_finite());
    player.set_master_gain(-5.0);
    assert!(player.master_gain() >= 0.0);
    // normal values pass through as is
    player.set_master_gain(0.5);
    assert!((player.master_gain() - 0.5).abs() < 1e-6);
}

// ---- test 162: re-layering of clips (R.28) -----------------------------------

/// Overlapping clips of one track is legal again and sounds as in v1:
/// equal-power for two, 1/N for three or more. The user's requirement:
/// “a project option that will allow several events of one track to be layered
/// at the same time… turn it on by default.”
#[test]
fn t162_overlap_mixes_like_v1() {
    let media = &[(1u64, AudioPattern::Ramp)];
    // overlay [1000, 2000): equal-power across the overlay zone
    let p = project_with_audio_clips(vec![
        audio_clip(1, 0, 0, 2000, 0, 1),
        audio_clip(2, 0, 1000, 2000, 0, 1),
    ]);
    assert!(p.overlap_layers, "layering is on by default");

    let r = AudioRenderer::new(&p);
    let mut f = direct(media);
    let mut out = vec![0f32; 3000 * CH];
    let stats = r.render_frames(&mut f, 0, &mut out);
    assert_eq!(stats.underruns, 0);

    let solo = project_with_audio_clips(vec![audio_clip(1, 0, 0, 2000, 0, 1)]);
    let mut dry = vec![0f32; 3000 * CH];
    no_underruns(AudioRenderer::new(&solo).render_frames(&mut direct(media), 0, &mut dry));

    for (f_idx, t) in [(1000usize, 0.0f64), (1500, 0.5), (1999, 0.999)] {
        let w_a = ((1.0 - t).max(0.0)).sqrt() as f32;
        let w_b = (t.max(0.0)).sqrt() as f32;
        // the early one plays its own material, the late one its own (source_in 0 for both)
        let expect = dry[f_idx * CH] * w_a + dry[(f_idx - 1000) * CH] * w_b;
        assert!(
            (out[f_idx * CH] - expect).abs() < 1e-4,
            "frame {f_idx}: equal-power mix, expected {expect}, got {}",
            out[f_idx * CH]
        );
    }

    // three clips at one point — the 1/N rule (v1)
    let p3 = project_with_audio_clips(vec![
        audio_clip(1, 0, 0, 3000, 0, 1),
        audio_clip(2, 0, 0, 3000, 0, 1),
        audio_clip(3, 0, 0, 3000, 0, 1),
    ]);
    let mut out3 = vec![0f32; 3000 * CH];
    no_underruns(AudioRenderer::new(&p3).render_frames(&mut direct(media), 0, &mut out3));
    let mut single = vec![0f32; 3000 * CH];
    AudioRenderer::new(&project_with_audio_clips(vec![audio_clip(1, 0, 0, 3000, 0, 1)]))
        .render_frames(&mut direct(media), 0, &mut single);
    for f_idx in [10usize, 1500, 2900] {
        // three sources with weight 1/3 give the original level
        assert!(
            (out3[f_idx * CH] - single[f_idx * CH]).abs() < 1e-4,
            "frame {f_idx}: 1/N rule, expected {}, got {}",
            single[f_idx * CH],
            out3[f_idx * CH]
        );
    }
}

/// With the overlap option switched off, overlaps remain illegal (R.24 mode).
#[test]
fn t162_overlap_rejected_when_option_off() {
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
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    p.clips = vec![audio_clip(1, 0, 0, 2000, 0, 1), audio_clip(2, 0, 1000, 2000, 0, 1)];
    p.overlap_layers = false;
    assert!(matches!(p.validate(), Err(ProjectError::ClipOverlap(..))));
    p.overlap_layers = true;
    p.validate().expect("with overlap — legal");
}
