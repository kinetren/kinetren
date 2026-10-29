//! t972 (plugin-api-stage1, D4; capability `plugins`): incompatible declarations are refused at
//! registration, by name, before any frame or sample — and a well-formed stateful audio plugin is
//! registered with its tail and look-ahead read by the same functions the audio processor uses.

use std::sync::Arc;

use kt_plugin_api::{
    Decl, Gpu, Manifest, Media, ParamKind, ParamSpec, ParamUnit, ParamValues, Plugin, Registry,
    Threads, VideoClass, lookahead_frames, tail_frames,
};

struct P(Manifest);

impl Plugin for P {
    fn manifest(&self) -> &Manifest {
        &self.0
    }
}

fn plugin(id: &'static str, media: Media, decl: Decl) -> Arc<dyn Plugin> {
    Arc::new(P(Manifest {
        id: id.into(),
        media,
        passport: vec![ParamSpec::num("time_ms", 1.0, 200.0, 20.0, 1.0, ParamUnit::Ms, ParamKind::Structural)],
        rows: Vec::new(),
        decl,
        ui: Default::default(),
    }))
}

fn video(class: VideoClass, gpu: Gpu) -> Decl {
    Decl { video_class: Some(class), gpu, ..Default::default() }
}

const TAP: &str = "fn tap_x(c: vec4<f32>) -> vec4<f32> { return c; }";

#[test]
fn t972_incompatible_declarations_are_refused_by_name() {
    let mut reg = Registry::new();
    reg.register(plugin("posterize", Media::VideoFx, video(VideoClass::PointWise, Gpu::Wgsl(TAP))))
        .expect("the first posterize registers");
    // the look-ahead of the last two cases: 48 frames per ms of the passport's `time_ms`
    fn ms_frames(p: &ParamValues) -> i64 {
        (p.get(0) * 48.0) as i64
    }
    fn tail_of(p: &ParamValues) -> i64 {
        (p.get(0) * 480.0) as i64
    }
    let cases: Vec<(Arc<dyn Plugin>, &str, &str)> = vec![
        (
            plugin("posterize", Media::VideoFx, video(VideoClass::PointWise, Gpu::Wgsl(TAP))),
            "posterize",
            "an id registered twice",
        ),
        (plugin("Posterize", Media::VideoFx, video(VideoClass::PointWise, Gpu::Wgsl(TAP))), "Posterize", "snake_case"),
        (
            plugin("v_state", Media::VideoFx, Decl { state: true, ..video(VideoClass::Barrier, Gpu::Wgsl(TAP)) }),
            "v_state",
            "declares state",
        ),
        (
            plugin("v_tail", Media::VideoFx, Decl { tail: Some(tail_of), ..video(VideoClass::Barrier, Gpu::Wgsl(TAP)) }),
            "v_tail",
            "declares a tail",
        ),
        (
            plugin(
                "v_look",
                Media::VideoFx,
                Decl { lookahead: Some(ms_frames), ..video(VideoClass::Barrier, Gpu::Wgsl(TAP)) },
            ),
            "v_look",
            "declares a look-ahead",
        ),
        (
            plugin("single", Media::AudioFx, Decl { threads: Threads::Single, ..Default::default() }),
            "single",
            "must not be called concurrently",
        ),
        (
            // 200 ms at the top of the range = 9600 frames ≥ 4800
            plugin("long_look", Media::AudioFx, Decl { state: true, lookahead: Some(ms_frames), ..Default::default() }),
            "long_look",
            "reaches the pre-roll (4800)",
        ),
        (plugin("no_gpu", Media::VideoFx, video(VideoClass::Barrier, Gpu::None)), "no_gpu", "without a GPU kernel"),
        (
            plugin("geo_wgsl", Media::VideoFx, video(VideoClass::Geometry, Gpu::Wgsl(TAP))),
            "geo_wgsl",
            "offers WGSL instead of the host's primitives",
        ),
    ];
    for (p, id, why) in cases {
        let err = reg.register(p).expect_err(&format!("{id} must be refused ({why})"));
        assert_eq!(err.id, id, "the refusal names the plugin");
        assert!(err.reason.contains(why), "{id}: the refusal does not name the declaration: {err}");
    }
    assert_eq!(reg.ids(), vec!["posterize"], "a refused plugin became visible");

    // a well-formed stateful audio plugin: state, a tail and a look-ahead under the pre-roll
    fn short_look(p: &ParamValues) -> i64 {
        (p.get(0) * 20.0) as i64 // 200 ms → 4000 frames < 4800
    }
    let ok = plugin(
        "delay_x",
        Media::AudioFx,
        Decl { state: true, tail: Some(tail_of), lookahead: Some(short_look), ..Default::default() },
    );
    reg.register(ok).expect("a stateful audio plugin with a tail and a short look-ahead is registered");
    let m = reg.get("delay_x").unwrap().manifest().clone();
    let at = ParamValues::defaults(&m.passport);
    assert_eq!(tail_frames(&m, &at), 9600, "the tail is read from the manifest");
    assert_eq!(lookahead_frames(&m, &at), 400, "the look-ahead is read from the manifest");
}
