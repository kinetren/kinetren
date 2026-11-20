# Tasks: subtitles-and-text-editing

> DESIGN ONLY until the owner answers Q1–Q3 (`proposal.md`). Depends on `ripple-and-trim-modes`
> (`ripple_plan`) for text deletion. Tests reserved: t3460–t3484.

## 0. Notes first

- [x] 0.1 F-TXT-004 refreshed: the owner's "downloaded on a click" revises the weights verdict
- [x] 0.2 proposal, design (D1–D11, class walk), deltas `transcript`, `subtitles`, `project-format`, `ui-reach`; `openspec validate subtitles-and-text-editing --strict`
- [x] 0.3 planned rows in "What OTIO export loses" (`docs/ktprompts/v26.10-index.md`)
- [ ] 0.4 the owner's answers Q1–Q3 in `docs/user-feedback.md`, folded into the design

## 1. Stage A — captions without recognition

- [ ] 1.1 kt-model: `LaneKind::Subtitle`, `SubTrack`, `Caption`, commands, validate (no overlap), ripple joins captions; t3467
- [ ] 1.2 kt-media: SRT/VTT parse/write; t3468
- [ ] 1.3 kt-engine: the subtitle procedural source (`SUB_TAG`), the layer at its row; kt-text highlight input; t3469
- [ ] 1.4 render window: burn-in, sidecar; t3470
- [ ] 1.5 format by content, kt-otio metadata and `.srt`; t3471, t3472

## 2. Stage B — transcript and the Text panel

- [ ] 2.1 kt-model: `Transcript`, `Word`, word visibility through `clip_material_at`; t3460
- [ ] 2.2 the recognizer component download (the `ffmpeg_setup` card pattern), dlopen loader, the job; t3461, t3462
- [ ] 2.3 the Text panel (follow, click, search, region, Delete through `ripple_plan`, retype); t3463, t3465
- [ ] 2.4 pauses from the pyramid, fillers from the catalogue; t3466
- [ ] 2.5 menus and the live scenarios `text_panel.py`, `captions.py`; t3464, t3473, t3474
- [ ] 2.6 font packs on a click (Q3 a), the notice for a missing script

## 3. Close

- [ ] 3.1 What's new; F-TXT-004 verdict → `CAPABILITY: openspec/specs/subtitles/`
- [ ] 3.2 idle measurement with the panel open on a long transcript
- [ ] 3.3 `src/scripts/finish.sh` (orchestrator), version in the report
