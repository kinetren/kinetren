---
id: F-RES-008
title: Installation and packaging: an application that did not start
status: candidate
priority: A
cost: M
category: resilience/environment
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, blender-5.2]
we_have: partial
crates: [kt-ffmpeg, kt-media, kt-app, kt-cli]
invariants: [9, 10]
ui_rules: [16, 19]
round: null
point: null
tests: []
depends_on: []
trackers: [kde-bugzilla, kde-invent, pitivi]
checked: 2026-09-08
---

# F-RES-008 — Installation and packaging: an application that did not start

## Essence

The editor downloads a file, unpacks it, clicks — and either it works, or it does not. If it does not,
they will not become an editor at all: not one feature of this base is worth anything on a machine
where the program does not open or does not open files. This is not "infrastructure" but
the first screen of the product. Separately and more harshly: building FROM SOURCES is the developer's problem, installing a READY build is the user's problem, and in the trackers these two
classes live in different places and are fixed differently.

## Who has it and how

| Editor | Version | Called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | installer + Boris FX Hub | Windows-only `.exe`, online activation; the question "on which distribution" does not arise at all |
| Adobe Premiere | 26.x | Creative Cloud Desktop | installed only by the subscription manager, which also keeps several versions side by side and rolls back |
| DaVinci Resolve | 21 | `.run`/`.dmg`/`.exe` from the vendor | a self-extracting archive into `/opt`, carries ITS OWN libraries, but requires a system GPU driver of the needed version |
| Final Cut Pro | 12.3 | Mac App Store | delivery, updates and the sandbox are done by the OS; the user has no packaging options |
| After Effects | 26.x | Creative Cloud Desktop | the same as Premiere: the only channel is the subscription manager |

**Outside the core.** Without open source this feature cannot be described: all five
core editors have exactly one channel, chosen by the vendor, and the fork "distribution versus a
self-contained package" simply does not exist for them — yet all the pain and all "the trick behind it" live
precisely in this fork.

| Program | Version | Called | How it works |
|---|---|---|---|
| Blender | 5.2 LTS | official tar.xz | a self-contained directory without installation, carries its own Python and a Qt-independent UI, but takes the system GPU driver — exactly the middle that we too have chosen |

Kdenlive, Pitivi and other open source do NOT get rows here: they entered the base as sources
(`_trackers.md`), not as comparison editors.

## The trick behind it

**A check for the ABSENCE of linking is a rare and therefore valuable kind of check.**
`src/scripts/release-build.sh` before building the tarball runs
`ldd kt-app | grep -q libav` and fails if anything is found. An ordinary build check makes sure that what is needed IS there; this one — that nothing superfluous is there. The point is that the promise "we load the system FFmpeg at runtime" breaks not by editing the loader but by accident: someone's `*-sys` crate in the dependency tree enables a feature, the binary silently links against the `libavcodec.so` of the build machine and continues to work perfectly
on the developer's machine — and breaks exactly where the major version differs. This happened to Kdenlive literally: the AppImage "suddenly" required `libOpenGL` starting from 24.02
(bug 485861), and the report died with the note UNMAINTAINED.

**The glibc symbol ceiling decides on how many distributions the binary will run, and it is counted by `objdump`, not by hope.** The script takes the maximum version of
`GLIBC_*` from the symbol table and refuses to make the tarball if it is above 2.35.
The number is checkable and right now shows why the guard is needed: our local
`target/debug/kt-app` requires `GLIBC_2.39`, that is, on Ubuntu 22.04 it would not
start at all — while the container build requires ≤ 2.35. The difference between "works for me"
and "works for him" here is exactly one line, `objdump | sort -V | tail -1`.

**The reverse side, and it is fundamental: "works everywhere" and "uses the hardware" are opposite goals.** A self-contained package carries its own FFmpeg,
built without the nvenc/AMF/MFX headers, and its own libva, which does not match
the host's driver; therefore for Kdenlive "VAAPI is unavailable in the AppImage" is not a one-off
bug but a series six years long (bugs 419954 → 420482 → 480204 → 510350 → 511169). The more hermetic the packaging, the less of the user's hardware is in it.

