---
id: F-TXT-003
title: Generators — colour, gradient, noise, shapes, timecode, test charts
status: done
priority: B
cost: M
category: titles/generators
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26]
we_have: yes
crates: [kt-media, kt-model, kt-otio, kt-app, kt-cli]
invariants: [1, 4, 5, 8, 11]
ui_rules: [4, 6, 8, 13, 20]
round: R.52
point: 402, 419
tests: [t469, t470, t471, t472, t473, t474, t475, t476, t477, t478, t479, t491, t492, t493, t494, t524, t525, t526, t527, t528, t529, t530, t531, t532, t533]
depends_on: []
checked: 2026-09-17
---

# F-TXT-003 — Generators — colour, gradient, noise, shapes, timecode, test charts

## Essence

A clip that has NO source file: it computes the picture itself. A solid
colour, a linear or four-point gradient, noise/grain, a simple shape, a checkerboard, a timecode of running digits, a countdown, an SMPTE bars chart with a 1 kHz tone. On the track it is an ordinary event: it can be trimmed, cut,
copied, accept FX, transitions and level. Working reasons: a backdrop under a title, a "dip to colour", a black frame at the end of the film, an even background under a matte,
a temporary stand-in in place of a frame not yet shot, a reference signal for
checking the path. The feature sounds dull, but it carries titles and
adjustment layers on itself: both are the same clip without media.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026 | Media Generators (a tab next to the transitions and FX) | a generator is a MEDIA ELEMENT ("generated media"): it is dropped on a track, the event refers to it, and the generator's window has its own keyframe track at the bottom — the parameters animate as usual (in the release notes of VP16 U5 a separate line fixed the deletion of these keys for Solid Color). The set: Solid Color, Color Gradient, Checkerboard, Noise Texture, Test Pattern, Timecode, Text/Credit Roll; the list is **[NOT VERIFIED]** against the official help, it was assembled from a Boris FX forum |
| Adobe Premiere | 26.x | File → New → Color Matte / Black Video / Bars and Tone / Universal Counting Leader / Transparent Video | elements of the PROJECT in the bin, not effects: the dialog asks for width, height, timebase and PAR — exactly as for an Adjustment Layer (F-MPH-004). In Premiere's OTIO export they come out as `GeneratorReference.1` — in our own fixture `src/fixtures/otio-samples/premiere_example.otio` the TITLE `test_title` is encoded this way. The list of menu items is **[NOT VERIFIED]**: `helpx.adobe.com` does not open from this environment |
| DaVinci Resolve | 21 | Effects Library → Generators (plus everything that Fusion draws) | by dragging straight into the timeline; officially: "open the effects library at the top left of the screen, find the text generator or Fusion title template you want, and drag it into the timeline above your video tracks". Parameters — in the Inspector, with keys. The set (Solid Color, SMPTE Bars, EBU Color Bar, Grey Scale, 10 Step, YCbCr Ramp, Window, Four Color Gradient, Noise) **[NOT VERIFIED]** against the Reference Manual. The timecode on output is NOT a generator, but a separate Data Burn-In panel at the delivery stage |
| Final Cut Pro | 12.3 | the Titles and Generators side panel: categories Backgrounds, Elements, Solids, Textures | a generator is a clip; Elements contains Timecode and Placeholder. Placeholder is a stand-in generator for a frame not yet shot: "Generated placeholders are useful when you want to fill a gap in the project with a clip that provides a hint about the final content", it is configured by the type of plan and has a notes field ("select View Notes"). Edited in the Generator inspector, in depth — in Motion |
| After Effects | 26.x | Layer → New → Solid + the effects Fill, Ramp, 4-Color Gradient, Fractal Noise, Cell Pattern, Checkerboard, Grid, Timecode | there is NO generator object at all: there is a layer without content (Solid — only colour and size) and effects that IGNORE the input and write the frame entirely. "Generator" in AE is a property of an effect, not an entity. The list of effects **[NOT VERIFIED]** against the Adobe help |

## The trick behind it

**1. A generator has neither a duration nor a frame grid — and yet the exchange format forces it to declare them.**
A source that can deliver a frame at any tick and at any resolution breaks the assumption
"a clip has material with a length and fps", on which half of the
editor stands: what is the length of a clip when dropped on a track, what to write
in the bin card, where is the EOF, what to show in the filmstrip. All five solve
this the same way and uninterestingly — with a global default in the settings
("still image duration"). But at the boundary of a FILE the infinity has to be cut: in our committed fixture
`src/fixtures/otio-samples/generator_reference_test.otio` the colour bars
are written as `GeneratorReference.1` with `generator_kind: "SMPTEBars"`,
an empty dictionary `parameters` — and a mandatory `available_range` of 50
frames at rate 24. That is, a source that has no length is stored with an invented length, and it travels down the pipeline as if it were real.
We already have exactly this contract written and debugged for another reason:
`PlaceholderSource` (`src/libs/kt-media/src/placeholder.rs`) declares
`fps: None` with the comment "there is no grid of its own: the engine plans by the PROJECT grid" (line 134) and a duration of `90_000 * 3600 * 1000` — a thousand
hours, "so as not to cut the edit" (line 21).

