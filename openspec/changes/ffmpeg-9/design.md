# Design: FFmpeg 9

## Context

See proposal.md, Why. The current state that shapes the approach:

- **Types per major, functions by `dlsym`** (`src/libs/kt-ffmpeg/build.rs`, `src/lib.rs`).
  `build.rs` runs bindgen over `vendor/ffmpeg-{6,7,8}/include` for types and constants only,
  with the cargo `TARGET`. `version_module!` instantiates the same `ffmpeg_api!` function list,
  `source_impl.rs` and `muxer_impl.rs` once per major (`v6`, `v7`, `v8`), so every struct
  access compiles against that major's layout. `Backend`, `Source` and `Muxer` dispatch with one
  arm per major. That is about 20 arms in `lib.rs`.
- **A latent hazard in the dispatch.** `rescan_with` matches `8 => V8, 7 => V7, _ => V6`. Today
  the loader never returns anything but 6/7/8, so the catch-all is harmless. If a major got into
  the search table without a module, the catch-all would bind FFmpeg-6 layouts to another
  major's libraries, which is memory corruption and not an error message.
- **Search plan** (`src/search.rs`): `MAJORS` is the one table of library majors per release,
  newest first. `judge()`, `file_name()`, the Linux SONAME candidates and `Reject::Unsupported`
  all derive from it. `ffmpeg_of_avcodec()` names releases newer than the table
  (`avcodec - 54`), so FFmpeg 10 is reported by name with no extra code.
- **The headers are a source-tree copy**, not an installed `include/`. Each `vendor/ffmpeg-N`
  holds every `.h` of the five libraries (internal ones too, about 11 MB) plus a hand-written
  `libavutil/avconfig.h`. Nothing records where they came from. The 8 headers are 8.0
  (avcodec 62.11), while the 8.1.x libraries in use are 62.28. Within a major the ABI is
  stable, so older headers of the same major are enough.
- **Download** (`kt-dirs::ffmpeg_download_dir()` = `<local>/ffmpeg/<FFMPEG_DOWNLOAD_VERSION>`)
  is ONE folder, and the Windows plan takes it as `Env::download_dir() -> Option<String>`. If
  only the constant moved, an 8.1.2 download would silently stop being searched.
- **The struct fields and constants our code touches** (from `source_impl.rs`/`muxer_impl.rs`):
  `AVFormatContext`/`AVStream`/`AVCodecParameters`/`AVCodecContext`/`AVFrame`/`AVPacket` core
  fields (time bases, pts, sizes, `ch_layout`, `pkt_timebase`, `disposition`), pixel and sample
  formats, `SWS_BILINEAR`/`SWS_FAST_BILINEAR`, `AV_PKT_FLAG_KEY`, `AV_DISPOSITION_ATTACHED_PIC`,
  `AV_PIX_FMT_FLAG_ALPHA`. The `FF_API_*` guards that the 8 headers set to expire at the 9 bump
  (`INIT_PACKET`, `CODEC_PROPS`, `V408_CODECID`, `COMPUTE_PKT_FIELDS2`, `INTERNAL_TIMING`,
  `OPT_INT_LIST`, `OPT_PTR`, …) name nothing we use. The compiler gives the real answer when
  `v9` builds.
- **Tests that load FFmpeg** run on the CI runner's FFmpeg 6.1 (Linux) and on the 8.1.2 pin
  (Windows job, wine). Nothing has ever loaded 7, 8 or 9 on Linux.

## Goals / Non-Goals

**Goals:**
- A `v9` module equal in standing to `v6`–`v8`. The same source compiles for all four, and any
  divergence is resolved inside the version module.
- Adding a major is a checked act: the table, the modules and the headers cannot disagree.
- A real FFmpeg 9 run on both OSes on every push, which cannot pass by accident on another
  major.

**Non-Goals:**
- No new FFmpeg 9 features (`avcodec_receive_frame_flags`, the new swscale backend, stream
  groups). We keep our current calls.
