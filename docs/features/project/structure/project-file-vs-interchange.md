---
id: F-PRJ-005
title: PROJECT format versus EXCHANGE format
status: accepted
priority: A
cost: L
category: project/structure
editors: [vegas-pro-2026, premiere-26, resolve-21, fcp-12, after-effects-26, kdenlive-26.08, otio-lib-0.18]
we_have: yes
crates: [kt-otio, kt-model, kt-app, kt-cli]
invariants: [11]
ui_rules: [19, 20]
round: 70  # closed, specs merged 2026-10-03
point: null
tests: [t810, t811, t812, t813, t814, t815, t816, t817, t818, t819, t820, t821, t822]
depends_on: [F-PRJ-002, F-PRJ-003, F-RES-003]
trackers: []
checked: 2026-10-03
---

# F-PRJ-005 — PROJECT format versus EXCHANGE format

## Essence

An editor has two different files, and they are easy to confuse: the **project file** (where the work lives; it is rewritten by every autosave) and the **exchange file** (it is handed to a foreign program). The requirements for them are opposite. The exchange file must be SELF-SUFFICIENT: the reader does not know our conventions, so everything it must show lies in it in expanded form. The project file must store each entity ONCE and refer to it, otherwise reuse (a nested composition placed in five places) multiplies the file.

Until R.70 Kinotrain kept the project in the exchange format (OpenTimelineIO) and paid for it with exactly what the exchange format does not protect against: the body of a composition was written anew into EACH of its instances, and for the owner three reels gave **15 220 clips and 115 MB** per file, which autosave writes regularly.

## Who has it and how

| Editor | Project file | Exchange file |
|---|---|---|
| DaVinci Resolve 21 | **a database** (Resolve Disk Database — a catalog managed by the program itself; studios have PostgreSQL on network storage) | `.drp` — a ZIP with XML, made by an explicit command `File → Export Project`; AAF/EDL/XML/OTIO |
| Final Cut Pro 12 | a **bundle catalog** `.fcpbundle`, inside `.flexolibrary`/`.fcpevent` — SQLite databases | FCPXML |
| Adobe Premiere 26 | `.prproj` — its own container, compression on save is enabled by default and is disabled for speed | AAF/EDL/XML/OTIO |
| After Effects 26 | `.aep` — its own binary; `.aepx` — the same thing in XML form | — |
| VEGAS Pro 2026 | `.veg` — its own binary | AAF/EDL/XML |
| Kdenlive 26.08 | MLT XML — the exchange format of the engine on which Kdenlive is built | the same MLT XML |
| OTIO 0.18 | — (a library, not an editor) | the exchange format itself |

## The trick behind it

**No major editor lives in an exchange format — and this is not a matter of taste, but a consequence of the fact that an exchange format has NO OBJECT IDENTITY.** In OTIO JSON (as in FCPXML and in AAF compositions) one cannot write "this composition is the same one as above": a reference to an object in the document is not expressed, only nested content is expressed. Therefore self-sufficiency and non-repetition are mutually exclusive requirements, and the editors choose differently for different files.

A detail that is absent from the feature lists: Resolve stores the project as a **record in a database**, and `.drp` can be produced only by an explicit command from a person — that is, for it exchange is not saving at all. FCP hides SQLite databases inside `.fcpbundle`, that is, the "file" of a project is in fact a relational middle. Premiere is the closest to us (its own container + compression), and it is precisely there that the price of the error is publicly visible: compression on save cuts 30–50 %, works on ONE core and is disabled for speed — that is, the size of its project file has already become a performance problem, not a problem of disk space.

A second observation, directly related to our §464: **Premiere copes well with only ONE level of nesting** — A in B works, B in C starts behaving strangely. Adobe's answer to growth is not to fix the format, but to CUT the project into parts (the "Productions" workflow). That is, the industry has already run into the same wall, and goes around it organizationally rather than engineeringly.

## Pros

- The project file becomes the size of the MODEL, not of its expansion: the measurement of R.70 on a synthetic scene (a reel of 10 clips, two nesting levels with 8 instances each)
  — `.otio` 1.97 MB against 3.9 KB of direct model serialization, **501×**; on a scene
  40/8/8 — 7.36 MB against 7.9 KB, **929×**. A flat project without repeats — 17×.
- Autosave stops being an event: writing megabytes every half a minute is both interface lag, disk wear, and a loss window on a crash.
- The model stops being squeezed through a foreign schema. Today morphers, cameras, the layer map, overlapping transitions, public parameters, row heights and the track order live in the `metadata.kt` "envelope" — that is, our format wrapped in someone else's, lies in the exchange format.
- Breaking changes become cheap: the version is our own, and there is no cascade of compatibility with a foreign schema.