**2. A timecode generator and a timecode burn-in are two different things, and
they are told apart by their CLOCK.** This is visible where both live side by side. In Final Cut Pro
the Timecode generator is a clip from the Elements category, which is placed
"over the primary storyline and any other clips or storylines" and
stretched to the length of the project: it shows the time of the RECORDING, the time of
the timeline. And the Timecode effect is another object, and it, in Apple's wording,
"displays the clip name and source clip timecode in the viewer and
in exported files", that is, the time of the MATERIAL plus the file name. Hence the
different place in the pipeline: a generator is edited, it can be trimmed and
moved, while a burn-in must lie on top of EVERYTHING (including the titles) and
not get into the master — which is why in Resolve it is moved out of editing into the Data
Burn-In panel at the delivery stage, and not into Generators. We already have both scales
and they are already separated: `VideoFxCtx.burst_index` is filled with the tick
of the TIMELINE (`burst_index: t.0`,
`src/libs/kt-engine/src/video.rs`, line 749), and Noise and
VHS depend on it, while the automation key `AutoKey.t` is the tick of the MATERIAL
(`src/libs/kt-model/src/project.rs`, line 1599). The question "which generator on which clock" is not open with us; it has already been answered twice, in different ways.

**3. A noise generator cannot be made from a noise effect, and with us this is provable by arithmetic.**
What distinguishes them is not the UI but one sign: whether the operation reads its own INPUT. Our `Noise` (`src/libs/kt-fx/src/video.rs`, line 481)
precisely corrects the input pixel: `k = amount * 32`, the offset lies in
±k, and further `*v = (*v + off).clamp(0, 255)`. From this follow two consequences
that are not in any list of capabilities. First, the amplitude ceiling
at `amount = 1.0` is ±32 out of 255, that is, 12.5%: this is grain on top of
the picture by construction; a "grey noise field" cannot be obtained from it
with any setting. Second, on a black frame half of the offsets
are negative and all of them run into the clamp at '0' — instead of uniform noise
one gets a sparse speckle with a mean of ≈ +8. The classic advice "put
a black solid and hang noise on it" will give us not what is expected with us.
This same difference is fixed by construction in our code: an FX without an input frame
does not exist — `layer_frame_cpu` reaches `let frame = src.frame.as_ref()?`
(`src/libs/kt-engine/src/video.rs`, line 744). So a generator with us
physically cannot be an effect; it must be a `MediaSource`. The opposite extreme is After Effects, where there are no generator objects at all, and
Fill/Ramp/Fractal Noise simply ignore the input, sitting on an empty solid:
there the boundary is drawn inside the effect, with us — outside it.

## Pros

- A generator is a `MediaSource`, not a new timeline entity, so
  the whole machinery of an event comes for free: trim, split, reverse, transitions,
  solo fades, `opacity`, the FX chain, automation, media substitution
  by a morpher, block copy-paste. Not a single new object in the model.
- "Any resolution" is free: the source receives `Quality` and draws
  exactly into it (`PlaceholderSource::frame_rgba(q)`). The backdrop does not blur
  when the project resolution changes and does not need to be recreated — unlike a
  PNG drawn in an image editor for a specific frame.
- No file — no decoder, GOP or seek: a frame request is O(1), and the audio
  burst is computed by a formula. The MediaWorker queue (invariant 5) is not
  involved, prefetch does not spend its budget, `keyframe_before` is trivial.
- The test table with a tone is the reference for an end-to-end check of the path:
  the same signal at the input of the preview and at the output of the export. This is exactly
  the comparison that is already fixed for us by invariant 7 and tests
  t11/t67 (CPU) and t184 (GPU), only now it can be shown to the user with their own eyes, not only in CI.
- The placeholder of a missing file and a generator are one mechanism. What was
  debugged in an accident (items 207–209) starts working on
  the user's deliberate choice: one branch of code instead of two.

## Cons

- A source without a length must name the length everywhere the UI shows it.
  With us the placeholder declares 1000 hours, and the bin card will honestly
  write "1000:00:00" (`duration_or_probe`,
  `src/apps/kt-app/src/bin_panel.rs`, line 635). Either the generator
  gets its own length field (and then it duplicates the length of the
  event — UI rule 20), or the bin learns the case "there is no length".
