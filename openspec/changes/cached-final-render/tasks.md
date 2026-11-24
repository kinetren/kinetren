# Tasks: cached-final-render

> The owner answered Q1–Q3 (kkt-26.11.20 A16); an executor builds it in stage 24 on the orchestrator's go. Tests reserved: t3322–t3335, t3870–t3876. Base: orch `ec30cb02`,
> revised on `5e7a8ea9` after his answers (Q1 b, Q2 a, Q3 b).

## 0. Notes first

- [x] 0.1 F-EXP-003 gains the owner's later word (X1, U5); `features-index.py --check` green
- [x] 0.2 proposal, design (D1–D10, class walk D9), deltas `export`, `video-plan`;
      `openspec validate cached-final-render --strict`
- [x] 0.3 his answers Q1–Q3 (kkt-26.11.20 lines 21–22: draft render, yes, SEI) (orchestrator: `docs/user-feedback.md`, `docs/owner-checks.md`); found-defects
      773 gets its verdict from Q1; `v26.10-index.md` X1 and `kkt-26.11.18b-index.md` U5 point here

## 1. The index

- [ ] 1.1 the fingerprint (D3); t3323
- [ ] 1.2 per-frame keys at the export's quality and the per-GOP hash, written as SEI (D5); `renders.tsv` in
      the project's cache folder; the strip tool; t3870, t3871;
      t3322, t3324
- [ ] 1.3 the durable key's layout guard; t3335

## 2. Keyframes and parameter sets

- [ ] 2.1 closed GOPs at edit identity changes, spacing from there (D2); t3325
- [ ] 2.2 in-band parameter sets, `avc3`/`hev1` in MP4 (D4); t3326

## 3. The copy

- [ ] 3.1 `Muxer` packet input; a reader of our own outputs' packets by the GOP table
- [ ] 3.2 candidate lookup, ordering by index hash, whole-GOP matching, re-stamping; encoder restart around
      copies; t3327–t3330
- [ ] 3.3 the preview's tier stays out (counter); t3331

## 4. The window and kt-cli

- [ ] 4.1 the "Cached render" step, the strip, the estimate; summary; i18n in every catalog; `reach_tests`,
      `layout_tests`, font tests; live scenario with screenshots; t3332, t3334
- [ ] 4.2 `kt-cli export --cached`; t3333
- [ ] 4.3 What's new line; F-EXP-003 status update

## 5. The draft render

- [ ] 5.1 the option in the render window, the prerender's frames decoded and scaled, the rest at the
      preview's quality, the draft tag, the summary; kt-cli `export --draft`; i18n; t3872–t3876; planned
      live scenario `draft_render` with a screenshot per step
