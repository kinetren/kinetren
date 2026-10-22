//! Varispeed resampler (a verbatim move from lib.rs). The warp is NOT a node of the
//! chain: it holds the monopoly on consuming the source (chain_source_*).

use crate::{CH, PullKind};
use rubato::{
    Resampler, SincFixedOut, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

// ---------------------------------------------------------------------------
// Warp (varispeed via rubato)
// ---------------------------------------------------------------------------

/// Reads the source SEQUENTIALLY and gives out a clip-local stream with a
/// changed speed (rate < 1 — slowed, the pitch drops: this is the desired
/// slowed&reverb aesthetic). No skips, no duplicates: the source position is
/// an internal counter of rubato, not recalculated from outside.
pub struct Warp {
    rs: SincFixedOut<f32>,
    /// The MODULE of the speed: the direction has no bearing on the resampler
    /// (R.57). The sign lives in the read address (`fill_source`), while rubato eats the
    /// stream at speed |rate| and does not know the direction.
    rate: f64,
    /// Next source frame that will be requested from pull.
    src_cursor: i64,
    /// Ready output (interleaved), not yet handed out.
    ready: std::collections::VecDeque<f32>,
    underruns: u64,
    /// How many times rubato REFUSED a change of ratio (window ±rel around
    /// the anchor). Previously the refusal was swallowed by `let _ =`, and the curve 0.25→32 silently
    /// got stuck at the last accepted value (R.57, a latent defect).
    rejected: u64,
}

const WARP_CHUNK_OUT: usize = 1024;

/// The ratio window around the anchor in the LEGACY mode (the whole curve within
/// 0.25…32): exactly as before R.57 — the byte identity rests on this.
const WARP_RATIO_REL_LEGACY: f64 = 32.0;
/// The ANCHOR ceiling in the extended mode. The anchor sets both the anti-aliasing cutoff
/// (rubato scales `f_cutoff` by the construction ratio) and the
/// buffer size `(rel+1)·needed_input`. The two are a compromise: higher, memory grows;
/// lower, aliasing becomes audible on fast passages.
const WARP_EXT_ANCHOR_MAX_RATE: f64 = 2.0;

impl Warp {
    /// `rate` — the MODULE of the speed. `reach` — the (min, max) of the module over the WHOLE
    /// profile: the resampler needs to know the spread of ratios BEFORE the first chunk,
    /// otherwise a change of ratio will be rejected (see `rejected`).
    pub fn new(rate: f64, src_start: i64, reach: (f64, f64)) -> Warp {
        let rate = rate.abs().clamp(kt_model::RATE_AUDIO_FLOOR, 32.0);
        // LEGACY MODE (the profile entirely within 0.25…32) — the body verbatim
        // as before: the same anchor, the same window. The bytes do not move.
        let extended = reach.0 < 0.25;
        let (anchor, rel) = if extended {
            let anchor = rate.clamp(kt_model::RATE_AUDIO_FLOOR, WARP_EXT_ANCHOR_MAX_RATE);
            let rel = WARP_RATIO_REL_LEGACY
                .max(reach.1 / anchor)
                .max(anchor / reach.0.max(kt_model::RATE_AUDIO_FLOOR))
                * 1.01;
            (anchor, rel)
        } else {
            (rate.clamp(0.25, 32.0), WARP_RATIO_REL_LEGACY)
        };
        let params = SincInterpolationParameters {
            sinc_len: 128,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };
        // ratio = f_out / f_in = 1 / rate (rate 0.5 → twice as much output)
        let mut rs = SincFixedOut::<f32>::new(1.0 / anchor, rel, params, WARP_CHUNK_OUT, CH)
            .expect("rubato init");
        let mut rejected = 0;
        if anchor != rate && rs.set_resample_ratio(1.0 / rate, false).is_err() {
            rejected += 1;
            debug_assert!(false, "rubato rejected the initial ratio");
        }
        Warp {
            rs,
            rate,
            src_cursor: src_start,
            ready: std::collections::VecDeque::new(),
            underruns: 0,
            rejected,
        }
    }

    /// `rate` — the MODULE of the speed.
    pub fn set_rate(&mut self, rate: f64) {
        let rate = rate.abs().clamp(kt_model::RATE_AUDIO_FLOOR, 32.0);
        if (rate - self.rate).abs() > 1e-9 {
            self.rate = rate;
            // ramp=true: rubato smoothly drives the ratio toward the target within the chunk
            if self.rs.set_resample_ratio(1.0 / rate, true).is_err() {
                // A REFUSAL IS NAMED (R.57): a silent sticking to the previous
                // ratio is heard as “the slowdown was not applied”
                self.rejected += 1;
                debug_assert!(false, "rubato rejected a ratio change: {rate}");
            }
        }
    }

    /// How many ratio changes rubato rejected (diagnostics, tests).
    pub fn rejected(&self) -> u64 {
        self.rejected
    }

    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Emit `out.len()/CH` frames, pulling the source via `pull`.
    /// `pull(frame, buf)` fills the interleaved frames of the source, starting from
    /// `frame`; false = failed (underrun, silence will follow).
    pub fn produce(
        &mut self,
        kind: PullKind,
        out: &mut [f32],
        pull: &mut dyn FnMut(PullKind, i64, &mut [f32]) -> bool,
    ) {
        let need_frames = out.len() / CH;
        while self.ready.len() < need_frames * CH {
            let in_frames = self.rs.input_frames_next();
            let mut input = vec![0f32; in_frames * CH];
            if !pull(kind, self.src_cursor, &mut input) {
                self.underruns += 1;
                input.fill(0.0);
            }
            self.src_cursor += in_frames as i64;
            // interleaved → planar
            let planar: Vec<Vec<f32>> =
                (0..CH).map(|ch| (0..in_frames).map(|f| input[f * CH + ch]).collect()).collect();
            let out_planar = self.rs.process(&planar, None).expect("rubato process");
            let produced = out_planar[0].len();
            for f in 0..produced {
                for chd in out_planar.iter().take(CH) {
                    self.ready.push_back(chd[f]);
                }
            }
        }
        for s in out.iter_mut() {
            *s = self.ready.pop_front().unwrap();
        }
    }

    /// Reposition the source WITHOUT re-creating the resampler — the wrap seam
    /// (see `ClipProcessor::seam`): the internal state of the sinc lives on, only the
    /// produced but not yet handed-out output is discarded
    /// (it was computed from the previous source position).
    pub fn seek_source(&mut self, src: i64) {
        self.src_cursor = src;
        self.ready.clear();
    }

    pub fn underruns(&self) -> u64 {
        self.underruns
    }

    /// Current source cursor (diagnostics/tests).
    pub fn src_cursor(&self) -> i64 {
        self.src_cursor
    }
}
