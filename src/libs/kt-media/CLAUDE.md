# kt-media

> The crate's contracts and mandatory tests — `openspec/specs/{media-index,generators}/spec.md`; the reasons for the decisions — `design.md` next to it.

Media sources: the trait `MediaSource`, `SyntheticSource` (tests), `FfmpegSource` (a bridge to kt-ffmpeg, feature `ffmpeg`), `PlaceholderSource` (a missing file), `GeneratorSource` (generators of R.52), a single factory of pseudo-paths (`factory`), the sidecar index DB (schema — docs/media-index.md).

## Invariants
- All audio going out — **48 kHz stereo f32 interleaved**, exactly 1024 frames per burst. Format conversion (swr) happens INSIDE the source. No consumer sees the sample rate of the file (defect a — the hiccups of slowed&reverb in the old code).
- `read_audio_burst(idx)` is sample-accurate: burst idx covers the samples [idx·1024, (idx+1)·1024) from the start of the audio stream. Checked by the conformance test 10 (== decoding from scratch).
- An instance of `MediaSource` is owned by one thread (`&mut self`) — the scheduler of kt-engine, not the crate itself, guarantees exclusivity.
- `SyntheticSource` lives in the main crate (not under cfg(test)) and counts the calls (`audio_reads`/`video_reads`) — the regressions "zero decodes" (tests 13, 15) are built on this.
- `CheckerSource` (R.46) — a source with a REAL native size: stripes of 2 px in the native size and honest box minification. It is needed where SHARPNESS is measured: `SyntheticSource` draws a fill at any requested size, its `info.width/height` are fictitious, and the loss of detail on decoding in Quality cannot be expressed on it (t312-t317, t315).
- The specifics of rsmpeg do not go beyond the crate; a change of the FFmpeg major does not touch the rest of the code.
- **Parsing of pseudo-paths (`synth:`, `gen:`) — ONLY in `factory.rs`** (R.52). Before the factory, the parsing of `synth:` lived in kt-app and kt-cli and diverged by case (footnote to item 211): "synth:Ramp" was silently taken for a missing file. Rules: the parsing is case-insensitive; a broken pseudo-path is an honest `Err`, it does not go to FFmpeg; no new copies of the parsing are to be created — `open_pseudo`/`open_indexed`, the predicate — `kt_model::is_pseudo_path`.
- **`GeneratorSource` (R.52, `generator.rs`)**: a frame of EXACTLY the requested Quality (the composite discards a foreign size silently), a deterministic function (parameters, pts, Quality) — invariant 11; "no sound" = `has_audio:false` AND `Ok(AudioBurst::silence)` (`Err` = false underruns in the negative cache of the worker). `info()`: duration=0 ("there is no duration", the UI substitutes its own default on drop), fps=None (the project grid), width/height=`GEN_NATIVE`=4096 — a "reserve of detail" for the decode scale step under a zooming chain (= `kt_fx::plan::MAX_INTER_DIM`, linked by a comment; the pseudo-paths do not take part in "Max (by sources)"). The vocabulary of kinds and the parameters — `kt_model::GeneratorKind` (the dependency kt-media → kt-model, acyclic).
- **Generators of R.53 (items 419-424)**: shapes (`Rect`/`Ellipse`/`Line`),
  a transparent backing, a countdown, engineering tables and a level
  window. Rules that the new kind will have to observe:
  - **Float — ONLY in the preamble of the frame.** The distance fields of the shapes are computed in
    integer 1/256 px (`isqrt`, `coverage`), the chroma plane — by the tables of columns and rows computed before the loop. A pixel loop with
    `f64::sqrt`/`atan2` inside is not "the same, but simpler": it is
    a different profile of rounding errors and a different speed.
  - **The alpha of a kind — an EXHAUSTIVE `kind_has_alpha`**, not a `matches!` with
    a single variant: a new transparent kind would slip past the line silently
    and would land in the composite as a black slab.
  - **The sound of a kind — the same `tone_1k` with a gate.** A second sine would
    diverge in phase; the phase is taken from the GLOBAL sample number, therefore seek does not click.
  - **The cache by Quality disables `depends_on_time`** — and, since R.59, also
    `GenState::animated()` (whether there are curves): static kinds stay in the
    cache, animated ones are computed on every tick.
  - The animation of a title is FOUR NUMBERS of the kind itself (`reveal`,
    `reveal_soft`, `offset_x/y`); `text_params` converts fractions of the frame into
    pixels, the quantum 1/64 px lives in kt-text. There is no separate `animate_text`
    any more: no modes remain in the model.