## Pros

- The binary is not linked with libav* (invariant 10), therefore the same file
  works with the system FFmpeg 6, 7 and 8 — and uses ALL the hardware acceleration
  of this system FFmpeg, not the ones the packager managed to compile in. The class
  "there is no nvenc in the package" is not expressible for us by construction.
- An update of the system FFmpeg does not break the delivery: the loader
  (`src/libs/kt-ffmpeg/src/loader.rs`) iterates over SONAMEs from new majors to
  old ones and checks `avcodec_version()`, rather than relying on a hard NEEDED.
- The lower bound of the system is set by a number (glibc 2.35 = Ubuntu 22.04) and is checked
  by a machine, not by an oral agreement "build on an old one".
- Installation goes into `~/.local` without root (`src/packaging/install.sh`): there is no class
  "broke the system with the package manager", no file conflict with someone else's package.
- Export is in-process (invariant 9): there is no second executable
  that has to be found, delivered and versioned separately. At Kdenlive the search for
  `melt.exe` is a separate source of reports (bug 385046).

## Cons

- **We exchanged one dependency for another.** Without a system FFmpeg 6/7/8 the program starts normally and silently does NOT open A SINGLE file: every clip
  turns into a `PlaceholderSource`, and the render dialog says "unavailable
  sources (N)". The diagnosis is wrong: the files are in place, the loader is missing.
- `kt_ffmpeg::backend_error()` and `backend_info()` were written for exactly this case and
  are **called from nowhere**: neither kt-app nor kt-cli asks what was loaded. The error text "FFmpeg not found on this system" exists but never reaches the user as such — he sees it smeared across the list of
  "unavailable sources".
- An exotic FFmpeg (major 9, a custom build, a trimmed set of SONAMEs) fails `try_candidate` as a whole: we have neither a list of "what exactly was not found" on the surface, nor a hint of what to install.
- Both delivery guards (ldd and the glibc ceiling) live ONLY in
  `release-build.sh`, which needs docker. `cargo test` knows nothing about them, and an accidental link will pass through all 221 tests in green.
- There is no update channel at all: the tarball is downloaded by hand. We do not create the class "you have a three-year-old version from the repository", but neither does "the program itself will say that a new version is out".

## What users say

SIGNAL: installing a ready-made package — 483 tasks in kde-bugzilla, `install|appimage|flatpak|snap|package|ppa|repository|installer`; share 7.6% versus 3.1% in kde-invent

SIGNAL: building from source — 65 tasks in kde-bugzilla, `build|compile|cmake|gcc|clang|msvc|makefile`; share 1.0% versus 3.1% in kde-invent

The two filters give a mirror-image picture, and this is the main finding: in the
user-facing bugzilla "installed and does not work" occurs 2.5 times more often
than in the developers' tracker, while "does not build" — exactly the opposite, three times less often. The two
classes are not mixed, they simply live in different trackers, and the one in the user-facing tracker matters more. The topic "install/build" as a whole — 729 tasks across three trackers with
a share of real fixes of 0.58, one of the worst (`--themes`).

About "this is the distribution, not us": of the 483 kde-bugzilla tasks, 101 (20.9%) are closed as
INVALID/DOWNSTREAM/UPSTREAM/WONTFIX/UNMAINTAINED, and with a REAL fix — 167 of 446
closed (37%). The share of fixes must be compared honestly only within
kde-bugzilla: the gitlab trackers close without a cause code, and the script is forced to
count such closures as fixes (in kde-invent and pitivi the topic comes out at 1.00 —
an artifact, not an achievement).

> But when I click `check hardware acceleration`, it said: `No hardware encoders
> found.`
> — kde-invent #895, "AppImage ffmpeg does not support hardware acceleration?",
> 27.12.2020, https://invent.kde.org/multimedia/kdenlive/-/work_items/895

