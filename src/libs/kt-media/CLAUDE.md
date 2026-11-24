# kt-media

> The crate's contracts and mandatory tests — `openspec/specs/{media-index,generators}/spec.md`; the reasons for the decisions — `design.md` next to it.

Media sources: the trait `MediaSource`, `SyntheticSource` (tests), `FfmpegSource` (a bridge to kt-ffmpeg, feature `ffmpeg`), `PlaceholderSource` (a missing file), `GeneratorSource` (generators of R.52), a single factory of pseudo-paths (`factory`), the project's cache folder with the media index and the reel previews.

## Invariants
- All audio going out — **48 kHz stereo f32 interleaved**, exactly 1024 frames per burst. Format conversion (swr) happens INSIDE the source. No consumer sees the sample rate of the file (defect a — the hiccups of slowed&reverb in the old code).
- `read_audio_burst(idx)` is sample-accurate: burst idx covers the samples [idx·1024, (idx+1)·1024) from the start of the audio stream. Checked by the conformance test 10 (== decoding from scratch).
- An instance of `MediaSource` is owned by one thread (`&mut self`) — the scheduler of kt-engine, not the crate itself, guarantees exclusivity.
- `SyntheticSource` lives in the main crate (not under cfg(test)) and counts the calls (`audio_reads`/`video_reads`) — the regressions "zero decodes" (tests 13, 15) are built on this.
- **A burst dispatches on its pattern ONCE, not per sample** (slowed-reverb-bimodal-cost). A
  per-sample `match` over an enum with float payloads lets the compiler hoist one arm's pure math
  (`sin`) out of the match and run it for EVERY variant, on the payload bytes a dataless variant
  leaves from its stack frame. The bytes do not move, the cost does: a subnormal there made a
  `Clicks` burst 6.5 times as expensive at random, and a whole audio measurement bimodal (104 or
  415 ms). Each arm fills the burst with its own loop (`fill(data, base, arm)`); `sample_at`
  calls the same arm functions, so the reference and the source agree byte for byte (t991).
