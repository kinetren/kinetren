//! FxStateStore: the state of FX chains by clip id, OUTSIDE the render graph
//! (invariants 2–3). Owner — the audio render thread; neither prefetch nor the UI
//! enter here. Rebuilding AudioRenderer (any model edit) does NOT
//! destroy the state: the same chain structure → the same processor,
//! parameters arrive smoothly (defects b, f of the old engine).

use kt_fx::{ClipProcessor, TrackProcessor, chain_fingerprint};
use kt_model::{ClipId, FxInstance, TrackId};
use std::collections::HashMap;

#[derive(Default)]
pub struct FxStateStore {
    map: HashMap<ClipId, ClipProcessor>,
    /// TRACK chains (R.62, change B). The key is `TrackId`, not “(kind,
    /// number)”: the number is the row's rank, and inserting a track higher would
    /// renumber the neighbours, i.e. kill their state
    /// (invariant 3). A separate map instead of a composite key: a track and an event have
    /// different processors, and a `HashMap` keyed by enum would cost a
    /// branch on every access for one and the same thing.
    tracks: HashMap<TrackId, TrackProcessor>,
    /// Sub-mix streams of nested reels (R.50 B4, key corrected in R.55 C4).
    ///
    /// The key is the address of the INSTANCE, not of the reel. A reel placed in five
    /// places is FIVE different streams through its chains, and sharing one reverb
    /// between them would be audible; up to R.55 the key was per reel, and the
    /// comment on that same spot contradicted itself. Under reverse and
    /// stretching (C4) the instances also read the reel in a DIFFERENT order,
    /// i.e. they would fight over one producer cursor.
    ///
    /// `ClipId` is unique across the whole document and survives trim, cut and
    /// move (invariant 3) — exactly the same key as `map`.
    nests: HashMap<ClipId, NestStream>,
    /// SIDECHAIN KEY BRANCHES (R.65): each TARGET track has its own
    /// sub-store in which its key stream lives — the states of the events of the
    /// key track and its chain.
    ///
    /// A second set of states is needed for the same reason that a reel has its own
    /// `NestStream`: the same processor cannot be run twice per frame
    /// (the key branch and the key track's own output are TWO streams through the same
    /// chains), and their windows also differ
    /// when the target has lookahead.
    keys: HashMap<TrackId, Box<FxStateStore>>,
}

/// Sub-mix stream of ONE instance of a nested reel.
///
/// The sub-mix is a STREAMING AUTOMATON: inside the reel live reverbs and delays with
/// their own state, and recomputing from a random position gives a different output.
/// Therefore the reel is rendered ONLY FORWARD and only in increasing order of its own
/// time, while the reverse and stretching of the instance act on the SAMPLES of the finished
/// mix — just as on the samples of a file. The reverse (“render the reel backwards”)
/// would give a third, nobody's result: the internal reverb must sound
/// forward INSIDE the reel, and a mirror from outside gives a reversed reverb.
pub struct NestStream {
    /// Which reel AND which of its REVISIONS (R.59, stage 3). The instance could have been
    /// re-pointed to another reel (`SetClipMedia`) or had its overrides changed —
    /// in both cases the stream starts anew
    /// rather than inheriting someone else's state.
    pub seq: crate::video::NestKey,
    pub store: FxStateStore,
    /// A continuous segment of already computed bursts, fresh ones at the end.
    /// A repeated request for the same burst is a common thing (`render_plain_clip`
    /// keeps its cursor on it, and the reverse mirror reads it many times), and the
    /// streaming chain must not be recomputed a second time.
    pub ring: std::collections::VecDeque<(i64, std::sync::Arc<kt_media::AudioBurst>)>,
    /// The next burst the producer will deliver. It goes STRICTLY FORWARD.
    pub next: Option<i64>,
    /// Pre-roll of a reposition in bursts — the same discipline as for a real
    /// source (invariant 11: seek with pre-roll `max(tail, 100 ms)`).
    /// A reel WITHOUT tail effects has zero pre-roll, and reverse costs exactly
    /// the same as forward playback.
    pub preroll: u32,
    /// Block size of the canonical grid. The reposition starts from the BLOCK
    /// BOUNDARY, not “from i back by P”: then a burst's value is a pure
    /// function of the index, and the bytes depend neither on the read history nor on
    /// the chunk in which it was requested.
    pub block: u32,
}