The maintainer confirms that this is a property of the delivery model, not a bug:

> We still miss a few dependencies to enable hardware encoding in some parts of
> Kdenlive, so new blueprints should be created.
> — kde-invent #1473, "Add hardware encoding libraries to our CD pipeline",
> 24.06.2022, https://invent.kde.org/multimedia/kdenlive/-/work_items/1473

The reverse side of hermeticity — the hermeticity is incomplete:

> Kdenlive snap (rev 139) breaks after ffmpeg-2404 background update:
> libavformat.so.61 missing
> — kde-bugzilla #521344, 11.06.2026, closed UPSTREAM,
> https://bugs.kde.org/show_bug.cgi?id=521344

This is exactly the class that is structurally impossible for us: we do not require `.so.61` — we search for it. The freshness of the distribution package is also visible in the count: "Old
version in snap" is closed INVALID (bug 422526, 06.06.2020), while the PPA broke because
`libmlt` in it was older than the program itself (bug 406515, 13.04.2019).

## How it fits Kinotrain

ADDRESS: src/scripts/release-build.sh — the class "the package requires libavformat.so.NN, but the system has another" is impossible for us (invariant 10 + `loader.rs`), and this is guarded by `ldd | grep -q libav` right in the build; but the class "the system FFmpeg is missing altogether" is possible, and nothing catches it: `src/libs/kt-media/src/ffmpeg.rs::backend_error` is not called from kt-app or kt-cli, and the loading failure reaches the user disguised as "unavailable sources" (`src/apps/kt-app/src/util.rs::open_source_or_placeholder`, `src/apps/kt-app/src/render_dialog.rs` → `missing_warn`).

### What already exists

- `src/scripts/release-build.sh` — the build of the `dist` profile in a container of
  Ubuntu 22.04, two delivery checks (absence of libav* in `ldd`, a `GLIBC_*` ceiling ≤ 2.35), the tarball in `target/dist-out`, `LATEST` with the version.
- `src/libs/kt-ffmpeg/src/loader.rs` — iteration over three SONAME sets (8/7/6),
  a check of `avcodec_version()`, `RTLD_LOCAL`, accumulation of the list of failures in
  `FfError::NotFound`.
- `src/libs/kt-ffmpeg/src/lib.rs` — `backend_info()` / `backend_error()`:
  the surface for diagnosis is already written.
- `src/packaging/install.sh` + `.desktop` + mime + icons — installation into
  `~/.local` without root, with a fallback to an absolute `Exec=` if `~/.local/bin` is not
  in PATH.
- `src/libs/kt-media/src/placeholder.rs` — an unavailable source does not prevent
  opening, playing and rendering the project (t208–t210).
- The build version `kt-version` (`vYY.MM.DD-hash`) and `scripts/finish.sh`, which
  prints it (item 103 of `docs/porting-notes.md`).

### What is missing

