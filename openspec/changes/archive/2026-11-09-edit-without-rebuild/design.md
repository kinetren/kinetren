# Design: edit-without-rebuild

## The mechanism

**Nothing in the structure said what an edit touched** (`docs/engine-audit.md` M1). A model edit
handed the player a whole project, and every consumer derived the whole engine from it on its own:
- the UI thread built a `VideoRenderer` and raised the generator sources (`gen_sources` resolves every
  reel and walks the instance tree);
- the `audio-build` thread built an `AudioRenderer`;
- prefetch built its own audio and video pair;
- the probe built another audio renderer on the next meter reading, the first one on the UI thread.

Measured before the change (release, `kt-cli edit-cost`, the relinked copy of the owner's project,
1977 events, machine load 10–15):

| | haski (1977 events) | showcase --heavy (1357) |
|---|---|---|
| one edit (a slider step), UI thread p50 / max | **87.8 / 150 ms** | 7.3 / 12.0 ms |
| one edit, process CPU | **197 ms** | 19 ms |
| renderer builds per edit | 5 | 5 |
| a 60 Hz drag (120 steps), UI p50 / max | 50 / 166 ms | 7.2 / 12.0 ms |
| a 60 Hz drag, process CPU | 244 % of a core, 570 builds | 96 %, 600 builds |
| the document clone alone | 0.14 ms | 0.29 ms |

The clone the audit suspected is cheap. Most of the UI thread's 88 ms was the generator sources
(each reel resolved, 7 ms for the root alone) and the video build (25–31 ms).

## Decisions

### D1. One build per version, published

`builds.rs`: a player owns `Builds` (the newest WANT: the project `Arc`, its version and a sequence
number; and the last published `Built`: the audio and the video renderer as `Arc`s, the project,
the build's cost counts) and an `engine-build` thread.
- `set_project` stores the want and wakes the thread. A source added or replaced asks the same
  version again with new facts (`Builds::refresh`).
- The thread takes the NEWEST want: the versions in between are never built. A 60 Hz drag of 120 steps
  built 124 renderers instead of 570 on haski.
- It publishes, then syncs the probe's FX states (only the changed events, `sync_fx_since`), pins the
  loop region's head with the new renderer, wakes the render thread and the UI.

Consumers:
- The **render thread** swaps to a published build exactly as it swapped to its own builder's result
  (`sync_fx_since`, `mix.revalidate`), so invariants 2 and 3 hold unchanged. A build asked on a pause
  still holds the next start.
- **Prefetch** reads the latest build. The build sequence joins its pass state, or a pause skipped
  the pass that takes a build landing after its edit (t20 turned red without it).
- The **probe** is the shared renderer plus the UI's own FX store, synced on the builder thread. A meter
  reading in the moment of that sync reads silence rather than waiting.
- The **composer** composes, and rechecks kept moments, with `Builds::wait_current`, the build of the
  newest want: the compose thread waits, the UI never does. Without it a composite of the old state
  would carry the new version and be shown as the edit's picture. The kept-moment recheck would also
  mark a moment "plays without work" by the old plan's key (t1936 turned red without it).
- The **UI's readers** (`video_grid`, `video_decode_qualities`, `nest_coverage`, `missing_layers`,
  `request_video`) read the last published build. The app's coverage-bar stamp gets
  `Player::built_seq`, or the bars would keep the old build's decode qualities under the new revision.

The opening builds once on the constructor's thread. It used to build a video renderer there, and the
render thread built its audio one before its first burst. Every later version builds off the UI thread.

Rejected: a build per consumer with a shared memo. It keeps the multiplicity, and the audio render
thread's swap would still wait on its own builder.

### D2. `AudioRenderer` shared across threads

Two render-time `Cell`s made it `!Sync`:
- `src_ends`, a lazily computed per-clip value, becomes a `OnceLock`. Whoever renders first computes
  it, and it is the same value.
