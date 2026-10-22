---
id: F-HLP-008
title: Anatomy of an error message: cause, consequence, action
status: candidate
priority: B
cost: M
category: help/errors
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, vscode-1.136]
we_have: partial
crates: [kt-app, kt-engine, kt-otio, kt-model]
invariants: []
ui_rules: [7, 16, 18, 19]
round: null
point: null
tests: []
depends_on: [F-HLP-010, F-HLP-012, F-HLP-013]
checked: 2026-09-07
---

# F-HLP-008 — Anatomy of an error message: cause, consequence, action

## Essence

Something did not work, and the program says so in one phrase. A good phrase answers three questions in a row:
WHAT happened (the cause), what that means for my work (the consequence), and what to do next (the action).
A bad one answers none of them: "an error occurred". Between the two lies the whole difference between "the editor
fixes it in a minute" and "the editor goes to a forum with a piece of text in the clipboard".

The note is not about WHAT the refusal was (that is F-HLP-010, the refusal of a gesture), but about the TEXT that is
read after the refusal.

## Who has it and how

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | the render message box | "An error occurred while creating the media file *(name)*. The reason for the error could not be determined." The object and the stage are named; the cause is declared UNDETERMINED in plain text. No code, no place, no action. The wording is carried over from the 10–13 line; for 2026.0 — live [NOT VERIFIED] |
| Adobe Premiere | 26.x | Error Compiling Movie | A dialog with FOUR fields: the class at the top (import / GPU render / out of memory / export / I/O / accelerated render / render), the text, the "Error Code" with selectors at the bottom, and the TIMECODE of the failure's place on the timeline. Class 3 = Unknown |
| DaVinci Resolve | 21 | Render job failed | The job's row in the queue turns red; there is no separate text of the cause in the window, the coordinate is the percentage at which the job stopped. The preference User ▸ UI Settings ▸ "Stop renders when a frame or clip cannot be processed" turns the refusal into continuation [NOT VERIFIED live, described from 20.x] |
| Final Cut Pro | 12.3 | the alert "The operation could not be completed" | The cause is generalised to a single phrase; sometimes a macOS negative system code gets through — in the window it is not decoded and is not linked to any help. The community's analysis boils down to "duplicate the sequence and remove the effects one at a time", that is, to searching for the coordinate by hand [NOT VERIFIED live; the specific code numbers are not confirmed by a primary source] |
| After Effects | 26.x | "After Effects error: … ( 86 :: 1 )" | The only one of the five that puts a STABLE paired code into every message. The vendor does not document the code: it works as a search key, not as an explanation |

Five out of five give a cause and (except VEGAS) a hint of the place. Nobody gives an action: none of the five
messages contains a button or an address of the setting by which the refusal is lifted.

**Outside the core.** Without VS Code the feature cannot be described: it is the only one where the three parts and
a fourth live in the message as CONTROLS, not as prose — and this is precisely what none of the five core editors
does. The line is needed not for comparing the quality of the text (here the core is no worse), but for the form of the channel.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| Visual Studio Code | 1.136 | Notifications | The level is declared as a contract ("three types … Information, Warning, Error"); the action is a button INSIDE the message, not advice in the text; the UX guidelines say "Add a **Do not show again** option for every notification" — that is, the channel is controllable by the user himself |

## The trick behind it

**What the mechanism carries that one status-bar line cannot hold.** It carries a COORDINATE and an ANCHOR — two
payloads that are not in the text about the cause, and both of which we already compute and both of which we throw away.

*Coordinate.* The most valuable field of the Premiere dialog is not the text but the timecode at the bottom: Adobe
explicitly advises looking at that place on the timeline. Resolve has no coordinate field at all, and editors use the
progress percentage ("it always falls on the same percentage — so the problem is in a specific place of the edit"),
while the switch "do not stop on an unprocessable frame" exists precisely so that there are MORE coordinates than one:
stopping at the first refusal shows one broken clip out of five. That is, three of the five core editors trade the
coordinate instead of the cause — and it works, because the coordinate is actionable even without the cause, while a
cause without a coordinate is not actionable.

