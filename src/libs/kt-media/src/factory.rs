//! The ONLY home of parsing pseudo-paths and opening sources (R.52).
//!
//! Before this, the `synth:` parsing was written TWICE (kt-app and kt-cli) and had already
//! diverged: kt-app understood `synth:Ramp`, kt-cli only `synth:ramp`,
//! and a real source was silently treated as a missing file (footnote
//! item 211). A third kind of pseudo-path (`gen:`) without a
//! single factory would reproduce the defect more quietly: instead of a colour frame, a
//! placeholder. Therefore:
//! - the parsing is case-insensitive and lives only here;
//! - a broken pseudo-path is an honest error, NOT an attempt to open it
//!   with FFmpeg (“no such file” would hide the real cause).

use crate::MediaSource;
use crate::generator::GeneratorSource;
use crate::synthetic::{AudioPattern, SyntheticSource};
use kt_time::Tick;

/// Duration of synth sources: an hour, as in kt-app from day one.
const SYNTH_DURATION: Tick = Tick(90_000 * 3600);

/// The sample `synth:<pattern>[:<arg>]`. The sample name is case-insensitive
/// (live scenarios write “synth:Ramp”, tests write “synth:ramp”).
pub fn synth_pattern(path: &str) -> Option<AudioPattern> {
    let rest = strip_ci(path, "synth:")?;
    let mut it = rest.split(':');
    match it.next()?.to_ascii_lowercase().as_str() {
        "silence" => Some(AudioPattern::Silence),
        "ramp" => Some(AudioPattern::Ramp),
        "impulse" => Some(AudioPattern::Impulse),
        "sine" => Some(AudioPattern::Sine { hz: it.next()?.parse().ok()? }),
        "dc" => Some(AudioPattern::Dc { value: it.next()?.parse().ok()? }),
        _ => None,
    }
}

/// A source by a pseudo-path. `None`: the path is not pseudo (an ordinary file,
/// opened by FFmpeg). `Some(Err)`: the path is PSEUDO but broken; above it is an
/// error “source unavailable” (a placeholder where it fits), and in
/// FFmpeg such a path does not go.
pub fn open_pseudo(path: &str) -> Option<Result<Box<dyn MediaSource>, String>> {
    if starts_ci(path, "synth:") {
        return Some(match synth_pattern(path) {
            Some(p) => Ok(Box::new(SyntheticSource::new(SYNTH_DURATION, p)) as _),
            None => Err(format!("unknown synth pattern: {path}")),
        });
    }
    if starts_ci(path, "gen:") {
        return Some(match kt_model::parse_gen_path(path) {
            Some(k) => Ok(Box::new(GeneratorSource::new(k)) as _),
            None => Err(format!("unknown generator: {path}")),
        });
    }
    None
}

/// Whether a LIVE source can be moved from the old path to the new one WITHOUT
/// recreation (R.53, item 410). The predicate lives here, in the single home of
/// pseudo-path parsing, and MUST match `MediaSource::retune` exactly (guard: t499):
/// if they diverge, the caller either needlessly recreates the stream or
/// considers reconfigured what was not.
///
/// Files are never retuned: a path change there IS a file change.
pub fn can_retune(old: &str, new: &str) -> bool {
    let (Some(a), Some(b)) = (kt_model::parse_gen_path(old), kt_model::parse_gen_path(new)) else {
        return false;
    };
    // the passport MUST stay the same: for SMPTE, enabling the tone makes
    // the media sound, and that is already a different set of clips on the timeline
    GeneratorSource::info_for(&a) == GeneratorSource::info_for(&b)
}

/// Generator source by the STATE OF THE EVENT (R.59, stage 2).
///
/// A separate door next to `open_pseudo`: a generator on an event no longer has a path
/// with values; the values come from the model, and the `gen:` path
/// of the media carries only the kind. Everyone who raises sources (the player, export,
/// kt-cli) calls exactly this one, so “what the generator draws” is computed
/// in one place.
pub fn open_gen(state: &kt_model::GenState) -> Box<dyn MediaSource> {
    Box::new(GeneratorSource::from_state(state.clone()))
}

/// Whether a LIVE source can be moved to new VALUES without recreation.
/// The twin of `can_retune` MUST match `MediaSource::retune_gen` exactly (guard: t660):
/// if they diverged, the caller either needlessly stops the stream, or
/// considers reconfigured what was not.
pub fn can_retune_gen(old: &kt_model::GenState, new: &kt_model::GenState) -> bool {
    // the passport MUST stay the same: for SMPTE, enabling the tone makes
    // the media sound, and that is already a different set of clips on the timeline
    GeneratorSource::info_for(&old.kind) == GeneratorSource::info_for(&new.kind)
}

