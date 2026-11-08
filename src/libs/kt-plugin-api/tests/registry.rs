//! t972 (plugin-api-stage1, D4; capability `plugins`): incompatible declarations are refused at
//! registration, by name, before any frame or sample — and a well-formed stateful audio plugin is
//! registered with its tail and look-ahead read by the same functions the audio processor uses.

use std::sync::Arc;

use kt_plugin_api::{Family, 
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
    fn ms_frames(p: &ParamValues, _c: &dyn kt_plugin_api::SlotCurves) -> i64 {
        (p.get(0) * 48.0) as i64
    }
    fn tail_of(p: &ParamValues, _c: &dyn kt_plugin_api::SlotCurves) -> i64 {
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
    fn short_look(p: &ParamValues, _c: &dyn kt_plugin_api::SlotCurves) -> i64 {
        (p.get(0) * 20.0) as i64 // 200 ms → 4000 frames < 4800
    }
    let ok = plugin(
        "delay_x",
        Media::AudioFx,
        Decl { state: true, tail: Some(tail_of), lookahead: Some(short_look), ..Default::default() },
    );
    reg.register(ok).expect("a stateful audio plugin with a tail and a short look-ahead is registered");
    let m = reg.get(Family::Fx, "delay_x").unwrap().manifest().clone();
    let at = ParamValues::defaults(&m.passport);
    assert_eq!(tail_frames(&m, &at), 9600, "the tail is read from the manifest");
    assert_eq!(lookahead_frames(&m, &at), 400, "the look-ahead is read from the manifest");
}

/// t1162 (generators-placement-and-audio, D3): a generator kind with both a synthesizer and a host
/// tone gives two answers to "what does it sound like" — refused by id; a synthesizer alone passes.
#[test]
fn t1162_synth_and_tone_together_are_refused() {
    use kt_plugin_api::generator::{PathTok, sound};
    fn hum(_: &[f64], _: i64, out: &mut [f32]) {
        out.fill(0.25);
    }
    fn level(_: &[f64]) -> Option<f64> {
        Some(-20.0)
    }
    const NO_PATH: &[PathTok] = &[];
    let mut reg = Registry::new();
    let both = sound("hum_tone", "genkind.hum", "genkind.group_audio", vec![], NO_PATH, hum).with_tone(level, None);
    let err = reg.register(Arc::new(both)).expect_err("a synth with a tone must be refused");
    assert_eq!(err.id, "hum_tone", "the refusal names the plugin");
    assert!(err.reason.contains("both a synthesizer and a tone"), "the refusal does not name the conflict: {err}");
    reg.register(Arc::new(sound("hum", "genkind.hum", "genkind.group_audio", vec![], NO_PATH, hum)))
        .expect("a synth-only sound kind is registered");
    let g = reg.get(Family::Generator, "hum").unwrap().generator().expect("a generator kernel");
    let synth = g.synth().expect("the kernel reports its synthesizer");
    let mut buf = [0f32; 8];
    synth(&[], 0, &mut buf);
    assert_eq!(buf, [0.25; 8]);
    assert!(g.decl().blank, "a sound kind's card frame is empty by meaning");
}

/// A stream kernel that renders nothing (only the declarations are under test).
struct S(Manifest, bool);

impl kt_plugin_api::stream::StreamKernel for S {
    fn lines(&self, _p: &[f64]) -> u32 {
        144
    }
    fn before(&self, _p: &[f64]) -> u32 {
        0
    }
    fn second(&self, _p: &[f64]) -> kt_plugin_api::stream::SecondFrom {
        kt_plugin_api::stream::SecondFrom::None
    }
    fn needs(
        &self,
        _p: &[f64],
        _l: &dyn kt_plugin_api::stream::StreamLayout,
        _n: i64,
        _m: &kt_plugin_api::stream::StreamMemo,
    ) -> Vec<(kt_plugin_api::stream::Side, i64)> {
        vec![]
    }
    fn render(
        &self,
        _p: &[f64],
        _i: &dyn kt_plugin_api::stream::StreamInput,
        _n: i64,
        _m: &kt_plugin_api::stream::StreamMemo,
        _o: &mut Vec<u8>,
    ) -> Option<kt_plugin_api::stream::StreamOut> {
        None
    }
}

impl Plugin for S {
    fn manifest(&self) -> &Manifest {
        &self.0
    }
    fn stream(&self) -> Option<&dyn kt_plugin_api::stream::StreamKernel> {
        self.1.then_some(self as &dyn kt_plugin_api::stream::StreamKernel)
    }
}

fn stream_plugin(id: &'static str, decl: Decl, kernel: bool) -> Arc<dyn Plugin> {
    let m = Manifest { id: id.into(), media: Media::VideoFx, passport: vec![], rows: Vec::new(), decl, ui: Default::default() };
    Arc::new(S(m, kernel))
}

/// t1240 (datamosh-codec-path, D2): a stream plugin is a source stage with a kernel and no GPU
/// side; the registry refuses each malformed form by name and takes the well-formed one.
#[test]
fn t1240_stream_declarations_are_refused_by_name() {
    let mut reg = Registry::new();
    let cases: Vec<(Arc<dyn Plugin>, &str)> = vec![
        (stream_plugin("s_no_kernel", video(VideoClass::Stream, Gpu::None), false), "without a stream kernel"),
        (stream_plugin("s_wgsl", video(VideoClass::Stream, Gpu::Wgsl(TAP)), true), "offers a GPU side"),
        (stream_plugin("s_barrier", video(VideoClass::Barrier, Gpu::Barrier), true), "not of the stream class"),
        (stream_plugin("s_tap", video(VideoClass::PointWise, Gpu::Wgsl(TAP)), true), "not of the stream class"),
        (
            stream_plugin("s_state", Decl { state: true, ..video(VideoClass::Stream, Gpu::None) }, true),
            "declares state",
        ),
    ];
    for (p, why) in cases {
        let id = p.manifest().id.to_string();
        let e = reg.register(p).expect_err(&id);
        assert_eq!(e.id, id);
        assert!(e.reason.contains(why), "{id}: {}", e.reason);
    }
    reg.register(stream_plugin("s_ok", video(VideoClass::Stream, Gpu::None), true)).expect("a well-formed stream plugin");
    assert!(reg.get(Family::Fx, "s_ok").is_some());
}

/// t1645 (gpu-render-hardening, D7): a shader kind with no compute device gives a transparent frame
/// of its size, and no panic. No device is installed in this test.
#[test]
fn t1645_a_shader_generator_with_no_device_is_transparent() {
    let f = kt_plugin_api::generator::shader_frame("test", "fn pixel(x: u32, y: u32) -> u32 { return 0xffffffffu; }", 4, 3, [0, 0], [[0.0; 4]; 4]);
    assert_eq!(f.len(), 48);
    assert!(f.iter().all(|&b| b == 0), "no device, no picture");
}
