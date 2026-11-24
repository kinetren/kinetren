//! loudness-norms (audio-render): the program stage in the export and in the match jobs. A test binary
//! of its own.

use kt_engine::export::{ExportStats, render_raw_audio_range};
use kt_engine::program;
use kt_media::MediaSource;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

const S: i64 = 90_000;

fn clip(id: u64, lane: u32, start: i64, dur: i64, media: u64, gain_db: f32) -> ClipEvent {
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
        kind: ClipKind::Audio,
        lane,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
        fx_in: Vec::new(),
    }
}

/// Media 1 — a 1 kHz tone of amplitude 0.5; media 2 — the same at 440 Hz; media 3 — silence.
fn project(clips: Vec<ClipEvent>) -> Project {
    kt_plugins_builtin::install();
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    for m in 1..=3 {
        p.media.push(MediaRef::new(MediaId(m), format!("synth:{m}")));
    }
    p.clips = clips;
    p.ensure_tracks(LaneKind::Audio, 3);
    p.ensure_tracks(LaneKind::Video, 1);
    p.normalize();
    p.validate().unwrap();
    p
}

fn sources(_: &Project) -> Vec<(MediaId, Box<dyn MediaSource>)> {
    [(1, AudioPattern::Sine { hz: 1000.0 }), (2, AudioPattern::Sine { hz: 440.0 }), (3, AudioPattern::Silence)]
        .into_iter()
        .map(|(m, a)| (MediaId(m), Box::new(SyntheticSource::new(Tick(S * 60), a)) as Box<dyn MediaSource>))
        .collect()
}

fn off() -> Arc<AtomicBool> {
    Arc::new(AtomicBool::new(false))
}

fn render(p: &Project, with: bool, range: Option<(Tick, Tick)>) -> (Vec<f32>, ExportStats) {
    let mut out = Vec::new();
    let stage = with.then(|| program::params(p)).flatten();
    let s = render_raw_audio_range(p, sources(p), range, stage, &off(), |_| {}, |c| {
        out.extend_from_slice(c);
        Ok(())
    })
    .unwrap();
    (out, s)
}

fn matched(p: &Project) -> Project {
    let stage = program::match_program(p, &mut |q| sources(q), &off(), &mut |_, _| {}).unwrap();
    let mut q = p.clone();
    q.program = stage;
    q.validate().unwrap();
    q
}

fn lufs(s: &ExportStats) -> f64 {
    s.loudness.unwrap().lufs.unwrap()
}

/// t3919 (the export's half): a mix with a wide crest at −19.5 LUFS is matched to −14 under −1 dBTP;
/// the export measures it; a louder event is another sound and a moved title is not.
#[test]
fn t3919_match_the_program_then_edit() {
    // a steady bed and short loud hits over it
    let mut clips = vec![clip(1, 0, 0, 20 * S, 1, -14.0)];
    for k in 0..10 {
        clips.push(clip(10 + k, 1, k as i64 * 2 * S, S / 250, 2, 6.0));
    }
    let mut p = project(clips);
    p.loudness = LoudnessTarget::of(LoudnessStandard::Streaming);
    let (_, plain) = render(&p, true, None);
    let (before, peak) = (lufs(&plain), plain.loudness.unwrap().dbtp);
    assert!((-21.0..-18.0).contains(&before) && peak > -0.5, "the scene: {before:.2} LUFS, {peak:.2} dBTP");
    assert_eq!(plain.program, None, "no stage, no report of one");

    let q = matched(&p);
    let stage = q.program.unwrap();
    let m = stage.measured.unwrap();
    assert!((m.lufs - before).abs() < 1e-9 && (m.dbtp - peak).abs() < 1e-9, "the mix before the stage: {m:?}");
    assert!(stage.gain_db >= -14.0 - before - 0.01, "the limiter takes loudness away, the gain makes up for it: {stage:?}");
    let (y, after) = render(&q, true, None);
    assert!((lufs(&after) + 14.0).abs() <= 0.2, "the export measures {:.2} LUFS", lufs(&after));
    assert!(after.loudness.unwrap().dbtp <= -1.0, "true peak {:.2}", after.loudness.unwrap().dbtp);
    let rep = after.program.unwrap();
    assert!(rep.gain_db == stage.gain_db && rep.took_lu > 0.0 && rep.deepest_db > 3.0, "{rep:?}");
    assert_eq!(y.len() / AUDIO_CHANNELS, 20 * 48_000, "the frame count does not change");

    assert_eq!(program::sound(&q), m.sound, "nothing changed since measured");
    let mut louder = q.clone();
    louder.clips[0].gain_db += 6.0;
    assert_ne!(program::sound(&louder), m.sound, "a louder event is another sound");
    let mut title = q.clone();
    let mut t = clip(99, 0, 0, S, 1, 0.0);
    t.kind = ClipKind::Video;
    title.clips.push(t.clone());
    assert_eq!(program::sound(&title), m.sound, "a picture is not heard");
    title.clips.last_mut().unwrap().start = Tick(5 * S);
    assert_eq!(program::sound(&title), m.sound);
    let mut faded = q.clone();
    faded.tracks.iter_mut().find(|t| t.kind == LaneKind::Audio).unwrap().gain_db = -3.0;
    assert_ne!(program::sound(&faded), m.sound, "a track's level is heard");
    // the number survives the file
    let back = project_file::from_text(&project_file::to_text(&q, None)).unwrap().project;
    assert_eq!((back.program, program::sound(&back)), (q.program, m.sound));

    // a mix too dynamic for its target: the last gain tried, and what it reached
    let mut hits = project((0..10).map(|k| clip(1 + k, 0, k as i64 * 2 * S, S / 20, 1, 6.0)).collect());
    hits.loudness = LoudnessTarget { standard: LoudnessStandard::Custom, target_lufs: -5.0, ceiling_dbtp: -1.0 };
    let g = program::match_program(&hits, &mut |q| sources(q), &off(), &mut |_, _| {}).unwrap().unwrap();
    assert!(g.measured.unwrap().reached < -5.5, "the limiter holds it back: {g:?}");
    // silence has no loudness to match
    let silent = project(vec![clip(1, 0, 0, 4 * S, 3, 0.0)]);
    assert_eq!(program::match_program(&silent, &mut |q| sources(q), &off(), &mut |_, _| {}).unwrap(), None);
    let cancelled = Arc::new(AtomicBool::new(true));
    assert!(program::match_program(&p, &mut |q| sources(q), &cancelled, &mut |_, _| {}).is_err());
}