## Cons

- **A foreign editor will no longer open the project file directly.** Today `.otio`
  can be handed to Resolve as is; tomorrow an explicit export will be needed for that.
  This is exactly the trade that all the others make, but for us it was the other way round.
- TWO doors are needed instead of one: importing existing `.otio` projects of the owner and
  exporting to `.otio` for exchange. Both will have to be maintained and verified.
- Our own format means our own responsibility for durability: the reader of a foreign version,
  behavior when a write is cut off, recovery. With an exchange format, a foreign ecosystem
  partly did this work.
- While the format is our own and young, "it opened, but part of the edit is missing" (F-RES-003)
  becomes more likely, not less.

## What users say

Premiere, Adobe Community, duanecwilson, 06.02.2019 — about a project file without media
(https://community.adobe.com/t5/premiere-pro-discussions/help-reducing-huge-project-file-prproj/td-p/10352545):

> my project file (not including media) is almost 1GB in size, and every time I
> save it, or it is auto-saved, it creates another 1GB file.

There, about the price of saving:

> it still takes a full minute or two to save it.

The industry's negative experience can also be read in the workarounds: Adobe answers growth not with a format but by cutting the project ("Productions"), and compression on
save in Premiere is disabled for speed — that is, the user is offered a choice between the size of the file and the time of saving.

Our own owner (`docs/user-feedback.md`, R.68 and R.70): three reels gave
115 MB and 15 220 clips; the decision — "we are moving away from the otio format, leaving otio
only for export, and making breaking changes so that our format becomes a hundred times
better".

## How it fits Kinotrain

CAPABILITY: openspec/specs/project-format/

The project moves into ITS OWN format, `.otio` remains a door outward (export) and
a door inward for existing files (import, rule R.67).
The owner's decision of 02.10.2026, round R.70; breaking changes are explicitly allowed.
The "before" measurement: 17× on a flat project, 501× and 929× on nested scenes.

## Estimate

Cost L: the format, reading, writing, `.otio` import, `.otio` export, autosave and recovery, a cascade of versions of our own format, live scenarios for both doors.
The gain is measurable and large (hundreds of times in file size), and it also removes the separate task §464 entirely — the growth disappears by construction, rather than being plugged.

## Verdict

**ACCEPTED (R.70, the owner's decision).** The exchange format and the project format are different files with opposite requirements; the attempt to sit on both chairs gave
115 MB. We take the separation that the industry made before us, and take it whole:
the project in its own format, OTIO — for export and import.

Rejected along with this: the R.68 option "expand a nest by one level" —
it rested on the wrong premise that the bodies of reels already lie in the envelope
`kt.sequences`; in fact there are no fields with clips there at all, and truncating the depth would take the content away from OUR reader.

**DONE (R.70, 03.10.2026).** The project file `.kt` — `kt_model::project_file`
(line-by-line deterministic JSON of the model, a version at the root, refusal forward);
OTIO — two doors: `File → Import older Kinotrain project…` (legacy `.ktproj`,
our old and a foreign `.otio` — one rule `kt_otio::import_interchange`) and
`File → Export → OpenTimelineIO…` (self-sufficient, the body of a reel in each
instance, foreign passthrough restored). Autosave and recovery —
`<name>.recovery.kt`. Measurement on the scene 10/8/8: the project file is 234× smaller than the
exchange file (line-by-line against compact cost 2×, and this is the price of git). kt-cli: `new`/
`demo` write `.kt`, `import`/`export-otio` are the doors, `render`/`probe`/`export`
read only `.kt`. Live scenarios go into the program through the same doors
(`kt_live.import_project`/`export_project`).

## Sources

- https://www.danielgrindrod.com/blog/databases — Resolve Disk Database versus PostgreSQL (read 02.10.2026)
- https://cutconvert.com/guides/what-is-a-drp-file — `.drp` as a ZIP with XML, made by an explicit command (02.10.2026)
- https://preservation.tylerthorsted.com/2023/12/15/final-cut-pro/ — `.fcpbundle`, inside `.flexolibrary`/`.fcpevent` (SQLite) (02.10.2026)
- https://community.adobe.com/t5/premiere-pro-discussions/help-reducing-huge-project-file-prproj/td-p/10352545 — quotes by duanecwilson, 06.02.2019 (02.10.2026)
- https://vakago-tools.com/faster-saving-times-in-premiere-p/ — project compression on save: 30–50 %, one core, disabled (02.10.2026)
- https://blog.frame.io/2024/09/30/ultimate-guide-to-premiere-pro-productions/ — "Productions" as the answer to large projects; one level of nesting (02.10.2026)
