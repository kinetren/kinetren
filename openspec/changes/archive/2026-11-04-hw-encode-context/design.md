# Design

## Context

See proposal.md — Why. Observed in the tree (2026-10-28):

- **The cascade** (`src/libs/kt-ffmpeg/src/encoders.rs:72-90`): Linux
  `nvenc → vaapi → qsv → libx264 → mpeg4`, every candidate with a `PixFmt` (`Yuv420p`/`Nv12`);
  `h264_vaapi` is declared `Yuv420p`. `Muxer::open` (`src/libs/kt-ffmpeg/src/muxer_impl.rs:120-196`)
  allocates a context per candidate, sets `pix_fmt` to that software format, calls `avcodec_open2`
  with the FFmpeg log at FATAL, frees it on failure and keeps only the last error text. Nothing in
  the crate creates a device or a frames context; `ctx.hw_frames_ctx` is never set.
- **Frames**: one software `vframe` in the chosen format, filled by `sws_scale` from RGBA
  (`muxer_impl.rs:266-289`), sent directly (`encode`, `:310`).
- **Symbols**: `declare_api!` (`src/libs/kt-ffmpeg/src/api.rs:13-32`) resolves every function with
  `?` — one missing name fails `Api::load` and with it the whole FFmpeg load.
- **Types**: `build.rs` binds 8 headers (`:12-21`), not `libavutil/hwcontext.h`; that header and
  `hwcontext_vaapi.h` are vendored for all four majors (`vendor/ffmpeg-{6,7,8,9}/include/libavutil/`).
- **The UI list** `available_encoders()` filters only by `avcodec_find_encoder_by_name`
  (`muxer_impl.rs:28-37`), so `h264_vaapi` is offered on every machine whose FFmpeg has it.
- **The F-EXP-004 measurement**: with the fields `Muxer::open` sets, `h264_vaapi` and `h264_qsv`
  returned −22 before touching a device; `ffmpeg -h encoder=h264_vaapi` lists "Supported pixel
  formats: vaapi". Since windows-build `h264_qsv` gets NV12; whether it opens with NV12 on Linux
  was not measured.
