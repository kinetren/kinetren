---
id: F-EXP-004
title: Choosing a hardware encoder on export
status: already-have
priority: B
cost: S
category: export/pipeline
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08]
we_have: yes
crates: [kt-ffmpeg, kt-engine, kt-app, kt-cli]
invariants: [9, 10, 11]
ui_rules: [4, 16, 19]
round: null
point: null
tests: [25]
depends_on: [F-EXP-001, F-RES-006, F-RES-008]
trackers: [shotcut, kde-bugzilla, kde-invent, openshot, olive, mlt, losslesscut, flowblade, pitivi]
checked: 2026-09-08
---

# F-EXP-004 — Choosing a hardware encoder on export

## Essence

The same hour of editing is encoded either by the processor (slower, better quality
at the same bitrate) or by a block in the video card (several times faster, worse
quality). The editor is presented with this as a choice in the render window: "Auto", "NVENC",
"QuickSync", "Native". The note is not about which encoder is better, but about
**where the list itself comes from and what it equals**: the presence of an encoder in an FFmpeg
build does not mean it will open on this machine, and the price of the difference
arrives at the end of a three-hour render, when it is too late to cancel.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | MAGIX AVC/HEVC render templates with the vendor's name ("…NVENC", "…Intel QSV", "…AMD VCE") | the encoder is part of the NAME OF THE PRESET, not a separate field: no device — no row in the list ("No render template in VEGAS Pro 18 MAGIX AVC/AAC MP4 with Intel QSV" — the title of a request on the vendor's forum). The reverse side: a template is portable between machines together with the nailed-down vendor, and it is broken by a DRIVER CHANGE with the preset unchanged (F-EXP-001, quote andyrpsmith 18.04.2026) [the composition of the items NOT checked live] |
| Adobe Premiere | 26.x | Export → Video → Encoding Settings → **Performance: Hardware Encoding / Software Encoding** | the choice is reduced to TWO items, the vendor is not named at all — "hardware" as a property, not as a codec name. With an incompatible combination of settings the menu switches itself to Software Encoding (VBR 2 pass and 5K/8K h264 do not run in hardware): the choice is not forbidden but silently cancelled [per the Adobe help; `helpx.adobe.com` still does not open from the environment, the data was taken from search results on 08.09.2026] |
| DaVinci Resolve | 21 | Deliver → Advanced Settings → **Encoder: Auto / Native / NVIDIA / Intel QuickSync** | the field appears only for H.264/H.265 and only with a device found; the software path has its own name ("Native"). That is, the list depends on the hardware, and "Auto" is a separate item on a par with the names, not an absence of choice [per user discussions; the BMD forum returns 403] |
| Final Cut Pro | 12.3 | none | there is no choice: one platform, VideoToolbox is applied silently, the question "what to encode with" is not asked of the user. The class "chose an encoder — the render failed" is not expressed here (the same observation — F-RES-006) [NOT VERIFIED] |
| After Effects | 26.x | Output Module → Format H.264 → Format Options → performance | direct export of H.264 from the render queue returned in 2023, and with it the same Hardware/Software switch as in Premiere and AME. AE has no policy of its own: it is the same implementation in three products [NOT VERIFIED against the primary source] |

What all five have in common: **none of them shows what it checked with.** An item in the list
means "we think this is here", and in none of the five can one find out
on what basis it is there.

**Outside the core.** Without Kdenlive and Shotcut there is no note: these are the only two
programs where both halves of the mechanism — the way of DETECTION and its failure —
are visible publicly, as a task with a permanent URL. The row in the table is received by Kdenlive
(the key is in `_editors.md`); Shotcut participates as a TRACKER — the "trick behind it" and the rejected
request for which the note was created are taken from there.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Kdenlive | 26.08.0 | setup wizard → "check hardware acceleration" + render profiles with the vendor's name | the verdict is taken ONCE by a third-party `ffmpeg`, settles in the settings and gates the list of profiles; the render itself goes to a third process (`melt`). The checker, the store and the executor are three different entities, and they can diverge in either direction (analysis — F-RES-006) |

## The trick behind it

