//! t973b (plugin-api-stage1, D3; capability `plugins`): an effect whose plugin this build lacks is
//! passed through by the engine and named by the model — a shared project is never hostage to the
//! plugin set of the machine that opens it.

use kt_engine::VideoRenderer;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::Tick;

const Q: Quality = Quality { width: 64, height: 36 };

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    }
}

fn clip(id: u64, lane: u32, media: u64) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

/// Two layers on ONE source, but with a different `source_in`: the synthetic frame is painted
/// by the TICK NUMBER, so the upper layer (track 0) reads frame 3000 (r = 184),
/// the lower one — frame 0 (r = 0).
/// So by a single pixel one can see which layer got into the composite.
const TOP_IN: i64 = 3_000;

fn project() -> Project {
    let mut p = Project::new(settings());
    p.media.push(MediaRef::new(MediaId(1), "synth:silence"));
    let mut top = clip(1, 0, 1);
    top.source_in = Tick(TOP_IN);
    p.clips = vec![top, clip(2, 1, 1)];
    p.normalize();
    p.validate().unwrap();
    p
}

fn sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    vec![(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence))
            as Box<dyn MediaSource>,
    )]
}

fn render(p: &Project) -> (Vec<u8>, bool) {
    let mut facts = kt_engine::SourceFacts::default();
    for (id, s) in sources() {
        facts.observe(id, s.info());
    }
    let video = VideoRenderer::new_with_facts(p, &facts);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources().into_iter().collect();
    // the REQUEST — what the engine asks for; we read exactly it
    for (media, pts, dq) in video.layers_at(Tick(0), Q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(pts),
            frame.into(),
        );
    }
    kt_gpu::testkit::compose(&video, &cache, Tick(0), Q)
}


#[test]
fn t973b_a_missing_plugin_is_passed_through_and_named() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    kt_plugins_builtin::install();
    let wobble: FxInstance =
        serde_json::from_str(r#"{"kind":"wobble_x","amp":0.25,"freq":3}"#).expect("an unknown kind reads");
    let with = |fx: Vec<FxInstance>| {
        let mut p = project();
        let n = fx.len();
        p.clips[0].fx = fx;
        p.clips[0].fx_auto = vec![Vec::new(); n];
        p.validate().expect("a chain with a missing plugin validates");
        p
    };
    let missing = with(vec![FxInstance::blur(2), wobble, FxInstance::glow(0.5)]);
    let without = with(vec![FxInstance::blur(2), FxInstance::glow(0.5)]);
    let (a, ca) = render(&missing);
    let (b, cb) = render(&without);
    assert!(ca && cb, "the composite is not complete");
    assert_eq!(a, b, "the missing plugin changed the frame: it must pass it through");
    assert_eq!(
        missing.missing_plugins(),
        vec![("wobble_x".to_string(), ClipId(1))],
        "the missing id is named once, with its event"
    );
    assert!(without.missing_plugins().is_empty());
}