/// Opening any path: pseudo: from the factory; file: FFmpeg with a sidecar
/// index DB (the packet index from the DB if the file is unchanged, test 34).
#[cfg(feature = "ffmpeg")]
pub fn open_indexed(
    path: &str,
    db: Option<&mut crate::index::MediaIndexDb>,
) -> Result<Box<dyn MediaSource>, String> {
    if let Some(r) = open_pseudo(path) {
        return r;
    }
    crate::ffmpeg::FfmpegSource::open_indexed(path, db)
        .map(|s| Box::new(s) as _)
        .map_err(|e| e.to_string())
}

fn starts_ci(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

fn strip_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    starts_ci(s, prefix).then(|| &s[prefix.len()..])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t499 (R.53, item 410): the factory predicate and the source implementation do not
    /// diverge on ANY pair of kinds. A divergence would mean either a needlessly
    /// stopped stream, or “reconfigured” what was not changed.
    #[test]
    fn t499_can_retune_equals_retune() {
        use kt_model::{GeneratorKind, format_gen_path};
        let mut kinds: Vec<GeneratorKind> = GeneratorKind::all();
        // variations that change the passport and those that do not
        kinds.push(GeneratorKind::Solid { color_r: 1, color_g: 2, color_b: 3 });
        kinds.push(GeneratorKind::SmpteBars { tone: true, tone_db: -12.0 });
        kinds.push(GeneratorKind::SmpteBars { tone: false, tone_db: -6.0 });

        for a in &kinds {
            for b in &kinds {
                let (pa, pb) = (format_gen_path(a), format_gen_path(b));
                let predicate = can_retune(&pa, &pb);
                let actual = GeneratorSource::new(a.clone()).retune(&pb);
                assert_eq!(predicate, actual, "{pa} -> {pb}: the predicate diverged from the implementation");
            }
        }
        // files and the test scheme are never retuned
        assert!(!can_retune("a.mp4", "b.mp4"));
        assert!(!can_retune("synth:ramp", "synth:sine:440"));
        assert!(!can_retune("gen:solid", "a.mp4"));
    }

    /// t660 (R.59, stage 2; twin of t499): the factory predicate and the
    /// source implementation do not diverge on ANY pair of STATES.
    ///
    /// Were they to diverge, the caller either needlessly stops the decoder stream, or
    /// considers reconfigured what was not, and the picture stays
    /// old forever.
    #[test]
    fn t660_can_retune_gen_equals_retune_gen() {
        use kt_model::{GenState, GeneratorKind, TextAnim, text_anim_curves};
        let mut states: Vec<GenState> =
            GeneratorKind::all().into_iter().map(GenState::new).collect();
        // variations that change the passport and those that do not
        states.push(GenState::new(GeneratorKind::Solid { color_r: 1, color_g: 2, color_b: 3 }));
        states.push(GenState::new(GeneratorKind::SmpteBars { tone: true, tone_db: -12.0 }));
        states.push(GenState::new(GeneratorKind::SmpteBars { tone: false, tone_db: -6.0 }));
        states.push(GenState::new(GeneratorKind::Leader {
            seconds: 3,
            fps: 30,
            beep: true,
            beep_db: -20.0,
        }));
        // and the state WITH CURVES: they do not touch the passport, so
        // the reconfiguration MUST accept them
        states.push(GenState {
            kind: GeneratorKind::default_text(),
            auto: text_anim_curves(TextAnim::Typewriter, 2.0, 2.0),
        });

        for a in &states {
            for b in &states {
                let predicate = can_retune_gen(a, b);
                let actual = GeneratorSource::from_state(a.clone()).retune_gen(b);
                assert_eq!(
                    predicate,
                    actual,
                    "{} -> {}: the predicate diverged from the implementation",
                    a.kind.tag(),
                    b.kind.tag()
                );
            }
        }
        // file sources never accept generator values
        let mut synth = crate::synthetic::SyntheticSource::new(Tick(90_000), AudioPattern::Ramp);
        assert!(!MediaSource::retune_gen(&mut synth, &states[0]));
    }

    /// t471b: factory routing: both pseudo-schemes in any case,
    /// a broken pseudo = Some(Err) (does not go to FFmpeg), a file = None.
    #[test]
    fn t471b_factory_routes_pseudo_paths() {
        assert!(open_pseudo("synth:ramp").unwrap().is_ok());
        assert!(open_pseudo("Synth:Ramp").unwrap().is_ok(), "regression of footnote item 211");
        assert!(open_pseudo("synth:sine:440").unwrap().is_ok());
        assert!(open_pseudo("synth:nope").unwrap().is_err());

        let g = open_pseudo("gen:solid:c=ff8800").unwrap().unwrap();
        assert!(g.info().has_video && !g.info().has_audio);
        assert!(open_pseudo("GEN:SOLID").unwrap().is_ok());
        assert!(open_pseudo("gen:unknown").unwrap().is_err(), "broken gen: — an error, not ffmpeg");

        assert!(open_pseudo("media/clip.mp4").is_none());
        assert!(open_pseudo("generator.mp4").is_none());
    }
}