- There is nothing to index, while the UI waits for strips and thumbnails. Invariant 8 requires
  that the waveform come from the peak pyramid, and the thumbnail — from
  `<project>.ktindex.db`; a generator has no file, and the sidecar will not help it
  (the record key in the DB is the file size and mtime, `file_identity`,
  `src/libs/kt-media/src/index.rs`, line 44). Today such media are simply
  STRUCK OUT of the thumbnail service: `filter(|m| !m.path.starts_with("synth:"))`
  (`src/apps/kt-app/src/thumbs.rs`, lines 171 and 189) — the card stays
  empty, and the filmstrip too.
- A pseudo-path in `target_url` is a lie for someone else's program. We already write
  test sources this way (`synth:<pattern>` — verbatim, `docs/file-format.md`),
  and for tests this is fine, but for a user's project it means
  that Resolve will show media offline. An honest scheme exists in OTIO itself
  (`GeneratorReference.1`), but with us it is not parsed: `decode_media_ref`
  (`src/libs/kt-otio/src/schema.rs`, line 462) knows only
  `ExternalReference.1` and `MissingReference.1`.
- Generators have many parameters and they are heterogeneous (colour, gradient angles,
  number of bars, grain size), and with us they are drawn by the single
  `widgets::param_row` according to `ParamSpec` — where a colour is THREE separate
  sliders 0..255 (`color_r/g/b`, `src/libs/kt-model/src/project.rs`,
  line 2351). The unit `Rgb8` is declared, but there is no colour picker in the program
  (`src/apps/kt-app/src/widgets.rs`, line 25 — `Rgb8` is handled as a
  plain number). Three rows for one colour are a direct violation of UI rule 13.
- Everything "random" falls under invariant 11: the frame of a generator must be a
  function of position, not of state. We have a ready answer
  (`noise_hash` / `noise_seed`, counter-based, repeated bit-for-bit in WGSL —
  `src/libs/kt-fx/src/video.rs`, lines 459 and 470), but binding the grain
  to the TIMELINE tick, as the effect does now, is wrong for a generator: move
  the clip — and the grain changes.

## What users say

The thread "Vegas Media Generator — CHECKERBOARD" on the official Boris FX forum:
the thread's author starts a series of reviews, and the regulars explain why they do not use generators.

> I’ve rarely ever found a use for them. I mostly use the media generators
> for testing purposes when I don’t want to dig for video media to drop on
> the timeline…
> — Fr0sty, forum.borisfx.com, 25.04.2026, https://forum.borisfx.com/t/vegas-media-generator-checkerboard/22837/4

> It’s true that these offerings have languished over many iterations of
> Vegas and, like most, I haven’t made a lot of use of them so I though it
> might be fun to experiment to see what might inspire others …
> — Selinita, forum.borisfx.com, 25.04.2026, https://forum.borisfx.com/t/vegas-media-generator-checkerboard/22837/5

In the same thread — about which of the generators is really used:

> There is one thing I did find a use for them for… generating backgrounds
> for lower thirds. However, most of the time I just use solid colors
> (which technically is also a media generator).
> — Fr0sty, forum.borisfx.com, 25.04.2026, https://forum.borisfx.com/t/vegas-media-generator-checkerboard/22837/6

The conclusion for us: of the whole list, ONE is needed every day — the solid colour; noise,
the checkerboard and the test tables live in the "once a year, but badly needed" mode.
This ratio matters more than the length of the list: the colour clip must be in the very
first stage, the rest can be added one at a time.

What is animated with keys in VEGAS generators, and what hurt, is visible from the
release notes:

> Animation keyframes can be properly deleted from the Solid Color media
> generator
> — VEGAS Pro 16 Update 5 release notes, 30.04.2019, quoted from https://forum.borisfx.com/t/vegas-pro-release-history/24284/38

## How it fits Kinotrain
CAPABILITY: openspec/specs/generators/ — the contract is in `spec.md`, the "why" and the code paths are in `design.md`.
Fill/gradient/shapes — generators with a pseudo-path via `kt_media::factory`; native 4096 feeds the decode scale step.

## Estimate

| What | Answer |
|---|---|
| Volume | M — 2–3 stages of the round: (1) the source + model + one colour, (2) the other kinds and parameters, (3) OTIO and thumbnails |
| Where the main work is | `kt-media` (a new `MediaSource`), `kt-app` (the entry into the bin, the colour picker, thumbnails), `kt-otio` (`GeneratorReference.1`); `kt-engine`, `kt-gpu`, `kt-fx` — NOT touched |
| Project format | needs a `GeneratorReference.1` branch in the reader and the writer. Bumping `kt.v` to 5 — an open question: an old build degrades into a NOTICEABLE placeholder rather than silently losing data, so the rule of R.29 ("a refusal is more honest than silent corruption") does not trigger automatically here. Decided in the round |
| Tests that will have to be written | determinism and the signature of a generator frame (on the model of t208); a round-trip of the model and of `.otio` with `GeneratorReference` — an extension of t154; a golden export with a generator in the chain (t11/t67); the bin card and a thumbnail without a file (kt-app unit, on the model of t69/t71); a harness test of the parameter window (`layout_tests.rs`); above all — a SINGLE parsing of the pseudo-path, case-insensitive (footnote item 211) |
| Depends on | nothing. The other way round: F-TXT-001 (titles) rests on this note — it independently arrived at the same decision "a title as a source: `MediaRef` with a scheme in the path plus a `MediaSource` next to `placeholder.rs`"; the background and the luma matte for F-MSK-003 are taken from the same place |
| Breaks compatibility | no: old files stay byte-stable, foreign ones are already handled by the passthrough |