*Anchor.* The second payload is visible only from five languages. The text of a message is translated, and the translated
message STOPS BEING FOUND: "not opened: …" finds nothing. That is why the paired AE code `( 86 :: 1 )` is not a decoration
or an engineer's excuse, but the only part of the message that survives translation; it turns the search string into the
fourth part "more". The price is shown in the same example: Adobe does not document the code, so the search leads to forums,
not to the help. The anchor is a necessary condition of "more", but not a sufficient one.

*Where the payload goes with us.* It will not go into the line: 160 characters are already occupied without it. The coordinate
moves to the timeline — we already have the selection (`self.selected`, `src/apps/kt-app/src/app.rs`) and scrolling to a
find (`search_hits`), and `gpu_required_clips()` already computes EXACTLY the ids that the user sees in the project
(`slice_origin` unfolds morpher pieces back into the originals) — and `export.rs:407` takes `.len()` from them. The anchor
moves into the brackets of the same message — we have done this twice already, and both times by accident.

## Pros

- The class of error as a separate field (Premiere) distributes responsibility BEFORE the text is read: "out of memory" and "I/O" lead to different parts of the system,
  and the editor does not read advice about codecs when the disk is full.
- The coordinate is actionable without the cause: by halving the edit, the guilty clip is found in five minutes, and no explanation was ever needed.
- A stable paired code (AE) depends neither on the version nor on the interface language, so it is the only one that survives translation and serves as a search key.
- The "do not stop" switch (Resolve) turns a refusal into a re-run: the first refusal — one place; a full pass — all of them.
- A level as a declared contract (VS Code: Information / Warning / Error) allows colouring, filtering and counting messages without parsing their text.
- Blender puts the same triple of levels (`INFO`/`WARNING`/`ERROR`) into the status bar and into the Info log at once — retrospect comes for free, from the same call;
  it is not singled out as a row in the table, because the channel repeats the colour-coded Events log of Premiere.

## Cons

- The class is a classification of the SUBSYSTEM, not of the cause, so "Unknown" inevitably becomes the most frequent class and devalues the field.
- A coordinate without a cause shifts the work onto the user: Premiere's official FAQ admits in plain words that the rest must be investigated by oneself.
- A code that the vendor does not document is searched for only in other people's forums: the mechanics of "searching" work, the promise of "more" does not.
- The "do not stop" setting treats the symptom: the file comes out with holes, and the message about the holes is the same single line as before.
- "Do not show again" is a fourth part that SWITCHES THE CHANNEL OFF: the user turns the warning off before he has time to understand it, and does not turn it back on.
- The phrase "the reason could not be determined" (VEGAS) is honest and useless at once: it reports the state of the program, not the state of the user's work.

## What users say

> Is there anyway to get more detailed information about the error beyond the
> cryptic 'An Error Occurred while creating the media file (Name of file) The
> reason for the error could not be determined'?
> — Nigel O'neill, Creative COW, VEGAS Pro forum, 08.11.2010,
> https://creativecow.net/forums/thread/an-error-occurred-while-creating-the-media-file-na/

The second quote is valuable because it comes not from a user, but from the vendor's side — an explanation of WHY the message
is sometimes empty:

> When an "Error Code" number is triggered as unknown, engineering cannot
> explain it explicitly in the error dialog box.
> — Kevin J. Monahan (Adobe), FAQ "Fix Error Compiling Movie errors in Premiere
> Pro", 18.02.2022,
> https://community.adobe.com/t5/premiere-pro-discussions/faq-fix-error-compiling-movie-errors-in-premiere-pro/m-p/12757295

In the same place — an admission about the price of such an anatomy for the editor: "Editors need to do
some investigation to solve the issue, unfortunately".

