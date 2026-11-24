//! The program stage in the engine (loudness-norms D3): what the document's stage asks of the kernel
//! (`kt_fx::audio::program`), its life in the player's render loop, and the fingerprint of what is heard.
//!
//! It stands at the OUTPUT, after the kept mix: its gain never enters `scene_hash`, so a change of it
//! drops no kept burst. It is fed the mix `LOOKAHEAD` frames ahead.

use crate::export::render_raw_audio_range;
use kt_fx::audio::loudness::{Loudness, LoudnessMeter};
use kt_fx::audio::program::{LOOKAHEAD, Params, Stage};
use kt_media::MediaSource;
use kt_model::{ClipEvent, ClipId, ClipKind, MediaId, PROGRAM_GAIN_DB, ProgramMeasured, ProgramStage, Project};
use kt_time::{AUDIO_CHANNELS, Tick};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// The stage's parameters of `p`; `None` — no stage: the output is the mix, byte for byte.
pub fn params(p: &Project) -> Option<Params> {
    p.program.map(|g| Params { gain_db: g.gain_db, ceiling_dbtp: p.loudness.ceiling_dbtp })
}

/// The stage in a play: made when the document gets one, kept as a plain delay when it is taken away
/// while it sounds (until the next seek: the head does not jump by the look-ahead).
#[derive(Default)]
pub(crate) struct PlayStage {
    stage: Option<Stage>,
    /// What the document asks; `None` with a `stage` — leaving.
    asked: Option<Params>,
    primed: bool,
}

impl PlayStage {
    pub fn new(p: Option<Params>) -> PlayStage {
        PlayStage { stage: p.map(Stage::new), asked: p, primed: false }
    }

    /// A new build's parameters.
    pub fn ask(&mut self, p: Option<Params>) {
        if p == self.asked {
            return;
        }
        self.asked = p;
        match (&mut self.stage, p) {
            (Some(s), p) => s.set(p.unwrap_or(Params::PASS)),
            (None, Some(p)) => {
                self.stage = Some(Stage::new(p));
                self.primed = false;
            }
            (None, None) => {}
        }
    }

    /// A seek, a change of direction, a loop armed or disarmed: the signal starts anew.
    pub fn reset(&mut self) {
        self.primed = false;
        match (self.asked, &mut self.stage) {
            (None, s) => *s = None,
            (Some(_), Some(s)) => s.reset(),
            (Some(_), None) => {}
        }
    }

    /// Frames the mix is read ahead of the head in a forward play.
    pub fn lead(&self) -> i64 {
        if self.stage.is_some() { LOOKAHEAD as i64 } else { 0 }
    }

    pub fn wants_prime(&self) -> bool {
        self.stage.is_some() && !self.primed
    }

    pub fn prime(&mut self, ahead: &[f32; LOOKAHEAD * AUDIO_CHANNELS]) {
        if let Some(s) = &mut self.stage {
            s.prime(ahead);
        }
        self.primed = true;
    }

    /// The forward chunk, in place; the deepest reduction in hundredths of a decibel.
    pub fn process(&mut self, chunk: &mut [f32]) -> u32 {
        let Some(s) = &mut self.stage else { return 0 };
        s.process(chunk);
        (s.take_deepest_db() * 100.0) as u32
    }

    /// Without the look-ahead (reverse): the gain and a clamp.
    pub fn clamp(&self, chunk: &mut [f32]) {
        if let Some(p) = &self.asked {
            p.clamp(chunk);
        }
    }
}

/// What is heard, as one number: the fingerprint of `kt_model::sound_bytes`.
pub fn sound(p: &Project) -> u64 {
    xxhash_rust::xxh3::xxh3_64(&kt_model::sound_bytes(p))
}

// ---- the measuring jobs (loudness-norms D2, D4) -----------------------------------------------

type Sources = Vec<(MediaId, Box<dyn MediaSource>)>;