/// t4200: with no stage the bytes are the mix's; a stage at 0 dB over a mix under its inner ceiling gives
/// the same samples, whatever the range starts at.
#[test]
fn t4200_no_stage_no_change() {
    let mut p = project(vec![clip(1, 0, 0, 6 * S, 1, -3.0), clip(2, 1, 2 * S, 3 * S, 2, -6.0)]);
    let (mix, _) = render(&p, false, None);
    let (same, s) = render(&p, true, None);
    assert!(mix == same && s.program.is_none(), "no stage changes the sound");
    p.program = Some(ProgramStage::default());
    let (thru, s) = render(&p, true, None);
    assert_eq!(mix.len(), thru.len());
    let bad = mix.iter().zip(&thru).filter(|(a, b)| a.to_bits() != b.to_bits()).count();
    assert_eq!(bad, 0, "{bad} samples changed under the ceiling");
    assert_eq!(s.program.unwrap().deepest_db, 0.0);
    let r = Some((Tick(S + 12_345), Tick(3 * S + 7)));
    assert!(render(&p, false, r).0 == render(&p, true, r).0, "a range reads the mix it stands in");
}

/// t3920: five dialogue lines of different loudness and two short ones are matched to −14, a music event
/// to −22: each long line measures it, the short ones take one correction of their own material, an
/// event of silence is left and named; the commands are one undo step.
#[test]
fn t3920_clips_are_matched_short_ones_together() {
    let long = [-21.0f32, -13.0, -9.0, -17.0, -7.0];
    let mut clips: Vec<ClipEvent> =
        (0..5).map(|k| clip(1 + k, 0, k as i64 * 5 * S, 4 * S, 1, long[k as usize])).collect();
    clips.push(clip(6, 1, 0, S, 1, -2.0));
    clips.push(clip(7, 1, 2 * S, S, 1, -12.0));
    clips.push(clip(8, 2, 0, 8 * S, 2, -4.0));
    clips.push(clip(9, 2, 10 * S, 4 * S, 3, 0.0));
    let p = project(clips);
    let lines: Vec<ClipId> = [1, 2, 3, 4, 5, 6, 7, 9].map(ClipId).to_vec();
    assert_eq!(program::match_groups(&p, &lines).len(), 7, "six long materials and one of the two short lines");
    let go = |ids: &[ClipId], target: f64| {
        program::match_clips(&p, ids, target, &mut |q| sources(q), &off(), &mut |_, _| {}).unwrap()
    };
    let m = go(&lines, -14.0);
    assert_eq!(m.silent, vec![ClipId(9)]);
    let music = go(&[ClipId(8)], -22.0);
    let mut model = TimelineModel::new(p.clone());
    let cmds: Vec<Command> =
        m.gains.iter().chain(&music.gains).map(|(id, g)| Command::SetGain { id: *id, gain_db: *g }).collect();
    model.apply_batch(cmds).unwrap();
    let alone = |id: u64| {
        let mut q = model.document();
        q.clips.retain(|c| c.id == ClipId(id));
        let c = &q.clips[0];
        let r = (c.start, c.start + c.duration);
        lufs(&render(&q, false, Some(r)).1)
    };
    for id in 1..=5 {
        assert!((alone(id) + 14.0).abs() <= 0.3, "line {id} measures {:.2}", alone(id));
    }
    assert!((alone(8) + 22.0).abs() <= 0.3, "the music measures {:.2}", alone(8));
    let gain = |id: u64| model.document().clips.iter().find(|c| c.id == ClipId(id)).unwrap().gain_db;
    let (a, b) = (gain(6) + 2.0, gain(7) + 12.0);
    assert!((a - b).abs() < 0.02, "the two short lines got different corrections: {a} and {b}");
    assert!((alone(6) + 14.0).abs() > 1.0, "the short line was matched on its own");
    // an event has at most +6.02 dB: what it cannot reach is said
    let quiet = project(vec![clip(1, 0, 0, 4 * S, 1, -3.0)]);
    let over = program::match_clips(&quiet, &[ClipId(1)], 4.0, &mut |q| sources(q), &off(), &mut |_, _| {}).unwrap();
    assert!(over.gains[0].1 <= 6.0206 && over.held == vec![ClipId(1)], "{over:?}");
    model.undo().unwrap();
    assert_eq!(model.document().clips, p.clips, "one Undo restores every level");
}
