# Tasks: shared-events

> IMPLEMENTATION-READY. The owner chose to build it now (kkt-26.11.20 A19, his Q31 b). Tests: t3367–t3375
> (spare t3376–t3378). Read first: `design.md` D1, D1a (the container), D2–D4, D6 (the class walk);
> `src/libs/kt-model/CLAUDE.md`; `openspec show project-format --type spec` and `transport`. Commit per
> section; `scripts/target-prune.py` after each commit.

## 0. Notes first

- [x] 0.1 proposal, design (D1–D7, D1a, class walk D6), deltas `project-format`, `transport`;
      `openspec validate shared-events --strict`
- [x] 0.2 the owner's answer: Q31 b, build now (kkt-26.11.20 line 27)

## 1. The before numbers

- [ ] 1.1 `cargo build --release -p kt-cli`; `kt-cli showcase` ×1 and ×10 (the 1357- and 13 357-event projects
      of found-defects 192/641); `kt-cli edit-cost` on both; paste the drag line, the handover and the
      incremental build times into design D5 under "before"

## 2. The cell and the container (kt-model, no behaviour change yet)

- [ ] 2.1 `src/libs/kt-model/src/shared.rs`: `Shared<T>` (D1: `Arc<Cell { value, fp: OnceLock<u64> }>`, Deref,
      `edit` via `Arc::make_mut` clearing `fp`, `edit_if`, pointer-first `PartialEq`, transparent Debug and
      serde, `fp()`); move `hash_debug` from `kt-engine/src/incremental.rs:198` into kt-model (the same xxh3
      over the same text) and re-export it for kt-engine's four callers (`incremental.rs`, `audio.rs`,
      `video.rs` ×2)
- [ ] 2.2 `Events<T>` with the table of D1a: Index/IndexMut, `iter` → `&T`, `iter_mut` through `edit`,
      `push/insert/extend` of `T`, `len/is_empty/first/last/get/remove/retain/sort_by_key/truncate/drain`,
      `edit(i)`, `edit_where`, `edit_if`, `From<Vec<T>>`, `FromIterator<T>`, `IntoIterator` for `&Events<T>`
      and by value (values out by `Arc::unwrap_or_clone`), `Default`, `Clone`, `Debug` = `debug_list` of
      values, serde as a sequence of `T`; unit tests in `shared.rs` for the Debug text and serde bytes equal
      to `Vec<T>`'s
- [ ] 2.3 `kt_time::cost`: `Cost::EventClone` (counted in `Shared::edit` when the cell was shared) and
      `Cost::EventHash` (counted when `fp()` computes)

## 3. The fields (kt-model, then the rest of the workspace)

- [ ] 3.1 `Project.{clips, transitions, morphers, cameras, foreign}` (`project.rs:1357…1439`) and
      `Sequence.{clips, transitions, morphers, cameras}` (`project.rs:1079…1090`) become `Events<…>`;
      `ResolvedReel` (`published.rs:197`): `clips: Events<ClipEvent>`, the two `Cow<'a, [T]>` become
      `Cow<'a, Events<T>>`; `effective_clips` (`project.rs:6866`) returns `Events<ClipEvent>`
- [ ] 3.2 compile and fix: the 19 `&[ClipEvent]` signatures take `&Events<ClipEvent>` (or
      `impl IntoIterator<Item = &ClipEvent>`); `clips: vec![…]` literals take `.into()`; an `Option<&Shared<_>>`
      that must be `Option<&T>` maps with `.map(|c| &**c)`. Do not change any reader's logic
- [ ] 3.3 every `iter_mut()` on these fields in kt-model's command paths (9 in `src/`; 61 workspace-wide,
      most in tests) becomes `edit(i)` / `edit_where` / `edit_if`, so only written events copy; the lane-order
      and renumbering writers use `edit_if` (D2)
- [ ] 3.4 green before going on: `cargo test -p kt-model -p kt-otio -p kt-engine`, t148, t811, t822, t2004 and
      the goldens (`GOLDEN_*`) unchanged; clippy; commit "shared-events 3: event vectors are shared cells"

## 4. The model's copies become pointer copies (D2)

- [ ] 4.1 `model.rs`: `simulate` (1536), the equality check in `apply_batch` (1494), `push_undo` (1634),
      `rebuild_bench` (1312), `reel_document` (1407), `doc_arc` (1464), `apply_coalesced` (1613): each now
      clones `Project` (pointer copies); check nothing in them deep-copies a vector of events by hand
- [ ] 4.2 kt-app: the Viewer's cut edits one event through `edit()`; the windows that clone `document()`
      (render dialog, settings, fx editors) need no change (D6)
- [ ] 4.3 t3367, t3368, t3369, t3370, t3375 in `src/libs/kt-model/tests/shared_events.rs` (red first: t3367
      and t3375 fail on section 3's state if any copy is still deep); `cargo test -p kt-model -p kt-app`
      (kt-app: `layout_tests`, `reach_tests` untouched); clippy; commit "shared-events 4: an edit copies the
      events it writes"

## 5. Builders and the resolve (D3, D4)

- [ ] 5.1 `incremental.rs` `BuildMemo` keys and the whole-project hashes (`scene_base` parts, generator salt
      in `audio.rs`/`video.rs`) read `c.fp()` where they called `hash_debug(c)`; the same number
- [ ] 5.2 `resolve_reel` (`project.rs:7024`) and `effective_clips`: an event no layer, link or override
      reaches is pushed as the same cell (`Events::push_shared`); the reach test is the existing index
- [ ] 5.3 t3371, t3372 (`kt-engine/tests/build_cost.rs`), t3373 (`kt-engine/tests/shared_build.rs`),
      t3374 (`kt-cli/tests/cli.rs`: `edit-cost` prints `EventClone` and `EventHash` per edit);
      `cargo test -p kt-engine -p kt-cli`; clippy; commit "shared-events 5: builds hash the events that changed"

## 6. After

- [ ] 6.1 the "after" numbers of 1.1 into design D5 (release); found-defects 192 and 641 point here
- [ ] 6.2 the idle check on the heavy showcase (pause, the mouse over a static place, an effects window
      open): unchanged at rest; a drag on the ×10 showcase measured in the trace (`kt-cli trace during`)
- [ ] 6.3 `src/libs/kt-model/CLAUDE.md`: "an event is written only through `edit()`; `iter_mut` on an event
      vector copies every shared cell"; kt-engine `CLAUDE.md`: event keys are `fp()`; the report's
      "Найдено, не исправлено"; target-prune
