# Tasks: subtitles-and-text-editing

> Decided 2026-11-20 (A11); revision 2 implementation-ready at `fc60a5d9` (2026-11-25). Stage A may start on the orchestrator's go
> (after `shared-events`, MODEL); stage B.2 needs `download-library` 2, Delete by text `ripple-and-trim-modes`. Tests: t3460–t3465,
> t3467–t3474, t4210, t4250–t4252.

## 0. Notes

- [x] 0.1 F-TXT-004; proposal, design (D0–D11), deltas `transcript`, `subtitles`, `project-format`, `ui-reach`; `openspec validate subtitles-and-text-editing --strict`
- [x] 0.2 his answers Q14–Q16 (kkt-26.11.20 line 19); planned OTIO rows

## 1. Stage A — captions without recognition

- [ ] 1.1 kt-model: `Track.sub`, `Caption`, `SubStyle`, commands, validation, `TlEntity::Caption`; t3467 (the splitter over a synthetic word list), t4250
- [ ] 1.2 kt-media `subtitles_io` (SRT, VTT); t3468
- [ ] 1.3 `GeneratorKind::Captions`, `caption_event`, `holes`, kt-text `highlight`; t3469, t4251
- [ ] 1.4 the row, its gestures, the in-place editor, the style form (the title form's widgets); File › Import subtitles…; t4252
- [ ] 1.5 the render window: burn-in (the track's own switch), sidecar; kt-cli `--subtitles`; t3470
- [ ] 1.6 the next free file version, kt-otio metadata and `.srt`; t3471, t3472

## 2. Stage B — the transcript and the Text panel

- [ ] 2.1 `Transcript`, `Word`, visibility, the string form; t3460
- [ ] 2.2 `kt-speech` (dlopen), the `speech` lane, the Library line; t3461 (runs where `KT_SPEECH_DIR` is, says so where not), t3462
- [ ] 2.3 the Text tab: follow, click, search, region, retype, Delete; t3463, t3465
- [ ] 2.4 fillers; t4210. "Create captions"; menus; live `text_panel`, `captions` with a screenshot per step; t3464, t3473, t3474
- [ ] 2.5 font packs in the style (`download-library` 4.1), the notice of a missing script

## 3. Close

- [ ] 3.1 What's new (two lines, one per stage); F-TXT-004 verdict; ★14, B5, N4, A11 moved
- [ ] 3.2 idle with the panel open on 6 000 words; `finish.sh` (orchestrator, per stage)