/// A stage over a stream that is fed as it comes: its output into a meter, the look-ahead dropped.
struct Through {
    stage: Stage,
    skip: usize,
    meter: LoudnessMeter,
    buf: Vec<f32>,
}

impl Through {
    fn new(p: Params) -> Through {
        Through { stage: Stage::new(p), skip: LOOKAHEAD * AUDIO_CHANNELS, meter: LoudnessMeter::new(), buf: Vec::new() }
    }

    fn feed(&mut self, x: &[f32]) {
        self.buf.clear();
        self.buf.extend_from_slice(x);
        self.stage.process(&mut self.buf);
        let drop = self.skip.min(self.buf.len());
        self.skip -= drop;
        self.meter.feed(&self.buf[drop..]);
    }

    fn finish(mut self) -> Loudness {
        self.feed(&[0.0; LOOKAHEAD * AUDIO_CHANNELS]);
        self.meter.finish()
    }
}

/// How far past the plain gain "Match the program" looks for what the limiter takes: copies 1 dB apart.
const TRIED: usize = 4;

/// "Match the program": the gain that brings the document's mix to its target after the limiter, and
/// what was measured. `None` — the mix is silent. The first pass renders the mix; a second one only
/// where the limiter acts, through `TRIED` copies of the stage a decibel apart. `progress` gets the
/// frames done of both passes' total as (done, total); `sources` opens the document's files.
pub fn match_program(
    p: &Project,
    sources: &mut dyn FnMut(&Project) -> Sources,
    cancel: &Arc<AtomicBool>,
    progress: &mut dyn FnMut(u64, u64),
) -> std::io::Result<Option<ProgramStage>> {
    let len = crate::export::project_range(p).to_audio_frames().max(1) as u64;
    let mix = render_raw_audio_range(p, sources(p), None, None, cancel, |n| progress(n, 2 * len), |_| Ok(()))?
        .loudness
        .expect("an audio pass measures");
    let Some(lufs) = mix.lufs else { return Ok(None) };
    let target = p.loudness.target_lufs;
    let clamp = |g: f64| g.clamp(*PROGRAM_GAIN_DB.start(), *PROGRAM_GAIN_DB.end());
    let g0 = clamp(target - lufs);
    let (gain_db, reached) = if mix.dbtp + g0 <= p.loudness.ceiling_dbtp - kt_fx::audio::program::INNER_DB {
        (g0, lufs + g0)
    } else {
        let tries: Vec<f64> = (0..TRIED).map(|k| clamp(g0 + k as f64)).collect();
        let mut through: Vec<Through> =
            tries.iter().map(|g| Through::new(Params { gain_db: *g, ceiling_dbtp: p.loudness.ceiling_dbtp })).collect();
        render_raw_audio_range(p, sources(p), None, None, cancel, |n| progress(len + n, 2 * len), |c| {
            for t in &mut through {
                t.feed(c);
            }
            Ok(())
        })?;
        let got: Vec<(f64, f64)> =
            tries.into_iter().zip(through).map(|(g, t)| (g, t.finish().lufs.unwrap_or(lufs))).collect();
        toward(&got, target)
    };
    // two decimals: a number a person can read, finer than any meter
    let gain_db = (gain_db * 100.0).round() / 100.0;
    let measured = ProgramMeasured {
        lufs,
        dbtp: mix.dbtp,
        reached,
        target,
        ceiling: p.loudness.ceiling_dbtp,
        gain_db,
        sound: sound(p),
    };
    Ok(Some(ProgramStage { gain_db, measured: Some(measured) }))
}

/// The gain whose loudness is `target`, between the tried ones; past them all — the last, and what it
/// reached.
fn toward(got: &[(f64, f64)], target: f64) -> (f64, f64) {
    let Some(k) = got.iter().position(|(_, l)| *l >= target) else { return got[got.len() - 1] };
    if k == 0 {
        return got[0];
    }
    let ((g0, l0), (g1, l1)) = (got[k - 1], got[k]);
    let t = if l1 > l0 { (target - l0) / (l1 - l0) } else { 0.0 };
    (g0 + (g1 - g0) * t, target)
}