- No CI runs for FFmpeg 7 and 8 on Linux, and no second Windows run for 8.1.2 (the owner chose
  "CI tests 9 only").
- FFmpeg 6/7/8 stay supported. Nothing is dropped.
- No refactor of the dispatch into a macro. Four majors are still readable as plain arms.

## Decisions

### D1. Vendor the 9.0.2 headers from the pinned source tarball, by a script

`src/scripts/vendor-ffmpeg-headers.sh <version> <sha256>` downloads
`https://ffmpeg.org/releases/ffmpeg-<version>.tar.xz` and checks the SHA-256. It copies every
`.h` of `libavutil`, `libswresample`, `libswscale`, `libavcodec`, `libavformat` into
`vendor/ffmpeg-9/include/<lib>/`, which is the layout of the other three, and writes the same
hand-made `avconfig.h` (AV_HAVE_BIGENDIAN 0, AV_HAVE_FAST_UNALIGNED 1). It writes
`vendor/ffmpeg-9/SOURCE` with the version and hash. The `ffmpeg-test` Docker stage (D5)
downloads the same tarball with the same hash, so the headers that compile `v9` and the libraries
that test it come from one file.
*Alternatives:* copying an installed `include/` would leave out the internal headers that
`avcodec.h` includes on some paths, and the layout would differ from 6–8. Hand-copying is how
6–8 were made, and it left no record of their origin. 6–8 are not re-vendored here; they get no
`SOURCE` file. That is recorded as known and not worth a rebuild of working modules.

### D2. One row in `MAJORS`, one module, one arm per enum, and an exhaustive dispatch

- `build.rs`: `MAJORS = ["6", "7", "8", "9"]`.
- `lib.rs`: `version_module!(v9, "types_v9.rs", 9)`, `Backend::V9`, `Source::V9`, `Muxer::V9`,
  and the matching arms.
- `search.rs`: the row `(9, Majors { avutil: 61, swresample: 7, swscale: 10, avcodec: 63,
  avformat: 63 })` goes first, since the table is newest first and that order is the priority
  inside a folder and on Linux.
- **`rescan_with` gets an exhaustive match: `9 | 8 | 7 | 6 => …`, otherwise an error naming the
  major.** It no longer uses `_ => V6`. A table row without a module then fails loudly.
- **t862** compares each `MAJORS` row with `vN::ffi::LIB*_VERSION_MAJOR`. bindgen already
  allowlists `LIB.*_VERSION_MAJOR`. It also checks that the set of modules equals the set of
  rows, through a `pub const VERSION_MODULES: [u32; N]` next to the `version_module!` calls, so
  forgetting either side breaks the test.
- If FFmpeg 9 removed a constant or field that `source_impl.rs`/`muxer_impl.rs` uses, the
  difference goes into the version module. Usually that is a small `compat` item defined per
  `version_module!` invocation (the macro gets the major as a literal already), never a runtime
  `if major >= 9` in shared code.

*Alternatives:* a `dispatch!` macro over all enums would cut repetition, but the change would
then touch every call site for no behaviour. A generic "any major ≥ 6 with the newest types"
would break the reason the modules exist: struct layouts differ between majors.

### D3. Earlier downloads stay searchable: the download ROOT, newest version first

- `kt-dirs` keeps `FFMPEG_DOWNLOAD_VERSION` (now `"9.0.2"`) and `ffmpeg_download_dir()` (the
  folder the Download button writes). It adds `ffmpeg_download_root()` (`<local>/ffmpeg`).
- `search::Env::download_dir()` becomes `download_root()`. The plan lists the root, keeps
  entries that parse as a dotted version (`9.0.2`, `8.1.2`), sorts them by version, newest first,
  and pushes each as a `Source::Download` candidate. Leftovers (`.partial-<pid>`) and other names
  are not candidates and are not reported, because they are the downloader's own temporary
  entries. The pinned version is the newest in the root, so it comes first, and an older
  download is the fallback.