- `CheckerSource` (R.46) — a source with a REAL native size: stripes of 2 px in the native size and honest box minification. It is needed where SHARPNESS is measured: `SyntheticSource` draws a fill at any requested size, its `info.width/height` are fictitious, and the loss of detail on decoding in Quality cannot be expressed on it (t312-t317, t315).
- The specifics of rsmpeg do not go beyond the crate; a change of the FFmpeg major does not touch the rest of the code.
- **Parsing of pseudo-paths (`synth:`, `gen:`) — ONLY in `factory.rs`** (R.52). Before the factory, the parsing of `synth:` lived in kt-app and kt-cli and diverged by case (footnote to item 211): "synth:Ramp" was silently taken for a missing file. Rules: the parsing is case-insensitive; a broken pseudo-path is an honest `Err`, it does not go to FFmpeg; no new copies of the parsing are to be created — `open_pseudo`/`open_indexed`, the predicate — `kt_model::is_pseudo_path`.
- **`GeneratorSource` (R.52, `generator.rs`)**: a frame of EXACTLY the requested Quality (the composite discards a foreign size silently), a deterministic function (parameters, pts, Quality) — invariant 11; "no sound" = `has_audio:false` AND `Ok(AudioBurst::silence)` (`Err` = false underruns in the negative cache of the worker). `info()`: duration=0 ("there is no duration", the UI substitutes its own default on drop), fps=None (the project grid), width/height=`GEN_NATIVE`=4096 — a "reserve of detail" for the decode scale step under a zooming chain (= `kt_fx::plan::MAX_INTER_DIM`, linked by a comment; the pseudo-paths do not take part in "Max (by sources)"). The vocabulary of kinds and the parameters — `kt_model::GeneratorKind` (the dependency kt-media → kt-model, acyclic).
- **A plugin generator kind draws, sounds and declares itself** (plugin-api-stage1, D13):
  `render_at` calls its `GenKernel::frame`, `info_for`/`read_audio_burst` its `tone`/`gate` (the
  sine stays this crate's one `tone_1k`), `kind_has_alpha`/`depends_on_time` read `GenDecl.alpha`/
  `live`. `factory::open_gen` opens a `Missing` kind as a NAMED `PlaceholderSource` (t1050). The
  3×5 font is `kt_plugin_api::pixfont` (re-exported here) so a family crate can draw text.
- **Generators of R.53 (items 419-424)**: shapes (`Rect`/`Ellipse`/`Line`),
  a transparent backing, a countdown, engineering tables and a level
  window. Rules that the new kind will have to observe:
  - **Float — ONLY in the preamble of the frame, on the CPU kinds.** The shapes, gradients, noise and the leader are compute shaders (gpu-render-hardening, D7), f32 pixels: WGSL has no i64. The distance fields were computed in
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
- **The cache folder (`cache_folder.rs`, project-cache-folder)**: `<project folder>/kinetren-cache/`,
  one per project folder, everything in it recomputable. One `Folder` per place and process
  (`Folder::open` memoizes), shared by the lanes; it never touches the user's data folder until a binary
  said where it is (`set_data_root`), so a test writes nothing of his.
  - **An entry is a file that appears by a rename**, under a 32-byte header (kind, the kind's version,
    length, checksum): what does not verify is a miss and is removed, its neighbours untouched. A kind's
    version replaces the schema bump, which wiped every media's index.
  - **No lock stands between a lane and an entry** (t1460, t1171). The folder's one lock is for the
    cycle's drops and the map's compaction, and it is only TRIED.
  - **Room is asked before the bytes** (`ask_room`, or `with_room` inside `write`/`append`); an owner that
    writes its own files (segments, copies) asks, then tells `wrote`/`used`/`gone` and reads
    `take_dropped`. The drop order is one function (`drop_for`): value per byte for frames, mix and
    copies, then the reels, then the media; nothing this process used in the last minute.
  - **Nothing scans at rest**: the tree is walked at the first ask for room, and again only when the
    cycle drops on a count older than a minute, or the system contradicted it.
- **A source copy is the picture only** (`copy.rs`, persistent-cache D3, t3440–t3442): `media/<id>/copy-<w>x<h>.mov`,
  all-intra, every frame at the original's own time in the original's time base (Matroska keeps milliseconds);
  `CopySource` answers the original's facts and reads the sound there. A copy is never opened by an export. No
  hardware encoder — no copy; a forced encoder is a test's.
- **A media is its content** (`ContentId`, the xxh3-128 of the file); `media/paths.tsv` (path, size, mtime
  → id) spares the hash. A path inside the project's folder is kept relative, so a renamed folder hashes
  nothing (t3829). Invalidation needs no delete: a changed file is another id.
- **A source says what it decodes** (`MediaSource::identity`, durable-moment-key D1, t4111): a file — its
  name, size and mtime read ONCE at the open (a file removed or replaced under an open decoder keeps its
  answer), a generator — its values and what it hears, after a retune too. `None` gets an identity of this
  process (`SourceIdent::once`): its frames are never written to disk. A new source kind whose picture
  depends on more than that answers `None` or says all of it.
- **Reel previews are keyed by CONTENT, not by a file (R.58)**: `(seq, hash, media_fp)` — the id of the
  reel, the revision (`kt_engine::content_hash`) and the fingerprint of the files of its tree; any
  discrepancy — `None`. The folder is shared by projects whose reel ids collide, so each project names its
  version of a reel under `reels/refs/`, and only a version nobody names goes (t639).
- **SQLite is only the import door** (`old_sidecar.rs`): a former build's `<project>.ktindex.db` is read
  once on the index lane and deleted; a locked one is left alone (t3832). No new code uses `rusqlite`.
- **A peak pass reads what has an end and stops when told** (t1740–t1742): a still and a placeholder say `placeholder::NO_END`, 1000 hours, and answer every read at once. A new "practically infinite" source uses that constant, or the pass spins for hours on it and starves the queue.
- The pixel font 3×5 and `Canvas` — the common module `pixfont` (a placeholder + a future timecode generator). This is NOT the rasterization of the user's title text: that lives in kt-text with its own determinism discipline.

## FfmpegSource (src/ffmpeg.rs) — how it is built
- Two INDEPENDENT demuxers per file (audio and video do not fight over the seek) + a third scan pass on opening: a packet index (positions/pts of the audio packets, pts of the video keyframes) without decoding.
- Audio: swr → 48k stereo FLT inside; a FIFO with an absolute anchor by the pts of the first decoded frame after a seek. Seek = a container BACKWARD seek to the indexed packet (with a margin of 3 packets) + a FULL re-creation of the decoder and swr. Sample accuracy is confirmed by the conformance test: FLAC — exact, AAC — zero phase shift (lossy values are compared by correlation).
- Video: `threads=1` for VP9/AV1 (a legacy of C++: MT decoding of these codecs is unstable under frequent seeks); a backward seek = seek + re-creation of the decoder ALWAYS (it covers MT H.264, and is cheaper than catching artifacts); forward ≤4 s — a continuous decode without seek. sws → RGBA with the source's own colour matrix and range (source-colour-matrix), the context cache by (size, source format, matrix, range). Kept thumbs carry a pixel version (`THUMB_PIXELS_VERSION`, the version of their kinds; the preview's frames on disk carry it too): move it whenever the decode's pixels change for the same file; `put_thumb_size` goes BEFORE `put_thumbnail` and drops the older ones (t2894).
- `unsafe impl Send` is correct, because an instance is owned by exactly one MediaWorker (&mut self); Sync is deliberately not implemented.
- The packet index is kept in the media index: `FfmpegSource::open_indexed(path, store)` takes it from there when the file is unchanged (`ensure_media`) and skips the full scan pass; after a fresh scan — it saves it. Its errors = cache errors: a silent fallback to the ordinary open. Test 34.
- The decoding layer implementation — the crate kt-ffmpeg (dlopen of the system FFmpeg 6/7/8/9, without linking); rsmpeg is removed.

