# Proposal: edit-without-rebuild

## Why

`docs/engine-audit.md` M1 (row 3 of its order table): an edit, a region change or one step of a
slider drag pays for rebuilding the WHOLE engine, several times, partly on the UI thread.
- The UI thread clones the document and builds a `VideoRenderer` per edit (row 150).
- The `audio-build` thread builds an `AudioRenderer`, prefetch builds its own audio and video pair
  (row 110), and the probe builds another on the next meter reading (row 111). The first probe and a
  region change build on the UI thread (row 117).
- Each build resolves each reel about five times (row 155).

Measured on the relinked copy of the owner's project (1991 effective events, release,
`kt-cli probe`): a rebuild of the unchanged project costs `effective_clips` 7 ms, audio 10.5 ms and
video 30 ms. One edit pays 2 audio + 2–3 video builds, 30+ ms of them on the UI thread.

The owner's go: registry `docs/ktprompts/kkt-26.11.09-index.md` I9 ("запускай edit-without-rebuild
и kernel-specialization").

## What Changes

- **One build per project version, shared.** A player's `engine-build` thread builds the audio and
  the video renderer of a version once, in one scope (each reel resolved once). It publishes them as
  `Arc`s that the audio render thread, prefetch, the probe (meters, the loop's pinned head) and the
  preview composer all use. No consumer builds its own.
- **Nothing on the UI thread builds.** `Player::set_project` takes an `Arc<Project>`: the app hands
  the model's shared document snapshot (`Model::doc_arc`), one snapshot per state, not a clone per
  edit. A frame is composed only by the renderer of the state it was asked for: the compose thread
  waits for that build, never the UI.
- **A region change is not an edit.** It moves the pinned head with the renderer already built.
- **An edit costs what it touched.** The builder keeps the per-event derived state of the last build
  (the audio event, the video layer before transitions), keyed by a hash of everything it derives from
  (the event's own fingerprint plus its reel context). A rebuild derives again only the events whose
  key changed. An unchanged event is copied, never recomputed. The memo is swept to the last build's
  events.
- Walked as a class: every consumer that builds or clones engine state (render, prefetch, probe,
  preview, export, reel previews, generator sources, kt-cli) and every UI path that triggers a build.
  The list is in `design.md`.

## Capabilities

- `audio-render`: one shared build per version. An incremental build renders the bytes of a fresh one.
- `transport`: zero builds on the UI thread; a region change builds nothing; the composer composes only
  with the renderer of its state.
- `video-plan`: the shared video renderer; an incremental build plans what a fresh one plans.
- `compositions`: a build resolves each reel instance once.

## Impact

`kt-engine` (player, a new `builds` module, both builders, the compose queue), `kt-time` (cost rows),
`kt-app` (the handover of the document, the live fader, the bars' key), `kt-cli` (`edit-cost`).
Tests t2000–t2019. No change to the `.kt` format, the plugin API or a test's expected bytes.