/// What "Match loudness" found: the new event levels, and the events it could not match.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Matched {
    pub gains: Vec<(ClipId, f32)>,
    pub silent: Vec<ClipId>,
    /// Events whose level ends short of the target: the level has no more travel.
    pub held: Vec<ClipId>,
}

/// The levels a match may give an event: above its mute, up to what its level line takes (twice the
/// amplitude).
const GAIN_DB: (f64, f64) = (kt_model::EVENT_GAIN_FLOOR_DB as f64 + 1.0, 6.0206);

/// An event shorter than this is measured with its like: a gated average needs at least so much.
const SHORT: Tick = Tick(3 * kt_time::TICK_HZ);

/// The materials of a match: an event alone, or the short events of one track and source together, in
/// the order of the timeline.
pub fn match_groups(p: &Project, ids: &[ClipId]) -> Vec<Vec<ClipId>> {
    let mut clips: Vec<&ClipEvent> =
        p.clips.iter().filter(|c| c.kind == ClipKind::Audio && ids.contains(&c.id)).collect();
    clips.sort_by_key(|c| (c.start, c.id));
    let mut out: Vec<Vec<ClipId>> = Vec::new();
    let mut short: std::collections::HashMap<(u32, MediaId), usize> = Default::default();
    for c in clips {
        if c.duration >= SHORT {
            out.push(vec![c.id]);
            continue;
        }
        match short.entry((c.lane, c.media)) {
            std::collections::hash_map::Entry::Occupied(e) => out[*e.get()].push(c.id),
            std::collections::hash_map::Entry::Vacant(e) => {
                e.insert(out.len());
                out.push(vec![c.id]);
            }
        }
    }
    out
}

/// The event alone: its chain, its curves and the layers over it; no track stage, no neighbour, no fade.
fn alone(p: &Project, id: ClipId) -> Project {
    let mut q = p.clone();
    q.clips.retain(|c| c.id == id);
    q.transitions.clear();
    q.program = None;
    for t in &mut q.tracks {
        t.fx.clear();
        t.fx_bypass.clear();
        t.fx_auto.clear();
        t.fx_in.clear();
        (t.gain_db, t.mute, t.solo) = (0.0, false, false);
    }
    q
}

/// "Match loudness": each material's events take the one correction that brings its integrated
/// loudness to `target`. `progress` gets (materials done, all).
pub fn match_clips(
    p: &Project,
    ids: &[ClipId],
    target: f64,
    sources: &mut dyn FnMut(&Project) -> Sources,
    cancel: &Arc<AtomicBool>,
    progress: &mut dyn FnMut(u64, u64),
) -> std::io::Result<Matched> {
    let groups = match_groups(p, ids);
    let all = groups.len() as u64;
    let mut out = Matched::default();
    for (k, g) in groups.iter().enumerate() {
        let mut meter = LoudnessMeter::new();
        for id in g {
            let q = alone(p, *id);
            let Some(c) = q.clips.first() else { continue };
            let range = (c.start, c.start + c.duration);
            render_raw_audio_range(&q, sources(&q), Some(range), None, cancel, |_| progress(k as u64, all), |b| {
                meter.feed(b);
                Ok(())
            })?;
        }
        let found = meter.finish().lufs;
        for id in g {
            let Some(c) = p.clips.iter().find(|c| c.id == *id) else { continue };
            match found {
                Some(l) => {
                    let want = c.gain_db as f64 + target - l;
                    let to = ((want * 100.0).round() / 100.0).clamp(GAIN_DB.0, GAIN_DB.1);
                    if (to - want).abs() > 0.1 {
                        out.held.push(*id);
                    }
                    out.gains.push((*id, to as f32));
                }
                None => out.silent.push(*id),
            }
        }
    }
    progress(all, all);
    Ok(out)
}
