//! Transport: the authoritative playhead and the playback state.
//!
//! Invariant 6: the master clock is the audio consumption. `advance_frames` is called
//! ONLY by the audio render thread, exactly by the number of frames consumed by the
//! audio callback (M4). Wall-clock is forbidden here (determinism, invariant 10).
//! In M2 the “callback” is an offline driver/test — the semantics are the same.

use kt_time::Tick;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering};

#[derive(Default)]
pub struct Transport {
    /// Position in 48 kHz audio frames (not in ticks: advancement is whole frames).
    playhead_frames: AtomicI64,
    playing: AtomicBool,
    /// Grows at every seek: MediaWorkers drop requests of stale
    /// generations (invariant 5), FX reset (invariant 3).
    generation: AtomicU64,
    /// Jumps across the edge of the loop region (R.15 item 2): the video part needs them,
    /// since otherwise it has no way to learn about the transition to the new lap.
    wraps: AtomicU64,
}

impl Transport {
    pub fn new() -> Transport {
        Transport::default()
    }

    pub fn playhead_frames(&self) -> i64 {
        self.playhead_frames.load(Ordering::Acquire)
    }

    pub fn playhead(&self) -> Tick {
        Tick::from_audio_frames(self.playhead_frames())
    }

    pub fn is_playing(&self) -> bool {
        self.playing.load(Ordering::Acquire)
    }

    pub fn set_playing(&self, on: bool) {
        self.playing.store(on, Ordering::Release);
    }

    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// How many times the position JUMPED across the edge of the loop region.
    /// Generation is deliberately NOT bumped on wrap (it is not a seek, the sound and the
    /// FX tails continue), so the video part learns about the jump only
    /// from here. Without this counter the anti-rollback ratchet of the key
    /// froze the picture on the second lap of the loop, while the sound
    /// kept playing (R.15 item 2).
    pub fn wraps(&self) -> u64 {
        self.wraps.load(Ordering::Acquire)
    }

    /// Advance by the consumed audio frames. Only the audio render thread.
    pub fn advance_frames(&self, frames: i64) {
        self.playhead_frames.fetch_add(frames, Ordering::AcqRel);
    }

    /// Advancement with a wrap over the loop region [start, end) in frames.
    /// Generation is NOT bumped: a wrap is not a seek, the FX state and the ring live on
    /// (otherwise an audible gap). `frames` may be negative
    /// (reverse transport, J4 v1.4): the wrap works both ways —
    /// rem_euclid is correct for negative offsets (test 43); outside the
    /// region the position is clamped to 0 (reverse does not drive the playhead into minus).
    pub fn advance_frames_wrapped(&self, frames: i64, region: Option<(i64, i64)>) {
        match region {
            Some((s, e)) if e > s => {
                let prev =
                    self.playhead_frames.fetch_update(Ordering::AcqRel, Ordering::Acquire, |p| {
                        let np = p + frames;
                        Some(if p >= s && p <= e && (np >= e || np < s) {
                            s + (np - s).rem_euclid(e - s)
                        } else {
                            np.max(0)
                        })
                    });
                // the counter is bumped AFTER the swap, using the returned old
                // value: the fetch_update closure may run several times on a race,
                // so it must not count inside
                if let Ok(p) = prev {
                    let np = p + frames;
                    if p >= s && p <= e && (np >= e || np < s) {
                        self.wraps.fetch_add(1, Ordering::AcqRel);
                    }
                }
            }
            _ => {
                let _ =
                    self.playhead_frames.fetch_update(Ordering::AcqRel, Ordering::Acquire, |p| {
                        Some((p + frames).max(0))
                    });
            }
        }
    }

    /// Jump. Bumps generation.
    pub fn seek_frames(&self, frame: i64) {
        self.playhead_frames.store(frame, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    pub fn seek(&self, t: Tick) {
        self.seek_frames(t.to_audio_frames());
    }
}

#[cfg(test)]
mod reverse_wrap_tests {
    use super::*;

    /// Test 43 (J4): loop wrap works in both directions; outside the region
    /// a negative advance is clamped to 0.
    #[test]
    fn t43_wrap_backwards_and_clamp() {
        let t = Transport::new();
        // forward across the boundary (regression not broken)
        t.seek_frames(9_800);
        t.advance_frames_wrapped(400, Some((5_000, 10_000)));
        assert_eq!(t.playhead_frames(), 5_200);
        // backward across the region start → to the end
        t.seek_frames(5_100);
        t.advance_frames_wrapped(-400, Some((5_000, 10_000)));
        assert_eq!(t.playhead_frames(), 9_700);
        // exactly at start: without movement, no wrap
        t.seek_frames(5_000);
        t.advance_frames_wrapped(0, Some((5_000, 10_000)));
        assert_eq!(t.playhead_frames(), 5_000);
        // backward exactly by span → same position
        t.seek_frames(7_000);
        t.advance_frames_wrapped(-5_000, Some((5_000, 10_000)));
        assert_eq!(t.playhead_frames(), 7_000);
        // outside the region (to the left): normal movement, no wrap
        t.seek_frames(1_000);
        t.advance_frames_wrapped(-300, Some((5_000, 10_000)));
        assert_eq!(t.playhead_frames(), 700);
        // no region: clamp to 0
        t.seek_frames(100);
        t.advance_frames_wrapped(-500, None);
        assert_eq!(t.playhead_frames(), 0);
    }
}