- Nothing deletes an old version folder. The downloader's own stale-temp cleanup
  (`STALE_AFTER`) only touches temporary entries, as now.
- kt-app and kt-cli pass the root instead of the version folder. The FFmpeg window keeps
  offering **Download FFmpeg 9.0.2** while `9.0.2\` is absent, even if an older download is
  loaded.

*Alternatives:* migrate (rename `8.1.2\` to `9.0.2\`), which would be a lie about the content.
Delete the old folder after a new download, which is risky while its DLLs are loaded and which
the person did not ask for. Keep the root's newest folder only, which would make a broken `9.0.2\`
hide a working `8.1.2\`.

### D4. Windows pin 8.1.2 → 9.0.2, one pin in three places, guarded

The pin is gyan.dev `ffmpeg-9.0.2-full_build-shared.7z` plus the GyanD/codexffmpeg GitHub
mirror. The SHA-256 and the size are measured once when the task is applied: both URLs are
downloaded and must be byte-identical, as was checked for 8.1.2. The pin lives in
`ffmpeg_setup.rs` (`PINNED_*`), `kt-dirs::FFMPEG_DOWNLOAD_VERSION`,
`src/scripts/fetch-ffmpeg-win.sh` and the windows-latest job of `ci.yml`. `windows-test.sh`,
`loader_windows.rs` (t846) and `reach_tests.rs` (t850) take the version folder
`target/ffmpeg-win/9.0.2`. t860 gets the check that the version and hash in `ci.yml`, the fetch
script and `ffmpeg_setup.rs` are equal. Before this change nothing kept the three in step.
The hints change too: `winget install Gyan.FFmpeg.Shared --version 9.0.2`, and the
`scoop … ffmpeg-shared@9.0.2` line in `README-windows.txt`, both verified to exist when applied.

### D5. Linux proof: the `ffmpeg-test` stage, the script, the CI job and the expected major

- **Stage** `FROM base AS ffmpeg-test` in `src/packaging/Dockerfile`, with
  `ARG FFMPEG_VERSION=9.0.2` and `ARG FFMPEG_SHA256=…`. It runs
  `apt-get install nasm libx264-dev` (jammy: nasm 2.15, x264 0.163), downloads and checks the
  tarball, and runs `./configure --prefix=/opt/ffmpeg --enable-shared --disable-static
  --disable-programs --disable-doc --enable-gpl --enable-libx264`. No other external library is
  needed: the fixtures are FLAC/AAC/H.264/VP9/PNG, all native decoders, and the encoders are
  libx264 → mpeg4 + native AAC. Then `make -j"$(nproc)" install` and nextest (the same version as
  the `windows` stage). The libraries land in `/opt/ffmpeg/lib`. The stage is never the last one,
  so `docker build` without `--target` still produces `linux`. `release-build.sh` never names it.
- **Script** `src/scripts/ffmpeg-test.sh` (`make ffmpeg-test`) mirrors `windows-test.sh`. It
  builds the stage, generates the fixtures on the host if they are missing, and mounts the
  repository at `/kt` with a separate `CARGO_TARGET_DIR=/kt/src/target/ffmpeg-test-container`.
  It runs `cargo nextest run -p kt-ffmpeg -p kt-media -p kt-engine -p kt-cli` with
  `KINETREN_FFMPEG_DIR=/opt/ffmpeg/lib`, `KINETREN_FFMPEG_SYSTEM=0` and
  `KT_EXPECT_FFMPEG_MAJOR=9`. The tests run inside the container, not on the host, because the
  libraries link the container's `libx264.so.163`. Ubuntu 24.04 has `.164`, so copying them out
  would fail to load.
- **CI job** `ffmpeg9` on `ubuntu-24.04`: checkout, fixtures, then the same script. The job
  gets the docker layer cache through `actions/cache` on a `docker save` keyed by the
  Dockerfile's hash, so the FFmpeg compile (about 5–8 minutes on the runner) happens only when
  the recipe changes. The job uploads only a failure log with `retention-days: 3`.
- **t865**: with `KT_EXPECT_FFMPEG_MAJOR=N` set, the test loads through the normal plan and
  asserts `BackendInfo.major == N`. On mismatch or failure it prints the full search report.
  Unset, it skips with a printed reason, like t846/t847. Because it sits in `kt-ffmpeg`, which
  the script always selects, a run where the folder silently did not load fails instead of
  passing on skips.
- **t866** (dist-check) guards the shape: the stage exists and checks the hash, the vendoring
  script's pin equals the Dockerfile ARGs, the `make` target exists, the job exists and sets the
  expected major, and `release-build.sh` does not reference the stage.

*Alternatives:* BtbN prebuilt shared builds are git snapshots (`n9.0.2-36-g…`), and their dated
releases get pruned, so they cannot be pinned by hash. A distribution image with FFmpeg 9 (Arch,
Debian unstable) is not reproducible: the version moves under the hash. The owner chose the
source build.

### D6. Wording and docs follow the table

`not_found_hint` says "FFmpeg 6, 7, 8 or 9", the README badge reads 6 | 7 | 8 | 9, and invariant 10
reads "6/7/8/9" in the root `CLAUDE.md` and `openspec/config.yaml`. The same update goes to
`docs/architecture.md`, `CONTRIBUTING.md`, the crate `CLAUDE.md` of kt-ffmpeg (the "FFmpeg 9 is
not supported yet" bullet is replaced by "adding a major: D1+D2 checklist") and kt-media, and
the Dockerfile and release-build comments. The `docs/features/` notes that quote "6/7/8" as the
state of the project at the time they were written are history and stay as they are. Only the
verdict of `resilience/environment/system-libraries-and-drivers.md` is updated after archiving.

## Risks / Trade-offs

- [FFmpeg 9 changed a field or constant we use] → the `v9` module does not compile. That is the
  intended early signal. The fix stays in the version module (D2). If a behaviour changed
  without an API change (for example `FF_API_COMPUTE_PKT_FIELDS2`: muxers no longer fill missing
  packet fields), t33 and the export tests in the FFmpeg 9 run catch it. Our muxer already sets
  pts/dts/duration from the encoder.
- [Decoded pixels differ between majors (swscale 10, decoder fixes)] → the conformance tests
  compare decodes with each other (seek against linear, span against per-frame), not against
  hashes of a major, and the golden export is synthetic. If any test turns out to hash
  FFmpeg-decoded output, the FFmpeg 9 run shows it. It is reported and decided with the owner,
  and the reference is never re-baselined silently.
- [CI time and flakiness of a source build] → the layer cache keyed by the Dockerfile means a
  rebuild happens only on a recipe change. The tarball comes from ffmpeg.org with a hash. A
  download failure is a red job with a named cause, not a skip.
- [The 9.0.2 gyan archive is larger or structured differently] → t848's fixture archive mirrors
  the real structure. The size and both mirrors are measured when the task is applied, before
  the constant is committed.
- [Windows 8.x stops being exercised on a real runner] → this is the owner's trade-off. The pure
  plan tests (t844/t845/t864) still check names, order and the earlier-download fallback.
- [Linux machines with both 8 and 9 installed during a distribution transition] → 9 wins by
  table order. That is the same rule that made 8 win over 7, and `KINETREN_FFMPEG_DIR` overrides
  it.

## Migration Plan

Users need no action. On Windows, an existing `…\ffmpeg\8.1.2\` keeps loading (D3), and Download
offers 9.0.2. Rollback is reverting the change: the 8.1.2 folder was never touched, and a 9.0.2
folder is then simply ignored by an older binary, which rejects it as "FFmpeg 9 not supported
yet".

## Open Questions

- The exact SHA-256 values (`ffmpeg-9.0.2.tar.xz`, `ffmpeg-9.0.2-full_build-shared.7z`) and the
  archive size are measured at apply time (tasks 1.1 and 4.1). They do not change the plan.