## How it fits Kinotrain

### What is already there

The split runs not between "good" and "bad" messages, but between those that were WRITTEN and those that LEAKED.

**Exemplary ones — written by us, lying in the catalog, translated into five languages**
(`src/apps/kt-app/i18n/*.toml`, sections `[msg]` and `[perf.note]`):

| Key | Cause | Consequence | Action |
|---|---|---|---|
| `msg.media_missing` | "source unavailable" | "a placeholder is shown" — work goes on | none (the file's return is caught by itself, `msg.media_online`) |
| `msg.software_render` | "no graphics acceleration" | TWO consequences named individually: "the UI may lag, preview glow is off" | none — and rightly so, the user is powerless here |
| `msg.video_fx_gpu_lost` | "GPU video renderer lost ({0})" | "switched to the CPU" | none |
| `msg.ktproj_import_only` | an old format | does not open as a project | **there is one, with an address: "via File / Import .ktproj… only"** |
| `msg.empty_lanes` | "no events on those tracks" | "the order of empty ones does not matter" — reassurance instead of an action | not needed |
| `msg.nothing_to_copy` | "no region and no selection" | "nothing to copy" | implied |
| `msg.paste_skipped_*` | "no targets survived" / "the tick is taken" | N skipped, the rest pasted | none |
| `perf.note.low_video_hit` | a measurement "video hit {0}%" | **a conclusion in human language**: "decoding cannot keep up" | none |
| `perf.note.eviction_spike` | "{0}/s" | "the cache is too small" | **nowhere to go**: the budget is `CacheConfig::default()`, `src/libs/kt-engine/src/player.rs:189`, there is no setting for it |

The seven threshold messages of `[perf.note]` (`ui_stall`, `render_underruns`,
`callback_underruns`, `eviction_spike`, `prefetch_thrash`, `low_video_hit`,
`offline_changed`) are the only place in the program where the MEASUREMENT is separated from the
CONCLUSION: first a number, then a phrase in human language. This is the ready model of the anatomy.

The same model, laid out by stages of work — a missing source (R.33 item 10): `msg.media_missing` on opening
(`src/apps/kt-app/src/app.rs:685`), `render.missing_warn` BEFORE the render starts (`src/apps/kt-app/src/render_dialog.rs:400`),
`render.missing_after` in the summary afterwards (there, 556). What Resolve hides in the setting "do not stop", we have as
the default behaviour. The analysis of this chain as a whole is **F-HLP-012**; here it is needed only as proof that the
anatomy is already ours and reproducible.

There is also a correctly built ANCHOR, twice, and both times not on purpose:
`averr()` in `src/libs/kt-ffmpeg/src/source_impl.rs:57` assembles
`«{context}: {av_strerror} ({code})»` — a human text plus a machine code in brackets, exactly as AE does it; and
`msg.video_fx_gpu_lost` carries `({0})` = `DeviceLost`. Both anchors today arrive inside messages that have neither
help nor a search to take the anchor to.

### What is missing

**1. Localisation.** The English string of `thiserror` travels to the status bar past the catalog, past t125 and past
the 160-character ceiling. Counted from the code:

- `kt_model::CommandError` — 24 variants (`src/libs/kt-model/src/model.rs:283`);
- `kt_model::ProjectError` — 33 (`src/libs/kt-model/src/project.rs:3001`); they are nested into the first through
  `InvalidClip`/`InvalidLayout`/`InvalidTransition`/`InvalidCamera`;
- `kt_otio::Error` — 10 (`src/libs/kt-otio/src/lib.rs:36`), including `KtMetaBroken`, `UnsupportedSchema`, `UnsupportedRoot`, `NonFinite`;
- `kt_engine::export::ExportError` — 5 (`src/libs/kt-engine/src/export.rs:280`), including `GpuRequired` and `FfmpegFailed`;
- `kt_ffmpeg::FfError` — 4, `kt_media::MediaError` — 2.

In total **77 named variants, of which exactly ONE is translated**: `kt_otio::Error::NotOtio` is recognised by name in
`src/apps/kt-app/src/app.rs:4963` and is replaced by the key `msg.ktproj_import_only`. All the others go through the
neighbouring line `Err(e) => Err(e.to_string())`. Beyond the 77 there is an open set of `std::io::Error` and 14 free English
notice strings which kt-otio generates directly in the library (`src/libs/kt-otio/src/convert.rs`, `legacy.rs`) and which
`app.rs:679` and `app.rs:5343` print to the user as they are.

**Places where a foreign string reaches the user: TWENTY.** Eighteen are calls of `note()`/`warn()` in
`src/apps/kt-app/src/app.rs` (357, 679, 685, 777, 1098, 1984, 2094, 2732, 2796, 3145, 3182, 3777, 4065, 5310, 5317, 5336, 5343,
5346), and two are in the render window (`render_dialog.rs:239` → `574`, `render_dialog.rs:90` → `400`/`556`). Of the twenty,
**six have not a single word of ours around the foreign text** — `app.rs:1984`, `2094` (`self.note(e.to_string())` in `apply()` and
`apply_batch()`), `3145`, `3182`, `4065` (`format!("{path}: {e}")`) and `3777`. The remaining fourteen wrap the English tail in a
Russian frame (`msg.open_failed`, `msg.save_failed`, `msg.autosave_failed`, `render.error_msg`), which produces a hybrid
"not opened: metadata.kt is unreadable: …".

A side effect of the same `to_string()`: the variants are formatted with `{0:?}`, so the user reads `clip ClipId(7) not found` — an
internal type name instead of the clip name, which kt-app has in `names_cache`.

**2. A level as a field of the message.** `enum Severity` (`src/apps/kt-app/src/app.rs:15`) knows `Info` and `Warn` and does not know
`Error`, so a rejected model command travels at the same level as "clips copied: 3". The level is not a property of the channel, but a
PART of the anatomy: it says "the work goes on" or "the work was not done" before the text is read, and it is by this that VS Code declares
its contract of three kinds. Where messages go after they are born and how to re-read them later — **F-HLP-013**; whether the refusal of a
gesture itself can be shown in red — **F-HLP-010**. Here it matters only that there is nothing to name it with.

**3. The fourth part.** `grep -c http` over all five catalogs gives **0**: no message leads anywhere. An address where the decision changes
(rule 19) is named by exactly one value out of 522 — `msg.ktproj_import_only`.

**4. Silence.** A failure of the audio output is printed by `eprintln!` (`src/apps/kt-app/src/app.rs:389`) and does not reach the user at all: there is
no sound and no message. This is exactly the class because of which `KtMetaBroken` was created in R.35; the channel itself — "no error is lost" — is the subject
of F-HLP-013, and it is named here because a message without text is also an anatomy, with zero of the three parts.

**5. Cancellation disguised as an error.** A pressed "Cancel" button leads to `ExportError::Cancelled` (`export.rs:517`), and the render window paints it red
through `render.error_msg`: "Error: export cancelled". The user did exactly what he wanted and received an error.

### Special case: `GpuRequired` knows the names and reports a count

`ExportError::GpuRequired(usize)` (`src/libs/kt-engine/src/export.rs:293`) is the most expensive message of the program: it is the only PREFLIGHT that refuses
before the file is created, and the R.29 decision "a silently corrupted file is worse than an honest error". It is built like this:

```
let need = gpu_required_clips(project);      // export.rs:405 — Vec<ClipId>
if !need.is_empty() {
    return Err(ExportError::GpuRequired(need.len()));   // export.rs:407
}
```

`gpu_required_clips()` (`export.rs:298`) deliberately reduces the ids to those that the user SEES: the comment in the code says it plainly — otherwise
"the preflight would name synthetic ids which the user does not see in the project", and for this the pieces of the morpher layer are collapsed through
`slice_origin`. So the coordinate has been computed, reduced to the user's view — and on the next line it is replaced with `.len()`. The message comes out as
"project uses 3D effects on *N* clip(s), but no GPU is available": the cause is there, the consequence is there, the coordinates are not there, the action is not
there, the language is not there.

Two more things pass by. First: the render settings dialog can warn BEFORE the start — `missing_sources()` is computed on opening
(`render_dialog.rs:175`) and is drawn in the window (`400`) — but `gpu_required_clips()` is not called there, so the only real preflight of the program fires
AFTER pressing "Render…", not before. Second: which exact effect requires the GPU is not named, although `requires_gpu()` is asked of the concrete `FxInstance`
(`src/libs/kt-model/src/project.rs:2340`).

### What gets in the way

The test t131 (`src/apps/kt-app/src/i18n_tests.rs:219`) forbids Cyrillic in the literals of ALL non-test `.rs` files of the workspace, not only
kt-app. So translating the text "in place", in kt-otio or kt-engine, is physically impossible, and the right form is not "make the messages nicer" but
**return a VALUE rather than a string at the boundary**: `e.to_string()` is replaced by an exhaustive `match` on the variant in kt-app, and then a new
variant in the library breaks the BUILD, just as a missing catalog key does. This is the same device that is already used for `NotOtio` — it has to be
extended to the other 76.

The second bottleneck — the 160-character ceiling (`src/apps/kt-app/build.rs:42`, `MAX_VALUE_CHARS`) — stands on the VALUE of the catalog, not on the
substituted result: `msg.media_missing` is 52 characters long and, with the `averr` tail, gives almost two hundred. The section "Estimate" takes this into
account; the overall account of the price of the line — F-HLP-033.

The third: the clip names live in kt-app (`names_cache`), and the error is born in kt-engine, which knows nothing about names and should not. So the
coordinate is carried not by the text of the message, but by the LIST OF IDS in the error variant, and the name is given to it by kt-app — exactly as
`msg.media_missing` receives `label` from `crate::util::media_label`.

### Which invariants it touches

None of the twelve root ones — and this is the main argument for "cheap": neither the time model, nor the cache, nor the threads, nor determinism is involved here.

The format invariant **"loading errors are explicit"** is touched (the defect `p` of the old format, `docs/porting-notes.md:72`, closed by test 12). It is precisely
this that produced `KtMetaBroken` in R.35 (item 243: a broken `metadata.kt` was opened silently, and the user would have lost tracks, morphers and the layout). The irony
lies exactly here: the variant was created so that the error would be EXPLICIT — and it is the only one of the ten that reaches the user as an untranslated English string.

Rules of the UI checklist: **19** (the dialog must say where the decision can be changed later) — not fulfilled by `GpuRequired` or `eviction_spike`; **18** (say WHAT and WHERE) — fulfilled
by one value out of 522; **16** (impossibility = an ERROR) — violated by the level `Severity`; **7** (the text fits) — under threat from the tails not counted by the 160 ceiling.

## Estimate

| What | Answer |
|---|---|
| Size | M — 2 stages of the round: (1) the boundary "value instead of string" + the catalog keys; (2) the coordinate of the failure and the level |
| Where the main work is | kt-app (exhaustive `match` → keys, `Severity::Error`), kt-engine (`GpuRequired` carries `Vec<ClipId>`, the preflight is called from the dialog), kt-otio (the notice as a value, not a string) |
| Project format | does not touch it |
| Tests that will have to be written | `i18n_tests::every_error_variant_has_a_catalog_key` (an exhaustive parse of all error variants, not a single `to_string()` on the path to the UI); a harness test "a cancelled render is neither red nor an error"; a harness test "a 3D refusal names the clips and selects them" |
| Depends on | F-HLP-010 (the red line of a refusal), F-HLP-012 (the chain about the source), F-HLP-013 (the channel and the log), F-HLP-033 (the price of the catalog line) |
| Breaks compatibility | no |

A frame, not a spec: the concrete set of keys, the wordings in five languages and the order of the stages are the subject of the round.

## Verdict

We take it, and cheaply: not a single invariant is touched, and the model has already been written by us — `[perf.note]` and the three levels of the message about a missing source.
The point is not to learn how to write messages, but for THREE QUARTERS of the program's messages to stop being written by `thiserror` past the person.
Separately and first of all — `GpuRequired`: it has already computed the coordinate and thrown it away in the same function.

TEST: i18n_tests::every_error_variant_has_a_catalog_key — today it fails on the first variant: of the 77 named error variants of kt-model/kt-otio/kt-engine/kt-ffmpeg/kt-media exactly one is translated (`NotOtio` → `msg.ktproj_import_only`, `src/apps/kt-app/src/app.rs:4963`); the rest reach the user at twenty places, six of them without a single word of ours around them, and neither t125 (catalog completeness), nor t131 (Cyrillic in literals), nor the 160-character ceiling (`src/apps/kt-app/build.rs:42`) sees this, because all three look at the CATALOG, and these strings do not go into the catalog.

CHECK: src/libs/kt-engine/src/export.rs — the clause "the preflight returns `GpuRequired(Vec<ClipId>)`, not a count, and is called from the settings dialog before the button, not from `export_mp4_with_backends` after it": `gpu_required_clips()` already reduces the ids to those visible to the user through `slice_origin`, and `export.rs:407` today erases this work with the call `.len()`.

## Sources

None of the five core editors was run live in this pass; the Russian-language summary of the rows comes from the official
materials of the vendors and from forums; the access dates are 2026-09-07.

- Adobe FAQ "Fix Error Compiling Movie errors in Premiere Pro" (the composition of the dialog:
  class, timecode, Error Code; quote by Kevin J. Monahan, 18.02.2022) —
  https://community.adobe.com/t5/premiere-pro-discussions/faq-fix-error-compiling-movie-errors-in-premiere-pro/m-p/12757295
- Creative COW, VEGAS Pro forum, a thread about "The reason for the error could not
  be determined" (quote by Nigel O'neill, 08.11.2010) —
  https://creativecow.net/forums/thread/an-error-occurred-while-creating-the-media-file-na/
- Creative COW, VEGAS Pro forum, "Exception thrown by method called" — the second kind of the same message —
  https://creativecow.net/forums/thread/sony-vegas-12-render-as-an-error-occurred-while/
- Blackmagic Design Forum, "Render Job failed please help!" (the behaviour of the render queue, the percentage as a coordinate) —
  https://forum.blackmagicdesign.com/viewtopic.php?f=21&t=205830
- Description of the Resolve setting "Stop renders when a frame or clip cannot be
  processed" (Preferences ▸ User ▸ UI Settings) — a secondary source, live
  [NOT VERIFIED] —
  https://motionarray.com/learn/davinci-resolve/davinci-resolve-render-crashes/
- Adobe Product Community, threads about "Error 86::1" (the form of AE's paired code and the fact that the vendor does not document it) —
  https://community.adobe.com/t5/after-effects-discussions/error-86-1-after-effects-2020-in-windows-10/m-p/10782599
- Apple Support Community, a thread about an export error in Final Cut Pro (the form of the alert; the text was read through a text proxy,
  the forum's markup does not open in this environment) — https://discussions.apple.com/thread/255390795
- VS Code UX Guidelines, Notifications (three levels, action buttons, "Do not
  show again") — https://code.visualstudio.com/api/ux-guidelines/notifications
- Nielsen Norman Group, "Error-Message Guidelines" — the source of the wording "do not blame the user" and of the requirement for constructive advice —
  https://www.nngroup.com/articles/error-message-guidelines/