## Decoders of one file (export-throughput D9, preview-throughput D1)

- **Every video read of a file goes through `read_runs`** (decode-waste): a frame, a span at one size,
  a span at several sizes. A run starts at the decoder that reaches its first key by decoding on (the
  nearest) and goes on while that decoder, standing at the run's last key, is still the nearest that
  reaches the next (`goes_on`, `kt_ffmpeg::Source::video_reach_from`) — never a fixed gap: keys of two
  places 1.5 s apart were one run, one decoder walked between them and sought back every chunk (t2880).
  Every key goes through `prepare_for` (a one-size span sought only before its first key, t2883).
- **A fresh decoder opens only when none is free to go** (`cursor_for`): each stands ahead of the key, has
  read a key of this request, or is the one a later key of the request would be given. Otherwise the free
  one standing longest unused goes (a key ahead of it is a seek forward: a filmstrip keeps one decoder,
  t2884), and the one a later key would be given goes last (t2882). Decoders per file follow the picture
  (`cursors_for_size`: two 4K pictures' pixels, 2..=`MAX_CURSORS` 4, t2881) — a decoder's frame pool
  grows with its picture. The choice is a function of the requests, never of timing; a frame does not
  depend on the decoder that read it. Every extra decoder is opened with the packet index the first
  already has and lives inside the source, on its owner's thread (invariant 5).
- What remains of a dense edit's decode is each cut's lead-in from its keyframe, walked again by cuts far
  apart on the timeline (found-defects): only kept pictures or proxies remove it.

- **`frame_alloc::return_large_frames_to_os()` is the binaries' FIRST statement** (kt-cli, kt-app;
  export-throughput D11, t1458): glibc's dynamic mmap threshold let freed 8–33 MB frames pile up in
  per-thread arenas — the pipelined export peaked at 16.2 GB, 7.3 GB with the fixed 1 MiB threshold.
  A new binary that decodes frames on several threads calls it too.
