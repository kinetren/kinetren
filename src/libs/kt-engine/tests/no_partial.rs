//! no-partial-picture (kkt-26.11.14 M1, capabilities `video-plan`, `ui-timeline`): a picture or a level
//! that is not complete is never shown, and the work of making one is not spent. t2531 — the plan
//! knows its starvation before an executor touches the device; t2532 — the meter under the cursor gives
//! no level for a mix whose bursts are not cached yet (it read them as silence).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use kt_cache::{BurstCache, CacheConfig};
use kt_engine::{Player, VideoRenderer};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{AudioBurst, MediaError, MediaInfo, MediaSource, Quality, VideoFrame};
use kt_model::demo::{DemoSources, demo_project};
use kt_model::*;
use kt_time::Tick;

const Q: Quality = Quality { width: 320, height: 180 };

/// t2531: `FramePlan::starved` is true exactly while a layer of the plan lacks its frame: a cold
/// cache, a cache holding all but one frame, and a warm one.
#[test]
fn t2531_a_plan_knows_its_starvation() {
    kt_plugins_builtin::install();
    let p = demo_project(DemoSources::Synth);
    let video = VideoRenderer::new(&p);
    // a moment of two layers at least (a transition or a stack)
    let t = (0..400i64)
        .map(|i| Tick(i * 9_000))
        .find(|t| video.layers_at(*t, Q).len() >= 2)
        .expect("scene guard: a moment of two layers");
    let layers = video.layers_at(t, Q);
    let mut sources: std::collections::HashMap<_, Box<dyn MediaSource>> = p
        .media
        .iter()
        .map(|m| (m.id, kt_media::factory::open_pseudo(&m.path).unwrap().unwrap()))
        .collect();
    for (a, st) in kt_engine::gen_sources(&p) {
        sources.insert(a, kt_media::factory::open_gen(&st));
    }
    let cache = BurstCache::new(CacheConfig { max_audio_bytes: 1, max_video_bytes: 256 << 20 });
    assert!(video.frame_plan(&cache, t, Q).starved(), "a cold cache");
    let (last, rest) = layers.split_last().unwrap();
    let put = |(media, pts, dq): &(MediaId, Tick, Quality), sources: &mut std::collections::HashMap<_, Box<dyn MediaSource>>| {
        let f = sources.get_mut(media).expect("a source").read_video_frame(*pts, Some(*dq)).unwrap();
        cache.insert_video(kt_engine::source_id(*media), Some(*dq), kt_engine::video_cache_idx(*pts), f.into());
    };
    for l in rest {
        put(l, &mut sources);
    }
    assert!(video.frame_plan(&cache, t, Q).starved(), "one layer short");
    put(last, &mut sources);
    assert!(!video.frame_plan(&cache, t, Q).starved(), "every layer is there");
}

/// An audio source whose reads wait until the test lets them through.
struct Held {
    inner: SyntheticSource,
    open: Arc<AtomicBool>,
}

impl MediaSource for Held {
    fn info(&self) -> &MediaInfo {
        self.inner.info()
    }
    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        let deadline = kt_time::test_deadline();
        while !self.open.load(Ordering::Acquire) {
            assert!(std::time::Instant::now() < deadline, "the test never let the read through");
            std::thread::sleep(Duration::from_millis(1));
        }
        self.inner.read_audio_burst(idx)
    }
    fn read_video_frame(&mut self, p: Tick, q: Option<Quality>) -> Result<VideoFrame, MediaError> {
        self.inner.read_video_frame(p, q)
    }
    fn keyframe_before(&self, p: Tick) -> Option<Tick> {
        self.inner.keyframe_before(p)
    }
}

fn tone_project() -> Project {
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
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick(90_000 * 4),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

/// t2532: while the bursts under the cursor are not cached, the meter's probe gives no level (it gave
/// the level of the part cached, the rest read as silence: zero for a loud tone); once they are, the
/// tone's level.
#[test]
fn t2532_a_partial_mix_gives_no_level() {
    let open = Arc::new(AtomicBool::new(false));
    let src = Held { inner: SyntheticSource::new(Tick(90_000 * 8), AudioPattern::Sine { hz: 440.0 }), open: open.clone() };
    let (player, _consumer) = Player::new(tone_project(), vec![(MediaId(1), Box::new(src) as Box<dyn MediaSource>)]);
    let t = Tick(90_000);
    assert_eq!(player.sample_levels_at(t), None, "no burst is cached: the level is not known");
    open.store(true, Ordering::Release);
    let deadline = kt_time::test_deadline();
    let level = loop {
        if let Some(l) = player.sample_levels_at(t) {
            break l;
        }
        assert!(std::time::Instant::now() < deadline, "the bursts under the cursor never came");
        std::thread::sleep(Duration::from_millis(5));
    };
    assert!(level.0 > 0.1 && level.1 > 0.1, "the tone's level once cached: {level:?}");
}
