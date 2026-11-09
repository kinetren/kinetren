# kt-plugin-api

> The crate's contracts and mandatory tests — `openspec/specs/plugins/spec.md` (and the families'
> capabilities: `video-fx`, `audio-fx`, `transitions`, `generators`, `automation` for the passport);
> the reasons — `openspec/specs/plugins/design.md`.

The plugin contract: a `Manifest` (id, passport rows, declarations, UI keys) plus pure kernels per
family (`VideoKernel`/`GeoKernel`, `AudioKernel`, `TransKernel`, `GenKernel`), and the `Registry`
keyed by `(Family, id)`. Depends on kt-time, serde and libm ONLY: a plugin crate sees nothing of the core.

- **A plugin id is the kind's file tag**, unique within its family (`Registry::get(family, id)`):
  the generator `checker` is not the transition `checker` (t1049). Changing an id changes files.
- **Refusals happen in `Registry::register`, before any frame** (t972): video state, tail or
  look-ahead; a kernel not callable from several threads; an audio look-ahead reaching
  `MIN_PREROLL_FRAMES`; a built-in point-wise kernel without WGSL; a geometry kernel offering WGSL;
  a generator decl naming a row its passport lacks. For a barrier (t1054): no kernel, no GPU
  side as a built-in, a tap's WGSL instead of a graph, a graph reading an unsaved slot, ending off the
  frame size or on a `Save`, a resample above `MAX_INTER_DIM`, a pass naming a missing shader, or an
  empty graph on a frame with alpha. The graph is checked on the defaults and the top, at K 1 and 4.
  A new declaration gets its refusal here too.
- **A `Stream` class is a SOURCE STAGE, not video state** (datamosh-codec-path): its
  `StreamKernel` names the HISTORY frame N reads and renders N as a pure function of it, keeping
  results only in the host-owned `StreamMemo` (bounded, keyed by addresses). Refused (t1240): the
  class without a stream kernel, a stream kernel on another class, any GPU side on a stream plugin.
- **Helpers shared by kernels and the host live here so each has ONE text**: `geo::invert3` and
  `TRANSPARENT`, `trans::Img`, the generator helpers (`smooth`, `rgb`, `coverage`, `lowbias32`…),
  `pixfont` (moved from kt-media).
- **`math` is the ONE math of the frozen paths** (ci-green-again, invariant 11): a kernel, a
  curve, a generator or the engine calls `x.fsin()`/`10f32.fpowf(…)` (`math::FrozenMath`), never
  `x.sin()`/`powf` — those are the PLATFORM's libm, and glibc and the Windows one round
  differently (t973c/t1019/t1048 failed on windows-latest). The `f32` functions are the `f64` ones
  rounded once (correctly rounded but for ~2⁻²⁸ of inputs); t1065 holds them within one ulp of
  glibc, t1066 refuses a direct std call in the frozen crates and every family.
- **A barrier may own a grid and many numbers** (vhs-tape-simulation): `Node::PassAt` renders a
  pass at its own size (checked like a resample, the graph still ends at the frame size), and
  `gpu_values` gives a kernel its passport values when eight cooked numbers do not fit. The host
  calls ONLY the value form; its default cooks, so a kind with ≤ 8 rows keeps its graph. A kernel
  overriding it keeps `gpu` as the cooked shim.
- **An audio state forks** (pitch-tail-choice D4): `AudioState::fork` gives a copy that continues
  exactly as the state would on the same input — the host splits the chain up to a pitch at the
  event's out-point to let the echo before it ring on. Default `None` (the host then keeps that
  echo cut); every built-in implements it by `Clone`, so a new state derives `Clone` and its
  `CurveRead` readers copy through `clone_box`. t1330 runs every built-in audio kind. `Decl.echo_row`
  names the choice row of an edge-reading kind (`reads_across_edges`).
