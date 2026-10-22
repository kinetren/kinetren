---
id: F-COL-008
title: What a foreign file loses when it passes through us
status: candidate
priority: B
cost: S
category: collab/handoff
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, otio-lib-0.18]
we_have: partial
crates: [kt-otio, kt-model]
invariants: []
ui_rules: []
round: null
point: null
tests: []
depends_on: [F-COL-007, F-RES-003]
trackers: [otio, losslesscut]
services: [premiere-productions, fcp-shared-library, bm-cloud]
checked: 2026-09-08
---

# F-COL-008 — What a foreign file loses when it passes through us

## Essence

A colleague sent an edit that was made not in our program. We opened it, moved one clip,
saved it — and sent it back. The note has one question: what from his document did not
make it back. Not "did it open" (that is F-RES-003 and F-COL-007), but precisely the LOSS
on the round trip: a comment at a marker, a disabled track, an effect we do not
know. The loss is silent by definition — there was no error, the file opened, the save
went through, and it is the file's author who learns of it, not us.

## Who has it and how

The column "How it works" answers one question: what happens to a FOREIGN file
after "open → save". The choice of output format is F-COL-007; the versions of OUR
format and our own old build are F-RES-003.

| Editor | Version | What it is called | How it works |
|---|---|---|---|
| VEGAS Pro | 2026.0 | Import (AAF / EDL / fcpxml) | there is no round trip at all: the foreign file is IMPORTED into `.veg` and stops existing as a document. Back comes a NEW file, generated from the Vegas model — agreement with the original is neither guaranteed nor verified, because there is nothing to compare it against |
| Adobe Premiere | 26.x | Import FCP XML / AAF, then a separate Export | the same: two independent paths, input and output. Symmetry is promised by no one, and it regularly fails — there is a live thread from 2014 about markers that did not survive the round trip through Premiere's own FCP XML |
| DaVinci Resolve | 21 | Import Timeline, Conform Lock, Reconform | the foreign file becomes a RECORD of the database, not a file; therefore "save back" is not a save but a new export. In exchange it gives what the others lack: `Conform Lock` and a repeated conform, that is, a mechanism for re-seating onto an updated foreign file |
| Final Cut Pro | 12.3 | Import XML → library | the harshest form: the foreign `.fcpxml` is drawn into the library package, and the file is no longer needed after that. The return `.fcpxml` is a generation from the library, and by content it does not match the accepted one (FCP has its own role model, which the source did not have) |
| After Effects | 26.x | Pro Import After Effects | the round trip is physically one-way: there is an importer, there is no exporter at all. A foreign edit comes in and does not go out — "loss on the round trip" is not defined here, because there is no round trip |

**Outside the core.** The five core editors give five ways of NOT having a round trip, so the
class cannot be described through them: for them a foreign file is always imported, and the
question "what was lost" has no addressee. OpenTimelineIO is the only place where the round
trip is declared a property, where the loss of each adapter is discussed by name in a
public tracker, and where the idea was born that "an unknown schema does not break the
reading, but becomes an object that remembers itself verbatim" (issue #326, 2018) — the direct
ancestor of our `RawObject`.