impl NestStream {
    /// A fresh stream. `pub` because the producer in `audio.rs` also creates it:
    /// there the reader becomes known only at the moment of reading.
    pub fn new_for(seq: crate::video::NestKey, preroll: u32, block: u32) -> NestStream {
        NestStream::new(seq, preroll, block)
    }

    fn new(seq: crate::video::NestKey, preroll: u32, block: u32) -> NestStream {
        NestStream {
            seq,
            store: FxStateStore::new(),
            ring: std::collections::VecDeque::new(),
            next: None,
            preroll,
            block,
        }
    }

    /// Start of the block containing burst `i`.
    pub fn block_start(&self, i: i64) -> i64 {
        let w = self.block.max(1) as i64;
        i.div_euclid(w) * w
    }

    pub fn cached(&self, i: i64) -> Option<std::sync::Arc<kt_media::AudioBurst>> {
        self.ring.iter().find(|(k, _)| *k == i).map(|(_, b)| b.clone())
    }

    pub fn push(&mut self, i: i64, b: std::sync::Arc<kt_media::AudioBurst>) {
        // The ring ceiling is a FUNCTION OF THE PROJECT (block plus pre-roll), not
        // a memory budget: if the output bytes depended on the budget, invariant 11
        // (“audio byte-for-byte always and everywhere”) would collapse.
        let cap = (self.block as usize + self.preroll as usize).max(2);
        self.ring.push_back((i, b));
        while self.ring.len() > cap {
            self.ring.pop_front();
        }
    }

    pub fn rewind(&mut self) {
        self.ring.clear();
        self.next = None;
        self.store.reset_all();
    }
}

impl FxStateStore {
    pub fn new() -> FxStateStore {
        FxStateStore::default()
    }

    /// Synchronize the clip processor with the model:
    /// - the chain structure is the same → reuse the state, carry the parameters;
    /// - the structure changed → recreate ONLY this clip;
    /// - the chain is empty → no processor needed.
    pub fn sync_clip(
        &mut self,
        id: ClipId,
        fx: &[FxInstance],
        auto: &kt_fx::AutoProfiles,
        src_in_frames: i64,
        clip_len_frames: i64,
        opts: kt_fx::ChainOpts,
    ) {
        if fx.is_empty() {
            self.map.remove(&id);
            return;
        }
        let fp = chain_fingerprint(fx);
        match self.map.get_mut(&id) {
            Some(p) if p.fingerprint() == fp => {
                p.set_clip_len(clip_len_frames);
                // GEOMETRY is carried too (R.43, item 293): `chain_fingerprint`
                // hashes only the KINDS of slots, so trim/cut/punch-out/
                // re-anchoring of a morph piece changed `source_in` with the same id and the
                // same chain structure — and the processor read from the old
                // anchor, i.e. “sound from another event”.
                p.set_src_in(src_in_frames);
                // curves are carried BEFORE the parameters: apply_params checks
                // which slots have a curve (volume, speed)
                p.set_auto(auto);
                p.apply_params(fx, false);
                // Lookahead is part of the chain's DEVICE, not the value of a
                // parameter (R.48, item 337): the length of the rings changes the alignment of the
                // stream, and it cannot be carried by smoothing. A mismatch
                // (an edit of the limiter's window, pitch neutral ↔ active) —
                // a processor rebuild: the next read will do hard_seek
                // with warm-up, like a real seek.
                if p.lookahead_frames() != kt_fx::chain_lookahead(fx, auto) || p.opts() != opts {
                    self.map.insert(
                        id,
                        ClipProcessor::new_with_opts(
                            fx,
                            auto,
                            src_in_frames,
                            clip_len_frames,
                            opts,
                        ),
                    );
                }
            }
            _ => {
                self.map.insert(
                    id,
                    ClipProcessor::new_with_opts(fx, auto, src_in_frames, clip_len_frames, opts),
                );
            }
        }
    }