**1. The only honest way to know whether an encoder works is to OPEN it;
and exactly this honest way breaks in a self-contained package, because
the probe is run by a process other than the one that later encodes.** Shotcut does not make a list, but
a real trial encoding: `EncodeDock::detectHardwareEncoders` runs
`ffmpeg -f lavfi -i color=s=640x360 -frames 1 -c:v h264_vaapi …` and looks at what comes back.
It cannot be more honest — and this is exactly why the log of the rejected task #1056
shows the whole mechanics: the command goes to
`/tmp/.mount_shotcuUXWOKF/usr/bin/bin/ffmpeg`, that is, to ITS OWN ffmpeg inside the
AppImage, and answers "Failed to initialise VAAPI connection", while `vainfo`
of the host in the same session lists the profiles without a single error. The request hung for
three and a half years and was closed as `not_planned` with the label `expired`
(18.03.2021 → 25.10.2024). A conclusion that is in no list of capabilities:
the probe answers for the FFmpeg that launched it, and in a package that is not the
FFmpeg the user has installed. The class "package against hardware" is already analysed in **F-RES-008**;
here the second half is added — in a hermetic package not only the use of the hardware breaks,
but the DETECTOR itself, and therefore the user is shown an empty list instead of an error.

**2. A list compiled without opening is a promise that gives way at the moment of
rendering, and it can diverge from itself INSIDE one window.** Shotcut #1460
(18.07.2023): the "Use hardware encoder" checkbox with an AV1 preset does nothing;
the "Configure" window shows hardware encoders for h264 and hevc and does not
show av1 — while the manual list contains the same av1 and it works. That is, in one
dialog two lists of "available" live side by side, compiled differently, and they do not
match. The reverse view of the same is kde-invent #2109 (12.12.2025, open): the maintainers
opened a task to keep a TABLE of "does hardware encoding work" along the axes "Linux native / AppImage / Flatpak / Windows" ×
"NVENC / VAAPI / AMF", and half of the cells in it are question marks. When the matrix
cannot be filled by the one who writes the program, the "list of available encoders" in the
render window ceases to be a fact and becomes a genre.

**3. The number in the "quality" field changes its MEANING together with the encoder, and its label is one.** A preset must say
what exactly it selects, because "faster" and "better" are opposite here, and the only digit
with which a person controls quality is called differently for each encoder: `crf` in libx264, `cq` in
nvenc, `global_quality` in qsv, `qp` in vaapi — and the last one is not "constant quality" at all, but a constant quantizer. Hence the complaints that look like
different bugs but are one: openshot #4002 — "Export does not respect crf
setting in 'Bit Rate / Quality' for h264_qsv": two files with crf 10 and crf 35
came out the same size; Shotcut, December 2024 — "Whenever I export a
video with hardware encoders enabled, it makes the bitrate crazy low", with the maintainer's reply
"I fixed this for the next version 25.01 so that you do not
need to enter `vtune=lossless`". Not "the hardware encoder is worse", but "the same
handle is connected to a different regulator", and until the label says which,
any preset promises something other than what it does.

## Pros

- Probing by opening gives the correct answer by construction: an encoder's fitness is
  exactly the success of its initialization, not an entry in a capabilities table. No question
  is asked of the user, no setting is created, nothing can go stale.
- A verdict taken AT THE MOMENT of export by the same process does not survive a change of
  driver, an FFmpeg update or moving the project to another machine — that is, it removes
  entirely the class "the preset describes the author's machine" (F-EXP-001).
- A ladder of candidates sets the policy in ONE line, not as a set of profiles per vendor: Kdenlive's "Improve GPU render profiles" (kde-invent #808,
  open since 13.10.2020, 19 comments) exists precisely because profiles have to be created, maintained and explained, and the ladder has none of the three.
- The name of the actually open encoder reaches the user (`ExportStats.encoder`): he sees with WHAT it was MADE, not with what they intended to use.
- An explicit choice without a fallback is honest: asked for exactly this one — it did not open — an error, not a silent substitution after which the file is made not as ordered.

## Cons

- The probe costs opening the encoder, that is, initializing the driver — it is not free and not instantaneous; therefore it is done once by the wizard, and therefore
  it goes stale for everyone.
- The search stays silent about what it lost: the failure of a hardware candidate does not
  turn into a message, the export simply takes twice as long, and the reason can be learned
  only from the encoder's name in the final summary (F-RES-006 — the same conclusion about our code).
- The hardware block gives worse quality at the same bitrate, and each has its own quality metric, so "switch the encoder" never means "the same
  thing, but faster", although the preset is presented exactly that way.
- The absence of profiles = the absence of a place to write about these differences: where Kdenlive has a profile with a name and a label, the ladder has one line of code.
- A list of five hard candidates is not extended by the user: hevc, av1,
  amf and videotoolbox do not get into it without a code change — exactly what Shotcut #1460 complains about from the other side.

## What users say

SIGNAL: choice of a hardware encoder — 12 tasks in shotcut, `(hardware|hw|gpu)[ -]?encod|nvenc|vaapi|_qsv|h264_amf|videotoolbox`; share 0.9% against 0.4% in kde-bugzilla