## Verdict

We take it. This is not "one more feature" but the missing third kind of source without a
file — the first two (`SyntheticSource`, `PlaceholderSource`) are already written,
debugged and covered by tests, so the road from "a clip must refer to
a file" to "a clip computes the picture itself" is shorter than it looks, and it does not touch
the engine, the GPU or invariant 7. The order is dictated by the feedback: the solid colour —
in the very first stage (it is used daily), the gradient and shapes — second
(backgrounds and mattes are made from them), noise, the checkerboard and the SMPTE table with a tone —
last. The timecode is to be split from the very beginning into two objects:
a generator clip on the TIMELINE clock and a burn-in at the output; if we glue
them into one, we will repeat someone else's mistake, which Apple and Blackmagic have already separated
into different places of the program.

This must be done TOGETHER with F-TXT-001: that note arrived at the same conclusion
independently ("a title as a source, not as a new layer of the plan"). The mechanism
is one — a registry of schemes in the media path and a `MediaSource` factory by scheme; if
we create it twice, the parsing of the pseudo-path will diverge for a third time (the first
two are `kt-app` and `kt-cli`, footnote item 211).

**The remainder of the note was closed in R.53 (items 419-422).** In R.52 the colour,
gradient, angles, checkerboard, noise, SMPTE and timecode arrived; shapes, the transparent
backdrop, the countdown and the engineering tables are R.53. Separate
notes were deliberately NOT opened for them: rule 2 of the README ("one feature —
one note") — the reconnaissance, the "trick behind it" and the verdict for these kinds live
here, and a second note would be a copy of them. The rule of the kind ("a kind = an enum variant when it has ITS OWN numbers")
is written in `src/libs/kt-model/CLAUDE.md`.

## Sources

- `https://support.apple.com/guide/final-cut-pro/add-timecode-counters-verca9e960b/mac` — the Timecode generator and its difference from the Timecode effect (accessed 2026-09-07)
- `https://support.apple.com/guide/final-cut-pro/add-placeholders-verca9ea012/mac` — the Placeholder generator (accessed 2026-09-07)
- `https://support.apple.com/guide/final-cut-pro/add-generators-ver2833f4152/mac` — the contents of the Generators section in FCP (accessed 2026-09-07)
- `https://www.blackmagicdesign.com/products/davinciresolve/edit` — a quote about the Effects Library and dragging a generator into the timeline (accessed 2026-09-07)
- `https://forum.borisfx.com/t/vegas-media-generator-checkerboard/22837` — the thread about VEGAS generators, posts 4–6 (accessed 2026-09-07)
- `https://forum.borisfx.com/t/vegas-pro-release-history/24284/38` — the release notes of VEGAS Pro 16 Update 5 about the keys of Solid Color (accessed 2026-09-07)
- `https://en.wikipedia.org/wiki/SMPTE_color_bars` — the purpose of the table and of the 1 kHz tone: "producers of television programs typically record _bars and tone_ at the beginning of a videotape or other recording medium so that the playback equipment can be calibrated" (accessed 2026-09-07)
- Local primary sources (checked against the repository code, not against documentation):
  `src/fixtures/otio-samples/generator_reference_test.otio`,
  `src/fixtures/otio-samples/premiere_example.otio`,
  `src/libs/kt-media/src/placeholder.rs`,
  `src/libs/kt-media/src/synthetic.rs`, `src/libs/kt-fx/src/video.rs`,
  `src/libs/kt-engine/src/video.rs`,
  `src/libs/kt-otio/src/{schema.rs,convert.rs}`,
  `src/apps/kt-app/src/{util.rs,thumbs.rs,widgets.rs}`,
  `docs/file-format.md`, `docs/architecture.md`
- The Premiere and After Effects rows and the full set of generators of VEGAS and
  Resolve — **[NOT VERIFIED]** against the primary source: `helpx.adobe.com` does not open from
  this environment, the search engines returned 403 and
  CAPTCHA, the Resolve Reference Manual is a PDF that WebFetch cannot read.
  To be checked in the next wave