- **T16** ("if you choose h264_vaapi the app just crashes at render", ktprompts v26.10 item 16):
  the code path for a forced `h264_vaapi` is "alloc context → `avcodec_open2` fails → free context →
  `FfError::Av`", which should be an error in the window, not a crash. **The root cause is NOT
  proven.** Candidates: (H1) FFmpeg's VAAPI encoder close after a failed init dereferences state
  that init never set, on some major; (H2) the libva driver loaded during the attempt (the
  machine's iHD/radeonsi/nvidia-vaapi) aborts; (H3) the crash is elsewhere in the GUI render path and
  only coincides with the choice.
- **This development machine has no render node**: `/dev/dri` holds only `card0` of a QEMU
  standard VGA (`1234:1111`); FFmpeg 6.1.1 with `h264_vaapi`, `h264_qsv`, `h264_nvenc`. The VAAPI
  encode can only be proven on the owner's machine; CI runners have no GPU either.

## Goals / Non-Goals

**Goals:**
- `h264_vaapi` opens and encodes where the machine's VAAPI driver can encode H.264.
- A hardware candidate that cannot work here never reaches `avcodec_open2` when the reason is
  knowable before it (no device, no functions) — and never ends the process.
- One binary for FFmpeg 6/7/8/9 still; FFmpeg builds without `hwcontext` keep working.

**Non-Goals:**
- Zero-copy from the GPU composite to VAAPI surfaces (wgpu → dmabuf → VA surface): wgpu lives only
  in kt-gpu, and the export's one composition path (invariant 7) hands RGBA bytes to the muxer; the
  extra copy costs far less than the encode it enables.
- 10-bit (`P010`, HEVC Main10), HEVC/AV1 hardware encoders, new containers — the pipeline is 8-bit
  H.264 (T18 is a separate item).
- Hardware DECODE of sources (VAAPI/NVDEC for playback) — another capability and another change.
- Choosing a device in the UI: the environment variable is the only override for now.

## Decisions

### D1. First prove the crash, then fix (stage 1)
Before any code: run `kt-cli export <fixture> out.mp4 h264_vaapi` under `gdb -batch -ex run -ex bt`
(1) on this VM (no render node), (2) on the owner's machine with its GPU, (3) inside the
`ffmpeg-test` container against FFmpeg 9; and the same through the GUI with the live rig. Record
the signal, the top frames and the FFmpeg major per run in `tasks.md`. The fix of D3 ("skip before
open") removes H1 by construction; H2 is removed only if the device step fails cleanly; H3 needs its
own fix and would be named there. t962 is the regression test of whatever the probe shows.

### D2. Each candidate declares its context: `HwKind`
`EncoderSpec` gains `ctx: HwKind` with `None` (system frames in `pix`) or `Vaapi` (device + VAAPI
frames context, software format NV12). `h264_vaapi` becomes `Vaapi`/`Nv12`; QSV stays `None`/`Nv12`
until the stage-1 measurement says otherwise (if `h264_qsv` with NV12 system frames still does not
open on an Intel machine, it gets `Qsv` derived from the VAAPI device on Linux and from D3D11VA on
Windows — that is a recorded revision of this decision, not a silent addition). AMF and MF accept
system-memory frames; no D3D11 device is created for them. Pure data, so t960 runs on any OS.

### D3. A hardware candidate is one step: device → frames → open
In `Muxer::open`, for a `Vaapi` candidate: (a) device: `KINETREN_VAAPI_DEVICE` if set, otherwise
the render nodes `renderD128…135` in order — `av_hwdevice_ctx_create(VAAPI, node)` for each until
one succeeds (the FFmpeg log stays at FATAL as for the probes); none → skip with "no VAAPI device"
and do NOT allocate the encoder context; (b) `av_hwframe_ctx_alloc` + fields `format = VAAPI`,
`sw_format = NV12`, `width/height` = the export size, `initial_pool_size = 20` +
`av_hwframe_ctx_init`; failure → unref the device, skip with "frames context: code N"; (c) the
encoder context with `pix_fmt = VAAPI`, `hw_frames_ctx = av_buffer_ref(frames)`, `avcodec_open2`;
failure → free context, unref frames and device, skip with "encoder: code N". The cascade order is
unchanged, so on a machine where NVENC opens nothing changes. The device is per export (a short-lived
resource of the offline job, invariant 5's offline rule): no cache of a device "verdict".
Alternative: `av_hwdevice_ctx_create(…, NULL)` (libva's default) — on a two-GPU machine it picks
whatever libva prefers, with no way to name or report it.

### D4. Upload per frame, same pts and duration
The muxer keeps the NV12 software `vframe` it has today, plus a reusable `hwframe`. Per frame:
`sws_scale` RGBA → NV12 into `vframe`, `av_hwframe_get_buffer(frames, hwframe, 0)`,
`av_hwframe_transfer_data(hwframe, vframe, 0)`, copy `pts` and `duration` (the ffmpeg-9 lesson:
every frame carries its duration), send `hwframe`, `av_frame_unref(hwframe)` after the send. The
flush and packet path are unchanged. `Drop` order: frames → encoder context → frames context → device.

### D5. Optional symbols: a second table, `HwApi`
`declare_api!` stays for the required table. A new `declare_optional_api!` builds
`HwApi { av_hwdevice_ctx_create, … }` with `load(libs) -> Result<HwApi, MissingSymbol>`; the
version module keeps `Option<&'static HwApi>` next to `Api` plus the reason when absent. All the
functions are in libavutil and exist in 6–9 (FFmpeg ≥ 3); the table is optional because a custom
`--disable-…` build or a future major may drop them, and invariant 10 is "load what the machine
has". `build.rs` adds `libavutil/hwcontext.h` (and `hwcontext_vaapi.h` only if a VA struct is
needed — it is not for D3) to `HEADERS`; t961 compares each module's enum values with its headers,
as t862 does for the majors. The test seam is a resolver closure (`load_with(|name| …)`), so t961
needs no FFmpeg.

### D6. The list and the report open devices, not encoders
`available_encoders()` returns `(name, device: Option<String>)`: `h264_vaapi` appears only with a
device that opened in D3(a) (cheap: no encoder init). `kt_ffmpeg::hw_report()` returns the
section of the media-index requirement; `kt-cli ffmpeg` prints it; `--probe-encoders` runs the
export's own `open` code (D3) for each hardware candidate at 1280×720 into a null output and prints
the outcome — the one command the Flatpak guide gives (`flatpak run --command=kt-cli … ffmpeg
--probe-encoders`). `KT_DRI_DIR` replaces `/dev/dri` in tests (t964).

### D7. Quality settings stay per encoder
`h264_vaapi` keeps `qp` (constant quantizer, F-EXP-004 trick 3) for the quality mode; for the
bitrate mode it gets `rc_mode = VBR` with the context's `bit_rate` (today it has no bitrate options,
so the bitrate mode silently used the driver's default). The render window names the knob "QP"
(change `render-window-v1-parity`).

### D8. The proof on hardware is opt-in and loud: `KT_EXPECT_HW=vaapi`, `make hw-test`
t963 runs only when `KT_EXPECT_HW=vaapi` is set, and then fails unless VAAPI opened (the t865
pattern). `make hw-test` runs the FFmpeg tests on the host with that variable. The decode bound for
the mean absolute difference is set from the first run on the owner's machine and written here
(starting proposal: ≤ 6 of 255 per channel at QP 23 for the synthetic ramp). Revised 2026-11-04:
that first run will not come, so the bound stays the proposal's 6, unmeasured on hardware. t1556 runs
t963's own body on the fake driver (D11), which is lossless to NV12, and holds it to 3.

### D9. As built (stages 1–4, 2026-10-29)
- **The probe on two machines without a render node did not crash** (task 1.1): FFmpeg 6.1.1 gives
  −22 for a forced `h264_vaapi`; the FFmpeg 9 test build has no `h264_vaapi`, and the error then
  carried an EMPTY reason ("last error: ") — now "not in this FFmpeg build". T16's cause (H1/H2)
  is still to be seen on a machine with a render node.
- **The test seam is thread-local** (`kt_ffmpeg::hw_seam`: device, `/dev/dri`, a missing function):
  the export tests run in one process in parallel, and `KINETREN_VAAPI_DEVICE` would steer every
  export of the binary. Processes (kt-cli) keep the variables, `KT_DRI_DIR` included.
- **The settings list is asked once per window**: listing a device candidate opens its device, and
  the combo used to call `available_encoders()` every frame (harmless while it was a name lookup).
- **The skipped candidates are their own log fact** (`ExportLog::Skipped`), separate from
  `NotOpened`: "the encoder was never opened" is exactly what t962 asserts.

### D10. A libva driver first meets a throwaway process (`hw_trial`)
D3 removes the failed-init path only when there is no device. With a render node the driver is
loaded into the process, and D3 made that NEW for every Auto export on Intel and AMD: until this
change Auto got −22 and never touched a driver, so a driver that faults would have taken from them
the render that worked. T16's cause could not be proven either way (no machine here has a render
node, and the owner does not take measurements), and H1 and H2 both fault inside the process, where
no return code reaches the cascade. So the cause is contained rather than argued about:
- a candidate with `trial` (`h264_vaapi`, Linux `h264_qsv`) is first opened by OUR OWN binary started
  again (`--kt-hw-trial`, served by `hw_trial::serve()` at the top of `main` of kt-app and kt-cli, before the supervisor):
  device, frames, encoder, three frames into `/dev/null`, flush. Only `ok` lets the driver into the
  render process. A signal, a time limit (20 s; 5 s for a device, asked on the UI thread) or a
  refusal is a skip with the reason. Forced, that reason is the export error. It is also why a
  driver that opens and cannot encode falls down the cascade instead of failing the first frame.
- `available_encoders` and `hw_report` ask the same way, one node at a time: the render window's
  list and the report that diagnoses a bad driver must outlive it.
- No render node → no trial: nothing to load, no process started. Every export asks again, nothing
  is remembered (the spec's "no cached verdict"). The cost is one process start per export on a
  machine with a render node, which only the hardware path pays.
- A process that does not host the trial (a test harness) opens in-process (`Verdict::NotHosted`).
Rejected: `fork` (a threaded process, and libva then runs in a copy of it); a list of known-bad
drivers (it is a cached verdict, wrong the day a driver updates). What it does NOT cover: a driver
that passes three frames and faults later ends the render in the render process; the supervisor's
crash window and the autosave are what is left there.
NVENC, AMF and Media Foundation keep the in-process open: no libva, and no report of a fault.

### D11. The path runs on no hardware: a fake VA driver (`fixtures/fake_va`)
Tasks 1.1, 1.2, 3.3 and 4.3 waited for "the owner's machine"; the owner does not take measurements
(2026-11-04), so nothing had ever executed the surface path. `ktfake_drv_video.c` is a VA driver the
REAL libva loads into the REAL FFmpeg (`LIBVA_DRIVER_NAME`, `LIBVA_DRIVERS_PATH`), with every known
way of failing as one value of `KT_FAKE_VA`; healthy, it is an H.264 encoder that writes I_PCM
macroblocks from the surface behind FFmpeg's packed headers, so the file decodes to exactly what was
uploaded. libva refuses a file that is not a DRM node and CI has none, so the same library is
LD_PRELOADed for `drmGetNodeTypeFromFd` and the "render node" is a regular file. No libva-dev: the
structures mirror the stable ABI by hand.
It proves OUR path through the real libva and FFmpeg 6.1 (open, frames, upload through the pipelined
export, flush, cleanup) and every failure arm. It proves NOTHING about a real driver: whether Intel's
or AMD's opens, its quality, its speed. Those stay unmeasured, and nothing here claims them.

### D2 revised (2026-11-04): QSV keeps system NV12 frames, no device context
Decided without the Intel measurement of task 1.2, which will not come. `ffmpeg -c:v h264_qsv` from
system NV12 frames is FFmpeg's documented use: with neither `hw_frames_ctx` nor `hw_device_ctx`
`qsvenc` builds an internal session of its own, and here it reaches exactly that step ("Error
creating a MFX session: -9", no device). A QSV device derived from VAAPI would only choose the node
on a two-GPU machine, and could not be run anywhere. So the brief's "VAAPI/QSV a device and a frames
context" holds for VAAPI only. On Linux QSV's session loads the same VA driver, hence its `trial`
(D10). NOT measured on an Intel machine; if it does not open there, the cascade reaches libx264.

## Risks / Trade-offs

- [Auto on an Intel/AMD Linux machine changes its encoder from libx264 to VAAPI — worse quality at
  the same number, F-EXP-004 Cons] → it is the owner's request ("video acceleration must work in
  render"); the window names the encoder and its knob; forcing `libx264` stays one click.
- [A driver that opens the device and the encoder but produces broken streams] → t963 decodes
  every frame; a driver fault seen there becomes a known-bad note with the driver string from the
  report.
- [The first open of a VAAPI device costs tens of milliseconds per render node] → done once per
  export and once per settings window open, never per frame or on the UI thread's hot path.
- [The upload doubles memory traffic at 4K] → accepted; measured on release with `kt-cli export
  --time`-style timing in stage 4, judged only on a release build.
- [The crash has a cause D3 does not remove (H3)] → stage 1 proves it first; t962 asserts the
  process survives.

## Migration Plan

Nothing stored changes. Rollback = revert; a machine falls back to libx264 as before.

## Open Questions

1. **Auto order on a machine with both NVIDIA and an Intel iGPU**: NVENC stays first (unchanged
   cascade). Recommended: keep — NVENC is the faster and better encoder of the two.
2. **Expose the VAAPI device choice in the UI?** Recommended: not now — the environment variable and
   the report cover the two-GPU case; a setting can follow if the owner meets it.
3. Closed: D2 revised (2026-11-04), no QSV context.