SIGNAL: packaging against the detector — 4 tasks in shotcut, `(appimage|flatpak|snap|portable|bundled).{0,60}(vaapi|nvenc|qsv|hardware accel|hw accel|hardware encod)|(vaapi|nvenc|qsv|hardware accel|hardware encod).{0,60}(appimage|flatpak|snap|portable|bundled)`; share 0.3% against 0.1% in kde-bugzilla

The topic is small in volume and heavy in outcome. Inside kde-bugzilla, the only one of the exported trackers where the reason for closing is set by hand, among tasks with an
encoder name in the title **8 FIXED out of 23 closed = 0.35**, the rest —
7 WORKSFORME, 4 INVALID, 2 DUPLICATE and 2 UPSTREAM. Why this class is fixed
worse than all others is counted and explained in **F-RES-006** and is not re-opened here; for the choice of an encoder another number matters:
even among REQUESTS (`tracker-rank.py --wishes`) the topic "hardware/driver" has a fix share of 0.67 —
the worst of the sixteen topics, although for requests it is usually higher than for complaints. The request
"let me choose the encoder" cannot be closed even when it has been accepted.

Age shows the same thing: `[CODEC] Hardware accelerated video
decoding/encoding` in Olive has been open since 08.02.2018, 33 comments, and the body of the task
consists of two words.

> do it
> — olive #26, 08.02.2018, open,
> https://github.com/olive-editor/olive/issues/26

The rejected one (for which the note was created) is about the detector, not the hardware:

> Detection fails and you get: … Yet, vainfo seems to say that everything is OK.
> — shotcut #1056, 18.03.2021, closed `not_planned` 25.10.2024 with the label
> `expired`, https://github.com/mltframework/shotcut/issues/1056

The live request is about profiles:

> This issue is to improve the render profiles based on community feedback.
> — kde-invent #808 "Improve GPU render profiles", 13.10.2020, open,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/808

Two lists in one window:

> When ticking the "Use hardware encoder" checkbox with an AV1 export preset
> selected nothing happens. It still uses a CPU encoder even if a hardware
> encoder would be available.
> — shotcut #1460, 18.07.2023, https://github.com/mltframework/shotcut/issues/1460

One handle — different regulators:

> When I export a project with the software codec "libx264" the results are
> good, but the render times are long. My hardware can also render and export
> with the GPU and the codec "h264_amf".
> — Hobbyfilmer, Shotcut forum, 26.05.2020,
> https://forum.shotcut.org/t/export-via-gpu-encoding-poor-results/18244

> Whenever I export a video with hardware encoders enabled, it makes the bitrate
> crazy low.
> — Sausum, Shotcut forum, 24.12.2024,
> https://forum.shotcut.org/t/hardware-encoders-make-the-bitrate-low/46967

## How it fits Kinotrain
CAPABILITY: openspec/specs/media-index/, openspec/specs/export/ — the contract is in `spec.md`, the "why" and code paths are in `design.md`.
A cascade of encoders with a real check of opening (nvenc → vaapi → qsv → libx264 → mpeg4) through dlopen of the system FFmpeg; we do not give the user a choice.
ADDRESS: src/libs/kt-ffmpeg/src/muxer_impl.rs:24-32 — the class "the detector lied" is impossible for us, because an encoder's fitness is determined by `avcodec_open2` in the very process that is writing the file right now; but the REVERSE class is possible, and nothing catches it: `available_encoders()` filters candidates only by `avcodec_find_encoder_by_name`, and `h264_vaapi` and `h264_qsv` with the parameters that `Muxer::open` sets are NEVER opened — both return −22 (EINVAL) before any access to the device.

## Estimate