| Program | Version | What it is called | How it works |
|---|---|---|---|
| OpenTimelineIO | 0.18.x | `SerializableObjectWithMetadata`, `UnknownSchema`, `otio_json` | the round trip is a subject of testing, not a side effect: each adapter has its own list of what it reads and what it writes, and these lists are DIFFERENT (issue #1050: the ALE adapter reads CDL, but does not write it back). An unknown schema is preserved verbatim and survives writing — the same decision as ours, and it was taken earlier than ours |

## The trick behind it

**Passthrough protects foreign schemas better than native ones — and this can be checked on our
own fixture.** `decode_marker`
(`src/libs/kt-otio/src/schema.rs:443`) types EXACTLY one version — `Marker.2`.
Everything else goes into `RawObject` and survives the round trip verbatim. And the current
version of the schema in the reference library is `Marker.3`: the field `comment` was added to it by
PR #1698 of 15.02.2024. So **not a single marker of a real foreign file passes through the typed path**:
in `src/fixtures/otio-samples/premiere_example.otio` all four markers are `Marker.3`, in
`screening_example.otio` all three are. They come back untouched — and at the same time they do not
appear on our timeline at all, because nobody turns them into `kt_model::Marker` there. Preservation
here is a consequence of invisibility.

**And the reverse: what we UNDERSTOOD gets thinner.** Our reader (`convert.rs:700`) for `Marker.2` takes four
fields — time, duration, name, colour — and our writer (`convert.rs:393`) leaves `comment`, `metadata` and `extra`
empty. The comment of a foreign marker disappears at the first save, and no notice is printed about it.
From this follows a rule worth saying aloud: **for us, understanding costs, and not understanding is free.** It is
confirmed by a second, independent place: a DISABLED foreign clip (`Composable::Clip(c) if c.enabled` does not
match, `convert.rs:849`) is not imported at all — it becomes a grey `ForeignEvent` with the raw data in
`Retained.foreign` and returns to the file byte for byte, with all its effects and markers. An enabled clip is understood — and loses exactly
what is absent from our model. The less we parsed, the more intact we return it.

**The third detail — about a version we signed for ourselves.** We WRITE our markers as `Marker.2` and put into them the field `comment`, which
appeared in `Marker.3` (this is visible directly in the committed reference `src/fixtures/demo-3d.otio`, where
next to each other stand `"OTIO_SCHEMA": "Marker.2"` and `"comment": ""`). That is, the version tag 2 carries
the field of version 3. A strict reader of the reference has the right not to expect this field in v2, and our own upgrade path
`Marker.2 → Marker.3` is absent — although the only written upgrade in the crate (`Clip.1 → Clip.2`) shows that
the mechanism for it exists and costs ten lines.

## Pros

- Foreign `metadata` namespaces are not touched at all, our `kt` is appended at the
  end: a file that has passed through us remains readable by the foreign editor.
- Unknown FIELDS of known schemas are preserved in their places
  (`#[serde(flatten)] extra`), and the key order is canonicalised and frozen by
  tests — that is, the diff of a foreign file after our round trip can be used.
- The round trip is checked, not declared: t154 runs nine upstream foreign samples
  and requires idempotence of re-saving; t145 — that what was not understood came back;
  t144 — that `write(parse(write(x))) == write(x)` byte for byte.
- A broken `metadata.kt` section is an EXPLICIT error, not "so the file is foreign"
  (`Error::KtMetaBroken`, items 243–244): silently discarding the envelope would carry away
  the settings, tracks, morphers and layout.
- A version of the structural spine newer than the supported one is a hard refusal, not a half-understanding:
  the positions of a track's children would become a guess.
- A disabled foreign clip and a nested `Stack` are not "supported half-way", but honestly become an opaque placeholder with raw data — and come back whole.

## Cons

- **Markers of a foreign file are invisible.** Real files carry `Marker.3`, we type only `Marker.2`;
  the timeline of a foreign edit arrives without a single marker, although the file has four.
  The user sees neither the markers nor the reason.
- **The comment of an understood marker is lost silently.** Neither a notice nor a field in the model:
  `kt_model::Marker` is `t`, `duration`, `name`, `color`, and that is all
  (`src/libs/kt-model/src/project.rs:1459`).
- **Two markers on one frame collapse without a warning** —
  `markers.dedup_by_key(|m| m.t)` (`src/libs/kt-model/src/project.rs:3488`),
  because a marker is addressed by a tick, not by an identifier. This concerns those markers that DID reach the model, that is, the root
  `Marker.2`; and that the case is not invented is visible in a real file: in `screening_example.otio` on one
  clip lie TWO markers with the same name "ANIM FIX NEEDED". The very same trouble, for the same reason, is described by the EDL adapter in otio #593.
- **The markers and effects of a TRACK disappear entirely, and this is the worst item on the list.**
  `TrackExtra` (`convert.rs:61`) stores the name, `extra`, `metadata` and gaps — and does NOT store
  `markers` or `effects`; the writer puts `vec![]` in their place
  (`convert.rs:364-365`). The same for the root `Stack` (`effects: vec![]`,
  `convert.rs:536`), for `Gap` and for `Transition`. This is not passthrough and not a placeholder — it is
  silent deletion, and not a single notice is printed.
- **A disabled foreign TRACK comes back enabled.** The writer sets `enabled: true` unconditionally (`convert.rs:366`), the reader does not read the field. The file
  goes back to its author with a playing track that he had switched off — and this is exactly
  the case where the loss is not "impoverishment" but a CHANGE OF MEANING. The industry knows this gap: otio #483 asks for the clip's status precisely because there is nowhere to store it.
- **Retained is bound to tracks, gaps and transitions BY POSITION** — a documented limitation of v1: a structural edit may re-glue foreign metadata to a neighbouring object. Clips are not affected (the key is `ClipId`).
- **NTSC frequencies are rounded.** 23.976 does not divide the 90 kHz grid, `grid_fps`
  takes the nearest whole number; the project of a foreign file becomes 24 fps, and byte stability of the round trip does not hold on such files (tick per clip).

## What users say

SIGNAL: markers and metadata on read/write — 10 tasks in otio, `(marker|metadata|comment|annotation)\w*.*(read|write|writer|reader|adapter|support|preserv|round)|((read|write|writer|reader|adapter)\w*.*(marker|metadata|comment|annotation))`; share 1.5% against 0.3% in losslesscut

The topic is rare EVERYWHERE: 1.5% at otio, 0.3% at losslesscut, 0.1% at shotcut and olive,
0.0% at kde-invent. And this, unlike the count in F-COL-007, is a conclusion not about
popularity but about OBSERVABILITY. A loss on the round trip is by construction invisible to
the one who caused it: the file opened, there was no error, the save went through. The report is written
not by the victim but by the adapter's developer, who happened to notice an asymmetry in his own
code — and therefore the count for this topic measures not pain but attentiveness. Our defect
is of exactly this form: it was found by reading the code, not by a complaint.

The asymmetry "we read — but do not write" is named by the author of the adapter about his own code:

> The ALE adapter reads CDL values into the `cdl` namespaces of clip metadata dictionaries from the `ASC_SAT` and `ASC_SOP` fields, however it does not write them back out.
> — reinecke, otio #1050, 24.08.2021, still open,
> https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/1050

Our lost `enabled` is a known gap of the industry, not our invention:

> There currently seems to be no 'status' flag that comes with a clip object to convey its state on the timeline regarding whether it is active, has been disabled, muted, etc...
> — andrewmoore-nz, otio #483 "AAF/XML Reader: Clip enabled/disabled/muted flag?", 08.04.2019,
> https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/483

And our `dedup_by_key` is also a known gap: the EDL adapter loses everything except the last marker, for the same reason we lose
— a marker is addressed by time:

> Right now the cmx_3600 adapter does not support multiple markers per clip.
> — elabrosseRodeofx, otio #593, 27.09.2019,
> https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/593

A round trip that failed inside ONE program and its own exchange format:

> The markers do not come back even if I put the original video file on the timeline before importing...
> — Thomas Kohler, 01.10.2014,
> https://community.adobe.com/t5/premiere-pro/premiere-markers-export-and-import-roundtrip-via-fcp-xml-not-working/m-p/6408563

## How it fits Kinotrain

ADDRESS: src/libs/kt-otio/src/convert.rs — the marker reader (`convert.rs:700`) takes four fields from `Marker`, the writer (`convert.rs:393`) returns `comment`, `metadata` and `extra` empty, and `TrackExtra` (`convert.rs:61`) does not store either `markers` or `effects` of a track — the writer puts `vec![]` in their place (`convert.rs:364-365`). Three silent losses in one file, and none of them prints a notice, although the notice mechanism exists in the same function and is used nine times.

### What already exists

- `src/libs/kt-otio/src/schema.rs` — `RawObject` and the `decode_*` dispatchers:
  an unknown schema remains raw data with `OTIO_SCHEMA` at its original position.
- `src/libs/kt-otio/src/convert.rs` — `Retained`: `timeline_extra`,
  `timeline_meta_foreign`, `stack_extra`, `stack_metadata`, `tracks`, `clips`,
  `transitions`, `foreign`, `stack_markers_foreign`, `timeline_kt_extra`.
  A foreign disabled clip and a nested `Stack` travel via `ForeignEvent` +
  `Retained.foreign` in full.
- `src/libs/kt-otio/src/convert.rs` — the `Vec<Notice>` mechanism: nine places already
  know how to tell the user what exactly did not make it (a foreign transition type, a layer
  map that did not match, an unreadable effect, time warp).
- `src/libs/kt-model/src/project.rs` — `Marker { t, duration, name, color }` and
  `dedup_by_key` on loading.
- Tests: `libs/kt-otio/tests/format.rs` (t144, t145, t146),
  `libs/kt-otio/tests/convert.rs` (t154 — nine foreign samples), the fixtures
  `src/fixtures/otio-samples` (they hold both `Marker.3` and two same-named
  markers on one clip).

### What is missing

- A branch `("Marker", 3)` in `decode_marker` and the upgrade `Marker.2 → Marker.3` —
  of exactly the same kind as the already written `upgrade_clip_1`.
- A `comment` field and a foreign `metadata` for `kt_model::Marker`, or — if we do not want
  them in the model — storing them in `Retained` under the key of the marker, as everything
  else foreign is stored.
- `markers` and `effects` in `TrackExtra`, `effects` in the fields of the root `Stack`,
  `markers`/`effects` of `Gap` and `Transition`. This is pure passthrough: read into
  `Retained`, write back.
- Reading and returning `enabled` of a track. Today the field is written as a constant.
- A notice for each silent loss. The rule "what is not understood does not stay silent" already
  operates in the crate for effects and transitions — markers and tracks fell out of it.
- A test "a foreign file with markers, comments and a disabled track passes the round trip
  without losses". Today t154 requires idempotence of OUR output, that is, a second
  round, — and the first round (foreign file → our file) is not compared with the original by anything.

### What gets in the way

Nothing constructive: all the changes are inside kt-otio, the project format
does not change, the version `kt.v` is not bumped. The only caution is the golden files: the
addition of the `Marker.3` branch and the upgrade must not shift `src/fixtures/demo-3d.otio`
and the legacy reference of t148, and the decision "write `Marker.3` instead of `Marker.2`" will shift
them inevitably, and this has to be done in one explicit regeneration, not as a side effect.

### In the `collab/` category — two more mandatory lines (rule 13)

OFFLINE: DEGENERATES — `src/libs/kt-otio/src/convert.rs:700` — the round trip of a foreign document on one machine works in full, but degenerates into the rule "we save the better, the less we understood": the not-understood schema comes back verbatim, while the understood one thins out by the marker's comment, the track's markers and the sign of disabled-ness, and there is no one to tell about this — the file's author learns of the loss on his own side.

NETWORK COST: for a foreign edit to stop going round in circles and losing something on every lap, it must stop being a file — project-as-database — this is what all five decided: at Resolve and Team Projects a foreign file is drawn into the database record once, after that the shared record is edited, and the notion of "loss on the round trip" disappears together with the round trip itself.

### Which invariants it touches

None of the twelve — and this is a strong argument for "cheap". Only the invariants of the crate itself are touched (`src/libs/kt-otio/CLAUDE.md`): idempotence of
writing, passthrough "do not understand half-way" and the canonical order of keys.
The first of them today even masks the defect: `write(parse(write(x))) == write(x)`
checks the SECOND round, when the loss has already happened on the first.

## Estimate

| What | Answer |
|---|---|
| Size | S — one stage of the round, all the work in one crate |
| Where the main work is | kt-otio (`schema.rs`: the `Marker.3` branch and the upgrade; `convert.rs`: `TrackExtra.markers/effects`, `Stack.effects`, `enabled`, notices), kt-model (`comment` on `Marker` or an explicit refusal in favour of `Retained`) |
| Project format | does not touch `metadata.kt` and does not bump `kt.v`; switching the tag of our markers to `Marker.3` regenerates the golden fixtures |
| Tests to write | "the first round of a foreign file": a sample with `Marker.3`, a comment, two markers on one frame, markers and an effect of a track, and a disabled track — comparison with the original, not with our own output; a notice for each remaining loss; immobility of t144/t148/t189 |
| Depends on | nothing; borders on F-COL-007 (what we give out) and F-RES-003 (versions of OUR format) |
| Breaks compatibility | no |

## Verdict

We take it, and we take it whole — this is the cheapest position of the whole wave: changes in one
crate, none of the twelve invariants, the format does not change. The reason to take it
is not the markers but the rule that the defect has revealed: **for us, understanding costs, and not understanding is free**, and this is exactly
the reverse of what we promise. A foreign `Marker.3` comes back verbatim; an understood `Marker.2` loses its comment; the markers
and effects of a TRACK do not come back at all; a disabled foreign track arrives at its author playing. The last is not impoverishment but a
change of meaning, and it alone would justify the stage.

How this note differs from F-RES-003. That one is about the versions of OUR format: our `
metadata.kt.v`, our old build, our own file, and there is someone to agree with there, because both sides are us. Here the version number is not ours, the schema
is not ours, and there is no one to agree with: `Marker.3` appeared on 15.02.2024 in a foreign
release, and one could learn of it only by checking one's own dispatcher against a foreign
table of schemas. The rule from this, common and worth taking beyond markers, is:
**the list of typed versions of foreign schemas is an external dependency, and it goes stale silently.**
It has to be checked by the same method as everything else foreign — a corpus of real files — and our nine upstream samples did not
catch this hole precisely because t154 compares our output with our own output.

Boundaries: what we give the edit outward — F-COL-007; what comes back to us from the second
person — F-COL-009; the versions of our format — F-RES-003; the portability of our own
project — F-PRJ-003.

## Sources

- Service registry: `docs/features/_services.md`. Tracker registry and the counting method:
  `docs/features/_trackers.md`; the number is reproduced by
  `python3 src/scripts/tracker-rank.py --query '(marker|metadata|comment|annotation)\w*.*(read|write|writer|reader|adapter|support|preserv|round)|((read|write|writer|reader|adapter)\w*.*(marker|metadata|comment|annotation))'`
  (export date 08.09.2026, `otio` — 680 entries, `losslesscut` — 1920).
- https://github.com/AcademySoftwareFoundation/OpenTimelineIO/pull/1698 — "Add
  optional comment field to Marker", merged 15.02.2024: it is exactly this that raised the schema to
  `Marker.3` (accessed 2026-09-08)
- https://opentimelineio.readthedocs.io/en/latest/tutorials/otio-serialized-schema.html
  — the current schema `Marker.3` with the fields `color`, `comment`, `marked_range`,
  `metadata`, `name` (accessed 2026-09-08)
- https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/1050 — ALE
  adapter round-trip CDL Values (accessed 2026-09-08)
- https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/483 —
  AAF/XML Reader: Clip enabled/disabled/muted flag? (accessed 2026-09-08)
- https://github.com/AcademySoftwareFoundation/OpenTimelineIO/issues/593 — EDL
  CMX_3600 adapter: does not support multiple markers per clip
  (accessed 2026-09-08)
- https://community.adobe.com/t5/premiere-pro/premiere-markers-export-and-import-roundtrip-via-fcp-xml-not-working/m-p/6408563
  — markers did not survive the round trip through FCP XML inside Premiere itself
  (accessed 2026-09-08)
- Our code and fixtures, checked by reading on 08.09.2026:
  `src/libs/kt-otio/src/schema.rs` (`decode_marker`, `Marker`),
  `src/libs/kt-otio/src/convert.rs` (`TrackExtra`, the marker reader and writer,
  the branch `if c.enabled`, `emit_track`), `src/libs/kt-otio/CLAUDE.md`,
  `src/fixtures/demo-3d.otio` (our `Marker.2` with the field `comment`),
  `src/fixtures/otio-samples/{premiere_example,screening_example}.otio`
  (four and three `Marker.3` respectively, two same-named on one clip).
- The table "Who has it and how" is compiled from the vendors' documentation and the composition
  of the import dialogues; not verified live — there is no public tracker for any of the five
  (`docs/features/_trackers.md`, "Whom we do NOT take").