    /// Synchronize the TRACK's processor with the model — by the same rule
    /// as for an event: the chain structure is the same → the state lives on, values
    /// arrive smoothly; changed (including the lookahead window or the
    /// pitch engine) → rebuild; the chain is empty → no processor needed.
    pub fn sync_track(&mut self, id: TrackId, fx: &[FxInstance], opts: kt_fx::ChainOpts) {
        if fx.is_empty() {
            self.tracks.remove(&id);
            return;
        }
        let fp = chain_fingerprint(fx);
        let empty = kt_fx::AutoProfiles::default();
        match self.tracks.get_mut(&id) {
            Some(p) if p.fingerprint() == fp => {
                p.apply_params(fx, false);
                if p.lookahead_frames() != kt_fx::chain_lookahead(fx, &empty) {
                    self.tracks.insert(id, TrackProcessor::new(fx, opts));
                }
            }
            _ => {
                self.tracks.insert(id, TrackProcessor::new(fx, opts));
            }
        }
    }

    pub fn track_mut(&mut self, id: TrackId) -> Option<&mut TrackProcessor> {
        self.tracks.get_mut(&id)
    }

    /// The KEY BRANCH sub-store of the target track (created on first
    /// access). Inside — a regular store: the events of the key track and its
    /// chain live there with their own states.
    pub fn key_mut(&mut self, target: TrackId) -> &mut FxStateStore {
        self.keys.entry(target).or_default()
    }

    /// Remove key branches of targets that no longer exist (or that
    /// stopped listening to someone else's signal).
    pub fn retain_keys(&mut self, alive: &dyn Fn(TrackId) -> bool) {
        self.keys.retain(|id, _| alive(*id));
    }

    /// Remove the state of clips that no longer exist in the model.
    pub fn retain_clips(&mut self, alive: &dyn Fn(ClipId) -> bool) {
        self.map.retain(|id, _| alive(*id));
    }

    /// Remove the state of tracks that no longer exist in the model.
    pub fn retain_tracks(&mut self, alive: &dyn Fn(TrackId) -> bool) {
        self.tracks.retain(|id, _| alive(*id));
    }

    pub fn get_mut(&mut self, id: ClipId) -> Option<&mut ClipProcessor> {
        self.map.get_mut(&id)
    }

    /// A full reset (transport seek): each processor will do a pre-roll
    /// at the next read.
    /// The instance's sub-mix stream (created on first access).
    /// The instance was re-pointed to another reel — the stream starts over.
    pub fn nest_mut(
        &mut self,
        id: ClipId,
        seq: crate::video::NestKey,
        preroll: u32,
        block: u32,
    ) -> &mut NestStream {
        let e = self.nests.entry(id).or_insert_with(|| NestStream::new(seq, preroll, block));
        if e.seq != seq {
            *e = NestStream::new(seq, preroll, block);
        } else {
            e.preroll = preroll;
            e.block = block;
        }
        e
    }

    /// Take the reel states OUT for the duration of the render.
    ///
    /// Needed for exactly one thing: the sub-render is called FROM the fetcher, and the fetcher
    /// lives next to the store — there is no way to take both and mutate them at the same time.
    /// The extracted map unties the borrows, and `put_nests`
    /// returns it back in place within the same call.
    pub fn take_nests(&mut self) -> HashMap<ClipId, NestStream> {
        std::mem::take(&mut self.nests)
    }

    pub fn put_nests(&mut self, v: HashMap<ClipId, NestStream>) {
        self.nests = v;
    }

    pub fn retain_nests(&mut self, alive: &dyn Fn(ClipId) -> bool) {
        self.nests.retain(|id, _| alive(*id));
    }

    pub fn reset_all(&mut self) {
        // seek goes DEEP: the chains inside a reel are the same streaming
        // automata, and a reverb tail from another place would sound “from the past”
        for n in self.nests.values_mut() {
            n.rewind();
        }
        for p in self.map.values_mut() {
            p.reset();
        }
        // a TRACK's chain — the same streaming automaton (R.62)
        for p in self.tracks.values_mut() {
            p.reset();
        }
    }
}