| What | Answer |
|---|---|
| Size | S — bring the list into line with the truth (candidates + combo + quality label) and name the encoder in the progress line. The option "bring the code to the list" (hw contexts and the NV12 path) is separate work of size L; this note does not solve it |
| Where the main work is | kt-ffmpeg (`VIDEO_ENCODERS`, `available_encoders` — return a pair "name + hardware flag", not a string), kt-app (combo and the quality label), kt-engine (nothing) |
| Project format | not touched |
| Tests that will have to be written | unit in kt-ffmpeg: each candidate from `VIDEO_ENCODERS` with the parameters of `Muxer::open` either opens or is absent from the build — "present in the list, but cannot be opened" must turn red; harness test `layout_tests.rs`: the combo does not offer an encoder that will not open; extension of t25 to the pair "name + hardware"; unchanged `GOLDEN_VIDEO` (the encoder reference is not touched) |
| Depends on | F-EXP-001 (the preset, which will CARRY the chosen encoder), F-RES-006 (the diagnosis about someone else's hardware), F-RES-008 (packaging against hardware) |
| Breaks compatibility | no |

## Verdict

**The mechanism already exists with us and it is better than all five: an encoder's fitness is
determined by its OPENING, at the moment of export, by the same process that
writes the file. There is no cacheable verdict — so there is nothing that can diverge from reality,
and the whole plot "the wizard found the wrong thing" is inexpressible with us.** For this line
the note was created: the proof is the rejected shotcut #1056, where the mechanism
is more honest than ours (a real trial encoding) and still lies, because
the probe is run by someone else's `ffmpeg` from the package.

**We take the narrow part and do not touch the mechanism.** The list shown to the user must
coincide with what will actually open: today `h264_vaapi`
and `h264_qsv` are offered in the combo and never open (−22 by measurement), and
no test catches this. As long as this is so, exactly the complaint from which we are
architecturally protected is reachable for us with one click. Along the way — name the encoder
in the progress line, not only in the result, and remove the "CRF" label from the slider,
which for three of the five candidates does not control CRF.

**Hardware encoding on Intel and AMD is a separate question and a separate price.**
It cannot be solved "for company" with the honesty of the list: these are hw contexts, a second
pixel format and a second branch of frame upload, that is, L, not S.

## Sources

- Tracker dumps of 08.09.2026, `~/.cache/kinotrain/trackers/`: shotcut (1368),
  kde-bugzilla (6346), kde-invent (2217), openshot (4516), olive (1853),
  mlt (601), losslesscut (1920), flowblade (981), pitivi (2672). **The dumps
  `blender-vse` and `ffmpeg-trac` are empty at the time of writing** (the export did not
  finish) — they do not participate in the numbers of the note.
- Reproduction of the numbers:
  `python3 src/scripts/tracker-rank.py --query '(hardware|hw|gpu)[ -]?encod|nvenc|vaapi|_qsv|h264_amf|videotoolbox'`,
  `--rejected` (there, shotcut #1056), `--neglect` (olive #26),
  `--wishes` (the "fixes" column of the topic "hardware/driver").
- https://github.com/mltframework/shotcut/issues/1056 — rejected
  ("Detecting hardware encoders does not appear to work with
  appimage/portable-build/snap…"), accessed 2026-09-08
- https://github.com/mltframework/shotcut/issues/760 — the same class on NVENC,
  a log with the path inside the AppImage (2026-09-08)
- https://github.com/mltframework/shotcut/issues/1460 — two lists of "available"
  in one window (2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/808 — "Improve GPU
  render profiles", a live request (2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/2109 — the matrix
  "packaging × vendor" with question marks (2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1393 — "Render profile
  problems. Defaults too coarsely granulated" (2026-09-08)
- https://github.com/OpenShot/openshot-qt/issues/4002 — crf does not act on
  h264_qsv (2026-09-08)
- https://github.com/olive-editor/olive/issues/26 — open since 2018
  (2026-09-08)
- https://forum.shotcut.org/t/export-via-gpu-encoding-poor-results/18244 and
  https://forum.shotcut.org/t/hardware-encoders-make-the-bitrate-low/46967 —
  "one handle, different regulators" (2026-09-08)
- https://www.vegascreativesoftware.info/us/forum/no-render-template-in-vegas-pro-18-magix-avc-aac-mp4-with-intel-qsv--124278/
  — in VEGAS the encoder is baked into the template name (2026-09-08)
- The Premiere / Resolve / AE rows are taken from search results of 08.09.2026:
  `helpx.adobe.com` from the environment still returns a proxy refusal, the BMD forum —
  403. There was no live check of the five editors of our own in this pass.
- Measurement of `avcodec_open2` — a direct call of the system FFmpeg with the fields
  that `Muxer::open` sets (`width/height/time_base/framerate/pix_fmt=YUV420P/
  gop_size/max_b_frames`); confirmed by `ffmpeg -h encoder=h264_vaapi`
  ("Supported pixel formats: vaapi") and `-h encoder=h264_qsv` ("nv12 qsv").
- Our code read in the tree:
  `src/libs/kt-ffmpeg/src/{muxer_impl,lib,loader}.rs`,
  `src/libs/kt-media/src/muxer.rs`, `src/libs/kt-engine/src/export.rs`,
  `src/libs/kt-engine/tests/{export_options,golden_export}.rs`,
  `src/apps/kt-app/src/render_dialog.rs`, `src/apps/kt-cli/src/main.rs`,
  `src/apps/kt-app/i18n/{en,ru}.toml`.