- `loop_region` was set by `render_looped_with` for the length of the call and read by the stages and
  `render_fx_clip`. It is now a thread-local keyed by the renderer's address. Only the render that
  armed it, on its own thread, sees it. A nested reel's renderer and prefetch on another thread see
  `None`, exactly as when each had its own renderer.

Moving it into the `FxStateStore` was rejected: the sidechain key branches render through sub-stores,
which would have had to carry it too.

### D3. Generator sources on the builder thread

The builder raises the generator sources of a version BEFORE it reads the source facts for the video
build: a raised generator is a procedural fact of its layer. It does so under `MediaParts::gen_lock`,
from the NEWEST project. `Player::update_gen` (a value edit, law P) replaces the stored project in
place under the same lock, and the builder must not retune a source back to values that edit has just
replaced.

`MediaParts` holds what both threads need (workers, facts, cache, generator states). The player's
`add_media`/`replace_media` call it and then `Builds::refresh`. The builder's own raising does not
refresh (it is about to build).

### D4. One resolve per reel instance per build (row 155)

`revisions_scope` (opened by both builds, `gen_sources`, and the player's build around all three)
also opens a resolve memo: `incremental::resolve` keys a `ResolvedReel` by (project address, reel,
overrides hash) and shares it. Inside the scope the project is borrowed and cannot change, the same
argument as `REVISIONS`. The audio build, the video build, `nest_scales` and `gen_sources` resolved
every reel on their own: t2005 counts at least six resolves apart, two in one scope.

### D5. The per-event memo, keyed by everything the event derives from

`incremental::BuildMemo`, kept by the builder thread across builds:
- audio events: before transitions, run predecessors and the derived fingerprint;
- video layers: before transitions and stream linking.

The key is a hash of the model event's Debug text — the same text its fingerprint `fp` hashes — plus
every context value the builder passes in:
- audio: the source address (salt), the track level folded into `gain`, the reel instance;
- video: the source address, the source facts of that address (native size, procedural, material
  length, view), the conform rule and raster through `LeadCtx`, the instance, its frame rate and
  scale, the track's silence, the reel's cameras (one hash per reel), the project size and frame rate.

A hit is a CLONE of the stored event, never a recomputation, so a key that names every input makes the
memo invisible to the bytes (invariant 11). t2004 is the proof: after each of eight edits (a gain, a
curve key, a trim, an edit inside the reel, a removed transition, a muted track, an opacity, an undo)
the incremental build's full state text equals a fresh build's, and so do the sound at three places
and the scene hashes.

`with_memo` sweeps what the build did not use, so the memo holds one project's events.
`AudioClip.ctx` (`chain_context`) moved into the derivation: a pure function of the chain and curves,
computed in the same order.

The Debug text is hashed through a `fmt::Write` adapter (`incremental::hash_debug`). No `String` is
built, and the digest is the one `xxh3_64` gave the text, so `fp` values are unchanged and the kept mix
keeps its keys.

Rejected: a per-event revision kept by the model. Every command would have to bump the right events'
revisions, and a missed bump is a stale event with no test to say so; a content hash cannot miss.

### D6. The UI hands a pointer

`set_project(impl Into<Arc<Project>>)`. `push_project_to_player` hands `Model::doc_arc()` at the root:
the effects, camera and composition windows already keep that snapshot, so the player and they share
one copy per state (t2008 checks the pointer). Inside a reel it hands `Arc::new(reel_document())`.
`Player::project()` returns the `Arc`.

A structurally shared `Project` (an edit copying only what it touched) was not done. The clone is
0.14 ms on 1977 events, and it is now one per state.

## Tests that changed

- **t1875**: "one renderer per version for the probe" becomes "nothing for region changes and meters,
  and the new version built once (an audio and a video renderer)". A strengthening.
- **t1840**: "the edit reached the renderer at once" becomes "it reaches the renderer while the composite
  is still stuck". The UI's grid is the last published build. The test's point (a stuck composite holds
  nobody, the builder included) stands.
- **t1844**: holds the shared builder (`gate_builds`; `gate_probe_builds` kept as its alias). The scenario
  is unchanged.