- **The values of a generator arrive as STATE (`GenState`), not as a path (R.59, stage 2).** They are a property of the EVENT, therefore:
  - the source is opened by `factory::open_gen(&GenState)` and stands at the address
    of the event (`kt_engine::gen_source_addr`), not of a media item;
  - **the curves are resolved by the SOURCE** by its own `pts` (`GenState::kind_at`,
    `Cow::Borrowed` without curves — zero cost for a non-animated one). The plan does not know
    about them at all: the order holds `(address, tick, quality)`, and the frame
    MUST be a function of this triple, otherwise the cache key stops
    determining the frame (invariant 4);
  - reconfiguration in place — `MediaSource::retune_gen`, the twin of `retune`
    with the same strict contract; the predicate `factory::can_retune_gen`
    MUST match it exactly (the guard t660). A separate method, and not
    "`retune` via a string": curves do not fit into a path.
- **The `reel_*` tables in the sidecar are keyed by CONTENT, not by a file (R.58,
  item 463).** A nested reel has no file, and `media(path, size, mtime,
  xxhash)` is not its home: the key is `(seq, hash, media_fp)` — the id of the reel, the revision
  (`kt_engine::content_hash`) and the fingerprint of the files of its tree. Consequences:
  `purge_media` does NOT touch these tables (and must not — a substitution of a file is caught by
  `media_fp`, not by `media_id`); the identity of the media travels IN THE KEY, not as an
  external key; any discrepancy — `None`, and the reel is rendered anew;
  one version per reel (`put_reel_preview` displaces the previous one entirely);
  the cleanup — `prune_reels(keep)`. The format of the peak blob is the same as that of
  `audio_peaks` (`peaks_blob`/`peaks_from_blob` are one pair for both tables). Test 639.
- The pixel font 3×5 and `Canvas` — the common module `pixfont` (a placeholder + a future timecode generator). This is NOT the rasterization of the user's title text: that lives in kt-text with its own determinism discipline.

## FfmpegSource (src/ffmpeg.rs) — how it is built
- Two INDEPENDENT demuxers per file (audio and video do not fight over the seek) + a third scan pass on opening: a packet index (positions/pts of the audio packets, pts of the video keyframes) without decoding.
- Audio: swr → 48k stereo FLT inside; a FIFO with an absolute anchor by the pts of the first decoded frame after a seek. Seek = a container BACKWARD seek to the indexed packet (with a margin of 3 packets) + a FULL re-creation of the decoder and swr. Sample accuracy is confirmed by the conformance test: FLAC — exact, AAC — zero phase shift (lossy values are compared by correlation).
- Video: `threads=1` for VP9/AV1 (a legacy of C++: MT decoding of these codecs is unstable under frequent seeks); a backward seek = seek + re-creation of the decoder ALWAYS (it covers MT H.264, and is cheaper than catching artifacts); forward ≤4 s — a continuous decode without seek. sws → RGBA, the context cache by (size, source format).
- `unsafe impl Send` is correct, because an instance is owned by exactly one MediaWorker (&mut self); Sync is deliberately not implemented.
- The packet index is persisted in the sidecar DB (`packet_index`, schema v2): `FfmpegSource::open_indexed(path, db)` takes the index from the DB when the file is unchanged (identity of size/mtime/xxhash via `ensure_media`) and skips the full scan pass; after a fresh scan — it saves it. DB errors = cache errors: a silent fallback to the ordinary open. Test 34.
- The decoding layer implementation — the crate kt-ffmpeg (dlopen of the system FFmpeg 6/7/8/9, without linking); rsmpeg is removed.