- A startup self-check of the environment: whether FFmpeg loaded, which major, which encoders are visible. The data exists (`available_encoders()` in
  `src/libs/kt-ffmpeg/src/muxer_impl.rs` asks
  `avcodec_find_encoder_by_name`; a real opening check under invariant 9
  lives further on, in the muxer's choice of encoder), but there is no surface.
- One honest message instead of N copies of "source unavailable", when the cause for all N is one and it is not in the sources.
- Moving both delivery guards from the release script to where they always run: today they fire only with `finish.sh --release` and docker.
- Nothing is missing on the update channel, and this is deliberate: see "Verdict".

### What gets in the way

The startup check must not decode anything and must not delay the appearance of the window: `init()` in kt-ffmpeg is lazy, and touching it in the first frame means
a dlopen of five libraries synchronously. The right place is the same background thread that
already warms the index, and the display — under rule 16 (an impossibility = an ERROR, not a faded
hint) and rule 19 (say where to change it: which package to install).
Second: the message about FFmpeg is text with the names of system packages, and those differ in
every distribution; naming an exact name wrongly is worse than naming the library.

### Which invariants it touches

Invariant 10 (the binary is not linked with libav*) — the feature PROTECTS it, and does not touch it.
Invariant 9 (export is in-process, automatic encoder choice with a real opening check) — the data source for the environment report. UI rules 16 and 19.
Invariant 12 (repaint only on movement) — the check must be one-off and background, otherwise it itself becomes a source of frames.

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2–3 stages of the round |
| Where the main work is | kt-app (surface and text), kt-ffmpeg/kt-media (the ready `backend_error`), scripts (moving the guards) |
| Project format | not touched |
| Tests that will have to be written | unit: when `backend_error().is_some()` the message is one and talks about FFmpeg, not about sources; a delivery test: `ldd` without libav* and the `GLIBC_` ceiling are checked outside the release script; a layout test of the environment banner |
| Depends on | nothing; overlaps with F-HLP-026 (what to show in "About") and F-HLP-030 (first launch on someone else's hardware) |
| Breaks compatibility | no |

## Verdict

We take it, priority A. Not because packaging in itself is important, but because
the middle we have chosen — "we do not carry FFmpeg with us, but we do not depend on
the distribution" — wins over both extremes exactly up to the moment when
the system FFmpeg is absent; at that moment today we lie to the user about
"unavailable sources". The expensive part (packaging, guards, dlopen) is already done;
what is missing is one background question "what loaded" and one honest message.

We do NOT take the update channel and do not pull it into this note: we have neither a server nor
a signature, and a self-updating binary is a separate class of defects (see the
series of snap reports above), which we did not create.

Boundaries: the error message as a genre — F-HLP-008; "About" and the composition
of the build — F-HLP-026; first launch on someone else's hardware — F-HLP-030; a missing source as such — F-HLP-012. Here — only what the program DOES when
the environment is wrong.

## Sources

- The registry and the counting method: `docs/features/_trackers.md`; the numbers are reproduced by
  `python3 src/scripts/tracker-rank.py --query '<filter>'` (export date
  08.09.2026). At the time of writing only `kde-bugzilla`,
  `kde-invent` and `pitivi` were in the cache — the other nine trackers of the registry were still being exported,
  so the shares for shotcut/olive/openshot are not given here.
- https://bugs.kde.org/show_bug.cgi?id=419954 — HW acceleration (VAAPI) not
  available in Appimage, closed UPSTREAM (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=420482 — Kdenlive installed via snap
  can't access vaapi (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=480204 — AppImage does not render through Intel
  VAAPI, the non-AppImage does (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=511169 — Default Environment Binaries Do
  Not Allow VAAPI Hardware Acceleration in Appimage Version (2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=525226 — Flatpak: NVIDIA transcode fails
  with 'Unrecognized option vsync' (FFmpeg 9.0.1) (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=485861 — AppImage linked and requires
  libOpenGL as of 24.02, closed UNMAINTAINED (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=521344 — snap breaks after the background
  update of ffmpeg-2404 (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=422526 — Old version in snap, INVALID
  (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=406515 — PPA: libmlt too old
  (accessed 2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/895 and
  https://invent.kde.org/multimedia/kdenlive/-/work_items/1473 — the quotes above
  (accessed 2026-09-08)
- https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2151 — flatpak + libva:
  `va_openDriver() returns -1` inside the sandbox (accessed 2026-09-08)
- Our code was checked by grep on 08.09.2026: `src/scripts/release-build.sh`,
  `src/libs/kt-ffmpeg/src/{loader.rs,lib.rs}`, `src/libs/kt-media/src/ffmpeg.rs`,
  `src/apps/kt-app/src/{util.rs,render_dialog.rs}`, `src/packaging/install.sh`.
  The versions of the core editors — `docs/features/_editors.md` (checked 2026-09-07);
  the delivery channels of Vegas/Premiere/Resolve/FCP/AE were not checked live.
