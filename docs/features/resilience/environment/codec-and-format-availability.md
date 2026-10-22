---
id: F-RES-009
title: Codec and format: "it does not open my file"
status: candidate
priority: A
cost: M
category: resilience/environment
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08]
we_have: partial
crates: [kt-ffmpeg, kt-media, kt-app, kt-cli]
invariants: [6, 9, 10]
ui_rules: [16]
round: null
point: null
tests: []
depends_on: [F-RES-008]
trackers: [kde-bugzilla, kde-invent, pitivi, shotcut]
checked: 2026-09-08
---

# F-RES-009 — Codec and format: "it does not open my file"

## Essence

The editor drops a file from a phone, a camera or OBS into the program. Then three outcomes follow, and they are not equal: the file opened; the file did not open at all; the file opened — but things go wrong (the sound drifts, the duration lies, frames repeat). The first
outcome is not a feature but a condition of work: until it holds, no other note in the base has a subject. The third is the most expensive, because it looks like success and surfaces in the finished render.

## Who has it and how

| Editor | Version | Called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | File I/O plug-ins | the set of decoders is shipped by the vendor together with the program (plus what Windows provides); the set is the same on any machine, the list of formats is published |
| Adobe Premiere | 26.x | Media Core | Adobe's shared media layer inside the release; the "Supported file formats" list is part of the documentation of the version, new camera formats arrive with a release |
| DaVinci Resolve | 21 | built-in decoders + Studio | its own decoders in the release; some formats and hardware paths are available ONLY in the paid Studio — that is, the set depends on the license, not on the machine |
| Final Cut Pro | 12.3 | AVFoundation / VideoToolbox | the format is opened by the OS; the set is the set of macOS of the given version, extended by a separate package from Apple. The only one of the five whose set is a function of the system, but the system is controlled by the same vendor |
| After Effects | 26.x | Media Core | the same media layer as Premiere; there is no difference in the set of formats from Premiere |

**Outside the core.** The five core editors describe only one pole of the fork: in all
five the set of codecs is FIXED by the vendor. The second pole — the set as a function of the user's
machine — is not represented in the core at all, and that is exactly our case; without the row
below, the "The trick behind it" section cannot be written.

| Program | Version | Called | How it works |
|---|---|---|---|
| Kdenlive | 26.08.0 | MLT on top of FFmpeg | the distribution build takes the SYSTEM FFmpeg, AppImage/Flatpak/snap carry their own; on top of this — its own VFR detector, which offers to transcode the clip to CFR before editing |

Pitivi and Shotcut do not get rows here: they entered the base as sources
(`_trackers.md`), not as comparison editors. Their construction (GStreamer with
separate good/bad/ugly plugin sets for the first; MLT with its OWN built-in
FFmpeg build for the second) is analysed below by COUNT, not by a verdict row — and
precisely they provide the two missing poles of the delivery fork.

## The trick behind it

**The list of supported formats in an editor on the system FFmpeg is not a list but a function of the user's machine, and therefore it cannot be printed.** This is checked
not by reasoning: the trackers have a separate class of tasks "it is in the list, but does not open". Kdenlive shows a filter in the file selector — and invent #1552
(2022, still open) reports that XCF is in the filter, and the import fails with
"Can't open file"; bugs.kde.org #506555 is titled verbatim "Cannot import a
Macromedia/Adobe SWF despite being listed as supported" and is closed UPSTREAM. That is,
the printed list outlived the FFmpeg build for which it was correct. We have
no such list at all, and not out of forgetfulness: material enters the program
ONLY by dropping files (`src/apps/kt-app/src/app.rs`, the `dropped_files` branch),
a media-add dialog with an extension filter does not exist in the program, and
`src/packaging/kinotrain-mime.xml` declares our two project types and not a single
media type. We do not lie about formats precisely because we promise nothing about them — but this is the absence of a promise, not an answer to the user's question.