- **`compute` is the door to the host's compute device** (compute-kernels): a kernel gives WGSL
  shaders and buffers as one `Job`, and bindings are fixed (0 the uniform, then reads, then writes).
  `host()` is `None` with no device or after `fail`, and a kernel then has a no-device path of its
  own (Datamosh passes the frame through); a job's error is never a panic. t1502, t1503.
- **A simulated device's two halves are one passport** (sim-audio-pairs): a video kind declares
  its audio pair (`Decl.pair`), the audio kind is built from the SAME passport function; the
  registry refuses a mismatch in either order (t1484) and the assembler refuses a missing half
  (`unpaired`). `Decl.foreign_rows` names the shared rows a half does not read (the picture's
  record circuits on the sound) — t322 skips them. An audio half reads the device at the TIMELINE
  position its picture reads: `AudioState::place` gives it its clip's timeline start and the
  project fps; it derives the state from rows + time, never shares it with the picture.
- **`audio_codec` is the ONE lossy audio codec of the simulations** (sim-audio-pairs): an MDCT
  (2048/1024, sine window, through a 256-point FFT), AAC-Main-like per-line prediction with resets,
  Rice-coded lines, a bit reservoir, a concealing decoder; frozen math, the same bytes everywhere.
  The predictor stays inside its stability triangle and the decoder inside its range: a broken
  stream smears and saturates, it never runs away (fed the wrong history it reached 56 000).
- **An audio state's block runs at the CPU's level** (audio-fx-cost): `process` wraps its body in
  `kt_plugin_api::fast!({ … })` — the same text compiled again with AVX2, taken where the CPU has it,
  the same bytes (t1981 on both paths, `KT_SIMD=0` / `cpu::with_baseline` force the baseline). The
  shipped build is SSE2: there every `floor`/`round`/`trunc`/`fract` is a C-library call, so a
  per-sample helper the body calls is `#[inline]` or it keeps the call. A pure function a per-sample
  loop takes of a smoothed parameter (`10^(dB/20)`) goes through `dsp::Memo` (bits-keyed, exact). A
  cheaper form of a frozen function is that function to the bit and proved on every input
  (`math::powf_neg_half`, t1982). A new kind gets a row in t1980's bounds and t1981's digests
  (`kt-cli fx-cost --digest`, `KT_PRINT_COST=1`).
- **A kernel standing still takes its constant path, to the bit** (kernel-specialization): a re-tune
  only when its inputs' bits move (`Biquad::tune_key`, the EQ's seven values), one set of values for a
  block whose smoothers stand still (`SmoothedParam::still` is a FIXED POINT of the tick, not
  `value == target`: a glide stops ulps short of its target, and −0 ticks to +0; `advance(n)`), a
  computation skipped where its answer is known exactly (the compressor under `below_threshold`). Each
  variant keeps its general path, which `cpu::with_general` forces on a thread (read once per block):
  t2171 holds every kind on the specialized, general and baseline paths to digests printed before, t2176
  checks that a variant is still taken and pays. A new variant gets a row in both.
- **`fuses_in_runs` is the ONE answer to "does this stage fuse into a point-wise run"**
  (fused-pointwise-runs): `PointWise`, `Gpu::Wgsl`, no barrier kernel. A point-wise tap is pasted
  into generated passes, so `register` refuses a WGSL without `fn tap_<id>(` or one that reads a
  texture or the host's `P` (`impure_tap`, t2194); t2195 pins the built-in list.
- The module is `generator`, not `gen`: `gen` is a reserved keyword in Rust 2024.
- Mandatory tests: t972 (`tests/registry.rs`) and every family's frozen-form test (t973,
  t1048) when a row or a declaration changes.
- **A kernel that runs a compute shader names it** (pipeline-prewarm D6): `Plugin::compute_shaders`
  (Datamosh), `Kind::with_shader` for a generator drawn by `shader_frame` — the same constant the frame
  passes. An unnamed shader compiles inside the job that first needs it; t2123 runs every generator kind.
