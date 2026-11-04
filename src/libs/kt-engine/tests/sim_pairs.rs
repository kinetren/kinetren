//! t1467 (sim-audio-pairs; capabilities `audio-fx`, `plugins`): the engine seats the audio half of a
//! simulated device on the TIMELINE. A VHS deck with a clogged head A loses the sound of field A —
//! the first 20 ms of every PAL tape frame counted from the timeline's zero — on an event that does
//! not start on a field boundary; the slot switched off gives the dry bytes; two renders agree.

use kt_engine::{AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const FS: f64 = 48_000.0;
/// 3.01 s: half a field off the tape's grid — a clip-local seat would swap the fields.
const START: i64 = 144_480;
const DUR: i64 = 48_000;

fn project(fx: Vec<FxInstance>, bypass: Vec<bool>) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 25,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:sine"));
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
        start: Tick::from_audio_frames(START),
        duration: Tick::from_audio_frames(DUR),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx,
        fx_bypass: bypass,
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

fn render(p: &Project) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let mut f = DirectFetcher::new(None);
    f.add_source(MediaId(1), Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Sine { hz: 1000.0 })));
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut out = vec![0f32; DUR as usize * CH];
    let stats = r.render_frames_with(&mut f, &mut store, START, &mut out);
    assert_eq!(stats.underruns, 0);
    out
}

#[test]
fn t1467_the_engine_seats_the_device_on_the_timeline() {
    kt_plugins_builtin::install();
    let dry = render(&project(vec![], vec![]));
    let mut tape = FxInstance::plugin("vhs_tape_sound").expect("vhs_tape_sound");
    tape.set_param("rec_a_spacing", 3.0);
    let wet = render(&project(vec![tape.clone()], vec![]));
    // the error while each head reads, on the TIMELINE's tape grid (PAL: 50 fields/s from zero; the
    // next head takes over 6.5 lines before a field's 288 active lines end — 18.016 ms in), 3 ms
    // after each switch left out (the compander's recovery)
    let t_sw = (288.0 - 6.5) * 64e-6;
    let mut e = [0f64; 2];
    let mut k = [0usize; 2];
    for i in (DUR as usize / 4)..DUR as usize {
        let t = (START + i as i64) as f64 / FS;
        let field = ((t * 50.0).floor() as i64 % 2) as usize;
        let tau = t - (t * 50.0).floor() / 50.0;
        let (head, since) = if tau >= t_sw { (1 - field, tau - t_sw) } else { (field, tau + 0.02 - t_sw) };
        if since < 0.003 {
            continue;
        }
        let d = (wet[i * CH] - dry[i * CH]) as f64;
        e[head] += d * d;
        k[head] += 1;
    }
    let rms = [(e[0] / k[0] as f64).sqrt(), (e[1] / k[1] as f64).sqrt()];
    eprintln!("head A {:.4}, head B {:.4}", rms[0], rms[1]);
    assert!(rms[0] > 8.0 * rms[1].max(1e-4), "head A's reading on the timeline's tape is not the lost one: {rms:?}");
    // the slot switched off: the dry bytes
    let off = render(&project(vec![tape.clone()], vec![true]));
    assert!(off == dry, "a switched-off sound slot changed the bytes");
    // determinism
    assert!(render(&project(vec![tape], vec![])) == wet, "two renders differ");
}