**Kdenlive paid for the VFR detector with a class of tasks ABOUT THE DETECTOR, not about VFR.**
The mechanism is reasonable: detect a variable frame rate, warn, offer to transcode into a constant one. Further in the tracker come "Constant frame rate footage
is mis-detected as variable frame rate" (invent #1942), "Kdenlive is detecting a
CFR video as VFR" (#1970), "VFR warning after transcoding" (#2079),
"Incorrectly identifying imported clips as having variable frame rate"
(bugzilla #491257, closed INVALID), "Transcoding from VFR to CFR does Nothing"
(#491601, FIXED) and "Transcoding of VFR clips loses orientation … due to upstream
FFmpeg bug" (#521234). Not one of the six is about VFR playing badly. We have no
detector and will not have one: `VideoPath` asks not "which frame in sequence is this" but
"the largest pts ≤ target" (`src/libs/kt-ffmpeg/src/source_impl.rs`, `target_ts` +
`prepare_for`, floor done by hand instead of `av_rescale_q`), therefore a variable frame rate
IN THE VIDEO is correct for us by construction — this whole class of tasks cannot be expressed.
The price is exactly the reverse, and it is in the sound: see "Cons".

**A placeholder that names the FILE solves "which", but not "why".**
`src/libs/kt-media/src/placeholder.rs` draws a dark frame with the name — and it is
the same for a missing file and for a file that is in place but has no decoder. The cause is in the type: `MediaError` (`src/libs/kt-media/src/lib.rs`) has
exactly two variants — `OutOfRange` and `Decode(String)`, into which "no file", "no video decoder" and "no rights" all fall, differing only in an English string
inside. The caller structurally cannot tell them apart, and therefore says one thing for all cases.

## Pros

- Codecs are taken from the SYSTEM FFmpeg via dlopen (invariant 10,
  `src/libs/kt-ffmpeg/src/loader.rs`): whatever the user has installed in the system is what
  we decode, without a rebuild and without our involvement. The class "the release has no
  codec, wait for a new release" is not expressible for us by construction.
- For the same reason one binary works on FFmpeg 6, 7 and 8 (major version via
  `avcodec_version()`, types from the version module) — an upgrade of the system
  FFmpeg extends our set of formats by itself.
- Video with a variable frame rate plays correctly without a single line of detector: frame selection is "the largest pts ≤ target" on the real pts of the stream
  (t112 keeps the floor when the project fps ≠ the source fps).
- Cover art in ogg/mp3 (attached picture) is not counted as a video stream
  (`AV_DISPOSITION_ATTACHED_PIC` is discarded in `open_with_index`) — the specific foreign class "a music file opened as a one-frame video" is closed, and closed by a test (t10 ogg).
- An unavailable source blocks neither opening the project nor rendering: a placeholder takes its place (t208–t210), and the edit stays intact.

## Cons

- **We cannot promise that a format will open, and we do not know it in advance.**
  There is no surface in the program for "which decoders exist on this machine":
  `available_encoders()` (`src/libs/kt-ffmpeg/src/muxer_impl.rs`) exists and is called from exactly one place — the encoder combo in the render dialog; a paired enumeration of DECODERS does not exist in any crate.
- **A failure to open when adding material is shown as Info and is immediately overlaid by the word "added".** `add_media_clip_at`
  (`src/apps/kt-app/src/app.rs`) first puts the media into the model with the `AddMedia` command and places the clip, then tries to open the source, and on error calls
  `note()` — `Severity::Info` — after which it unconditionally calls `note(ADDED)`. The status bar keeps saying "added", the clip sits on the track, and the player has no source.
  This is a direct violation of rule 16 of the UI checklist ("an impossibility is an ERROR, not a faded hint").
- **The render preflight counts as unavailable only what is missing from the disk.**
  `missing_sources` (`src/apps/kt-app/src/render_dialog.rs`) filters by `Path::exists()`, while `open_all` substitutes a placeholder on ANY open error.
  A file is in place, the codec is missing → there will be NO warning "unavailable sources (N)" before the start, and the line "rendered with placeholders (N)" appears only after
  the render. Two definitions of one word in one dialog.
- **Two different policies for one condition.** kt-app substitutes a placeholder and continues; kt-cli (`src/apps/kt-cli/src/main.rs`) prints the raw error and
  returns 1 — both on the audio path and on the video path. Which is more correct is a question, but the divergence is not declared anywhere.
- **Sound of a VFR/broken file: the FIFO is anchored ONCE.** `AudioPath::push_frame`
  (`src/libs/kt-ffmpeg/src/source_impl.rs`) takes the pts only of the FIRST frame
  after opening/seek (`if self.fifo_start.is_none()`), and further frames
  are appended on the assumption of continuity. A pts gap in the audio stream (a broken
  file, a container splice, a hole in the recording) silently compresses the sound; and since the
  audio callback is the master clock (invariant 6), the video will follow the displaced sound. No test checks this: all fixtures of `src/fixtures/gen.sh`
  are generated by ffmpeg and are perfectly continuous.
- **The project grid is four integers.** The fps is chosen by the 24/25/30/60 combo
  (`src/apps/kt-app/src/app.rs`), is never derived from the clip, and 23.976/29.97
  cannot be expressed in the `u32` type. Snapping an NTSC series to the nearest integer exists,
  but ONLY on import of a foreign project (`kt_otio::convert::grid_fps`, t178) — for a media file there is no such path.
- The absence of FFmpeg altogether is a neighbouring class; it is analysed in F-RES-008 and is not repeated here. The opposite case is interesting here: the loader worked, the major version was determined, but there is no decoder for this codec_id
  (`avcodec_find_decoder` → `FfError::Av("no video decoder")`).

## What users say

SIGNAL: "he does not open my file" — 64 tasks in kde-bugzilla, `(cannot|can.?t|unable to|fail(s|ed)? to|won.?t|does ?n.?t) (open|import|load|read|play|add|recognize|detect) [^.]{0,24}(clip|video|audio|movie|media|footage|mp4|mov|mkv|avi|mp3|image|sequence)|unsupported (video |audio )?(codec|format)|missing (codec|decoder|plugin)|(codec|format) (is )?not supported`; share 1.0% versus 0.1% in shotcut

SIGNAL: "ugly is not installed" — 12 tasks in pitivi, `gstreamer.*(plugin|ugly|bad|good)|plugins?.(good|bad|ugly)|gst-plugins`; share 0.4% versus 0.0% (none of 6346) in kde-bugzilla

SIGNAL: the constant/variable frame rate detector — 5 tasks in kde-invent, `vfr|variable frame ?rate|constant frame ?rate|\bcfr\b`; share 0.2% versus 0.1% in kde-bugzilla

**The first pair of numbers is the delivery fork, measured.** Kdenlive and Shotcut
stand on ONE engine — MLT — their tasks are caught by the same filter, and the difference
between them is exactly one thing: where the FFmpeg comes from. The third pole is GStreamer.

| Codec delivery model | Tracker | "does not open my file" | Own class on top |
|---|---|---|---|
| own FFmpeg build, set fixed | `shotcut` (1368) | 2 = 0.1% | — |
| GStreamer, plugin sets separate | `pitivi` (2672) | 14 = 0.5% | 12 = 0.4% about plugins |
| system FFmpeg via MLT | `kde-bugzilla` (6346) | 64 = 1.0% | — |

The tenfold difference between the two MLT editors is not about code quality, but about
the fact that at Shotcut the codec set is the same for all users, while at Kdenlive it is
its own on every machine. **And the price of a fixed set is visible in the same tracker
literally**: for Shotcut to start opening AV1, a task had to be filed, "Add
libdav1d to FFMpeg build to support AV1 files" (#919) — that is, rebuild
FFmpeg and release the editor; nvenc was added the same way (#372). For us this
class is not expressed: a new codec arrives with an update of the system FFmpeg and requires nothing from us. The reverse side is there too: one's own libraries can be
WORSE than the system ones ("Bundled VAAPI libraries fail when using av1", #1752 — the author
fixes it by renaming the files to `lib` so that Shotcut does not load them).

Pitivi's share is lower than Kdenlive's not because GStreamer opens more:
half of the class has moved into its own, separate one — the set of plugins is visible to the user
and is named by its name (the second line of the SIGNAL, 12 tasks against zero for
Kdenlive). Kdenlive has no such class not because MLT has everything, but
because FFmpeg is monolithic and the user does not have to split it into good/bad/ugly. The difference is visible in the tracker directly: "Do not have gst bad/ugly/etc.
installed / Import a clip that requires those, the gst codec installer shows up"
(pitivi #889) — Pitivi has a whole installer dialog for this, and a noticeable part of
the tasks about it are exactly that the dialog did not appear or appeared for no reason.

The mandatory `--rejected` analysis. The expectation "the format is the upstream FFmpeg, we are not to blame" was NOT confirmed: in the "formats/import" topic, kde-bugzilla's
UPSTREAM/DOWNSTREAM/MOVED — 27 of 709 (3.8%) against 3.9% across all of bugzilla, and
WONTFIX/INVALID/UNMAINTAINED — 51 (7.2%) against 8.2%. The topic is NOT dumped outward more
often than average, and that in itself is a finding: "upstream" in the eyes of a maintainer is
not an excuse but a rare and precise verdict (across all of bugzilla UPSTREAM is set 123
times out of 6346). The topic differs by another column: the share of tasks closed by a REAL
fix — 33.9% of all tasks in the topic, the worst indicator among the large ones (timeline
44.2%, interface 47.3%, tracker average 40.5%), with WORKSFORME at 25.0% and 21.7%
still open. "He does not open my file" more often than any other topic ends
not in a solution but in the file having opened for the maintainer — which is what
should happen when the codec set differs between two people. All percentages in this paragraph
are compared ONLY within kde-bugzilla: closing codes exist in Bugzilla and do not exist in
gitlab/github, where a closure without a mark is forcibly counted as a fix by the script.

> XCF seems to be one of the “supported” file types for the file selector, but
> if you try importing an XCF it doesn’t work. The error is just “Can’t open
> file [path/to/file.xcf]”.
> — invent.kde.org #1552, 20.10.2022, open,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1552

A format in the list is not a promise that it will open on THIS build:

> "WARNING No decoder available for type 'video/x-dv, systemstream=(boolean)true,
> format=(string)PAL'." On Archlinux `gst-play-1.0` can play the file just fine.
> — pitivi #2628 "Missing decoder for `video/x-dv`", 13.08.2022, open,
> https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2628

The user is not obliged to know that "the plugins are installed" and "the decoder is there" are different claims:

> The program does not want to load the xptv file because it says it does not
> have the appropriate mp3 decoder. However, gstreamer-plugin-lame,
> gstreamer-plugin-ffmpeg, gstreamer-plugin-good, gstreamer-plugin-bad,
> gstreamer-plugin-ugly is all installed.
> — pitivi #956, 05.11.2010, https://gitlab.gnome.org/GNOME/pitivi/-/work_items/956

A mirror of our own case from F-RES-008 (the media layer did not come up — the whole project turns into "unavailable"), only in a neighbour and with a fixed set:

> Only the AppImage bundle is impacted, the Portable TAR is working fine …
> When started from CLI, the AppImage flavor don't display anymore supported
> codecs (actually, it doesn't print anything) whereas Portable TAR prints a lot
> of formats/codecs
> — shotcut #1482 (two adjacent list items), 24.10.2023,
> https://github.com/mltframework/shotcut/issues/1482

On the price of the VFR detector — two quotes in a row:

> If you check it with MediaInfo, it is very clearly a constant frame rate
> video. Yet, Kdenlive INSISTS that it is VFR and needs to be transcoded.
> — invent.kde.org #1970, 22.02.2025,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/1970

> Even after transcoding a VFR clip, the render dialog gives a warning that the
> video needs transcoding.
> — invent.kde.org #2079, 28.10.2025,
> https://invent.kde.org/multimedia/kdenlive/-/work_items/2079

## How it fits Kinotrain

ADDRESS: src/apps/kt-app/src/app.rs — the class "the release has no needed codec" is impossible for us, because there are no codecs in the release at all (invariant 10, dlopen of the system FFmpeg); the class "FFmpeg is there, the codec is not" is possible, and nothing catches it: `add_media_clip_at` puts the media into the model BEFORE the attempt to open, the failure of `open_source_indexed` goes to `note()` at the Info level and is overlaid by the message "added", and the render preflight `src/apps/kt-app/src/render_dialog.rs::missing_sources` counts as unavailable only a non-existent path (`Path::exists`), so the user is not warned before the start.

### What already exists

- `src/libs/kt-ffmpeg/src/loader.rs` — iteration over SONAME 8/7/6, a check of
  `avcodec_version()`, `RTLD_LOCAL`, `FfError::NotFound` with the list of failures.
- `src/libs/kt-ffmpeg/src/source_impl.rs` — opening the container, scanning the packet
  index, lazy `avcodec_find_decoder` for audio and video (the texts "no audio
  decoder" / "no video decoder" already exist), discarding of the attached picture,
  frame selection by floor on the real pts.
- `src/libs/kt-media/src/placeholder.rs` + `src/apps/kt-app/src/util.rs::open_source_or_placeholder`
  — an unavailable source does not prevent opening, playing and rendering (t208–t210).
- `src/libs/kt-media/src/index.rs` — the sidecar `<project>.ktindex.db`: the probe and the
  packet index are already cached, a place for "what we found out about this file"
  exists.
- `src/libs/kt-otio/src/convert.rs::grid_fps` — snapping of the NTSC series to the
  90 kHz grid on import of a foreign timeline (t178).
- `src/libs/kt-ffmpeg/src/muxer_impl.rs::available_encoders` — a sample of
  what polling the capabilities of the system FFmpeg looks like.

### What is missing

- Separation of the cause at the level of the TYPE: `MediaError::Decode(String)` does not distinguish
  "no file" from "no decoder" and from "a broken file". As long as the type does not distinguish them, neither the message
  nor the placeholder will be able to tell them apart.
- A poll of decoders (a pair to `available_encoders`) and one honest phrase about
  which exact codec is missing.
- A render preflight that counts as unavailable the same thing as `open_all`.
- Fixtures with a pts gap in the audio and VFR fixtures — today `src/fixtures/gen.sh`
  has none of them, and the FIFO anchor is not checked by anything.
- Fractional project frame rates (23.976/29.97) — today the project fps is a `u32` from
  four values and is not derived from the material.

### What gets in the way

The separation of causes runs into the crate boundary: the detail lives in
`kt_ffmpeg::FfError` (there it is known that exactly `avcodec_find_decoder` failed), while the decision is made by kt-app through `kt_media::MediaError`, which has two variants.
Extending the enumeration touches every place where the error is today folded into a
string. Second: polling the decoders cannot be done in the first frame — `init()` in
kt-ffmpeg is lazy, and dlopen of five libraries synchronously would delay the appearance of the
window (invariant 12); the place is the same as for the startup self-check from F-RES-008 —
the background index thread. Third and main: the missing codec can be named BY NAME,
but one cannot say WHAT to install — the package names differ in every distribution;
naming an exact name wrongly is worse than naming the codec.

### Which invariants it touches

Invariant 10 (the binary is not linked with libav*) — the feature stands on it and does not touch it; invariant 9 (export is in-process) — the data source for polling
capabilities; invariant 6 (the audio callback is the master clock) — it explains why
a gap in the audio FIFO also drags the video; invariant 12 (repaint only on movement) — the poll must be one-off and background. Rule 16 of the UI checklist is already
violated now, in `add_media_clip_at`.

## Estimate

| What | Answer |
|---|---|
| Scope | M — 2–3 stages of the round |
| Where the main work is | kt-media (variants of `MediaError`), kt-ffmpeg (polling decoders, parsing `FfError`), kt-app (the message, the render preflight) |
| Project format | not touched |
| Tests that will have to be written | a fixture with a pts gap in the audio: the sum of the samples handed out equals the length per the container; a VFR fixture: the frame for each tick equals "the largest pts ≤ tick"; a unit test — `missing_sources` and `open_all` give ONE list; kt-app: a failure to open when adding media arrives as Warn and is not overlaid by "added" |
| Depends on | F-RES-008 (the startup self-check of the environment — the same surface and the same background thread) |
| Breaks compatibility | no |

## Verdict

We take it, priority A. Not for the sake of "format support" — we inherit that from
the system FFmpeg in full anyway, and this is the strongest side of the chosen model. We take it for the sake of honesty ABOUT it: today the program gives "FFmpeg is there, codec is not" the same diagnosis as "the file is not there", and it first says "added", then silently
renders a placeholder without a warning before the start. The expensive part (dlopen, versions,
the placeholder, floor by pts) is already done; what is missing is one variant in the enumeration of
errors, one background poll, and reconciling two definitions of the word "unavailable" in
one dialog.

We do NOT take the VFR detector and the suggestion to transcode: the six Kdenlive tasks above are
the price of the detector itself, not of VFR, and there is no reason to pay it while frame selection goes
by the real pts. Instead of the detector we take a fixture and a test: our problem is not in
video, but in the anchor of the audio FIFO, and it is checked by nothing.

Boundaries: complete absence of the system FFmpeg — F-RES-008 (do not duplicate; it is analysed there, including why `backend_error()` is not called); the genre of the error message — F-HLP-008; a missing source as a fact and as a wording — F-HLP-012;
checking the project before the start of render — F-HLP-011; pictures and frame sequences as a separate kind of material — F-MED-003.

## Sources

- The registry, lexicon and counting method — `docs/features/_trackers.md`; each number is reproduced by `python3 src/scripts/tracker-rank.py --query '<filter>'`
  (dumps of 08.09.2026). The complete ones at the time of writing were `kde-bugzilla`
  (6346), `kde-invent` (2217), `pitivi` (2672), `shotcut` (1368), `mlt` (601)
  and `otio` (680) — only they take part in the numbers. `olive` was being exported at
  that very moment (405 records out of ~1853) and was not counted; `openshot`, `flowblade`,
  `losslesscut`, `blender-vse` and `ffmpeg-trac` were not exported at all, so the
  question "which formats break for years" (the only one `ffmpeg-trac` would answer)
  is not analysed here and remains for the next wave.
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1552 — XCF in the file selector filter, import fails (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=506555 — "Cannot import a
  Macromedia/Adobe SWF despite being listed as supported", UPSTREAM (2026-09-08)
- https://gitlab.gnome.org/GNOME/pitivi/-/work_items/2628 — Missing decoder for
  `video/x-dv`, open since 2022 (accessed 2026-09-08)
- https://gitlab.gnome.org/GNOME/pitivi/-/work_items/956 and
  https://gitlab.gnome.org/GNOME/pitivi/-/work_items/959 — "missing plugin"
  as a separate user class (accessed 2026-09-08)
- https://gitlab.gnome.org/GNOME/pitivi/-/work_items/889 — the dialog for installing
  GStreamer codecs and its side effects (accessed 2026-09-08)
- https://gitlab.gnome.org/GNOME/pitivi/-/work_items/1276 — "No dialog to
  install missing codecs after discovering clips" (accessed 2026-09-08)
- https://invent.kde.org/multimedia/kdenlive/-/work_items/1942,
  https://invent.kde.org/multimedia/kdenlive/-/work_items/1970,
  https://invent.kde.org/multimedia/kdenlive/-/work_items/2079 — false
  triggers of the VFR detector (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=491257,
  https://bugs.kde.org/show_bug.cgi?id=491601,
  https://bugs.kde.org/show_bug.cgi?id=521234 — the detector and transcoding
  VFR→CFR from the Bugzilla side (accessed 2026-09-08)
- https://bugs.kde.org/show_bug.cgi?id=493024 and
  https://bugs.kde.org/show_bug.cgi?id=423635 — "Unsupported video codec:
  libwebp" / "libx264" (accessed 2026-09-08)
- https://github.com/mltframework/shotcut/issues/919 (libdav1d for AV1),
  https://github.com/mltframework/shotcut/issues/372 (a rebuild for nvenc),
  https://github.com/mltframework/shotcut/issues/1752 (own VAAPI libraries
  are worse than the system ones) and https://github.com/mltframework/shotcut/issues/1482 —
  the price of a fixed codec set in the release (accessed 2026-09-08)
- Our code was checked by grep on 08.09.2026: `src/libs/kt-ffmpeg/src/{loader.rs,lib.rs,source_impl.rs,muxer_impl.rs}`,
  `src/libs/kt-media/src/{lib.rs,ffmpeg.rs,placeholder.rs}`,
  `src/apps/kt-app/src/{app.rs,util.rs,render_dialog.rs}`,
  `src/apps/kt-cli/src/main.rs`, `src/fixtures/gen.sh`,
  `src/packaging/kinotrain-mime.xml`, `src/libs/kt-otio/src/convert.rs`.
- The rows of the five core editors — from the vendors' documentation and `_editors.md`
  (checked 2026-09-07); the codec set was not checked live for any of them;
  the Kdenlive row is from its public tracker, not from the documentation.