## The class walked

Every consumer that builds or clones engine state, and every UI path that triggers a build:

| Place | Before | Now |
|---|---|---|
| audio render thread | its own `audio-build` thread per edit | swaps to the published build |
| prefetch | its own audio + video build per version | reads the published build; the build sequence is in its pass state |
| probe (meters, the loop head) | its own build, the first on the UI thread (rows 111, 117) | the shared renderer; FX states synced on the builder |
| preview composer and the kept-moment recheck | the UI thread's `VideoRenderer` (row 150) | the build of its state, waited for on the compose thread |
| UI readers (grid, decode qualities, reel coverage, missing layers, `request_video`) | the UI-built renderer | the last published build; the bars' stamp names it |
| `set_project` → `sync_generators` | `gen_sources` on the UI thread per edit | on the builder thread, in the build's scope |
| `add_media`/`replace_media` → `rebuild_video_renderer` | a video build on the UI thread | `Builds::refresh` |
| `set_loop_region` | the probe's build when its version was stale | nothing is built; the head is pinned with the published renderer |
| `update_gen` (a title keystroke) | `gen_sources` on the UI thread | unchanged: row 190 |
| app's `sync_gen_sources` passport fallback | a copy + `sync_generators` | unchanged (rare): row 190 |
| app `push_project_to_player` | `model.document()`, a copy per edit | `model.doc_arc()`, shared with the windows |
| the live level drag (`timeline.rs`) | a copy + `set_project`, a build on the UI thread | a copy (0.14 ms) + `set_project`; the build is off the UI thread |
| export (`export.rs`, `export_pipeline.rs`) | one audio + one video build per export, offline | unchanged; each builder resolves once now. An extra video build to ask `has_stream_stages`: row 191 |
| reel previews and posters (`reel_preview.rs`) | an offline build per release, on the service thread | unchanged (offline, content-keyed, never per edit) |
| thumbnails, analysis services (LUFS, sync, ducking) | no renderer builds | — |
| kt-cli (`probe`, `render`, `export`, `edit-cost`) | offline single builds | unchanged; `edit-cost` measures the player |
| the speed-profile segment lists per build (row 167) | every warped event in every build | the player's builds reuse unchanged events' derived state; a fresh offline build still lays them out once |

What still walks the whole project on an edit, off the UI thread: the root's resolve and the Debug hash
of every event (row 192). That is the audit's step 4, worth doing only if a 10× project misses the
budget.

## Measured after (release, same tool, machine load 28–43 from other sessions)

| | haski | showcase --heavy |
|---|---|---|
| one edit, UI thread p50 / max | **1.40 / 4.8 ms** | 1.61 / 6.6 ms |
| one edit, process CPU | **40.5 ms** | 14.5 ms |
| renderer builds per edit | 2 (one shared build) | 2 |
| events derived per edit's build | 1 | 1 |
| a 60 Hz drag, UI p50 / max | 1.16 / 4.2 ms | 1.69 / 14.2 ms |
| a 60 Hz drag, process CPU | 99 % of a core, 124 builds | 77 %, 220 builds |
| 20 region changes | 0 builds | 0 builds |
| an edited version's build (generator sources + audio + video in one scope) | fresh 31.6 ms, incremental **20.5 ms** (root resolve 7.6) | fresh 12.7, incremental 8.0 |

`kt-cli probe … --time` on haski: `VideoRenderer::new` 31.6 → 25.4 ms (the root resolved once,
not twice); `AudioRenderer::new` 10.4 ms unchanged; the warm render 0 % of real time as before.

Live (debug kt-app, llvmpipe):
- `edit_drag`: 21 edits while playing, 42 renderer builds, 0 callback underruns; a drag frame costs the UI
  thread 1.9 ms against 1.8 ms hovering.
- `idle` on the heavy showcase: a pause at 2 % of a core, the effects window open at 2 %, and a pointer
  over static places causes no engine work.
- `cached_range` and `audiofx` green.
