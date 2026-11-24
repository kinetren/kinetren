# Proposal

## Why

The owner's `v26.10` G5 names four generator groups. Three are done (technical, backgrounds, shapes).
The last is audio-reactive: a waveform, a spectrum and level rings. A generator drew from its values
and time only and could hear nothing. The orchestrator chose option (a) (2026-11-20): an analysis
series per file, computed in the background and keyed by the file's fingerprint. The generator
listens through it, and the series' hash joins the frame key.

## What Changes

- `kt_plugin_api::audio_series`: the series type. Per audio burst it holds the RMS of each channel,
  the peak, 16 log bands and 32 waveform points, one byte each. It also has `frame_values`, the audio
  values one frame reads, with a held (falling) level and bands.
- `kt-media` `listen.rs`: the analysis from a short-lived source of its own, and the store trait
  `SeriesStore`. The sidecar implements it today; the store moves with the caches later (A14).
- `kt-model` `GenState.feed`: the listened pieces of series in the generator's material time.
- `kt-engine` `listen`: a process registry of series by media path. `gen_sources` builds each
  listening generator's feed from the track it listens to, and the video builder hashes the feed
  into the generator's `src_rev` and the event memo.
- A plugin family `generators-reactive` with three kinds: `waveform`, `spectrum` and `levelrings`,
  drawn on the compute device from a storage buffer of the frame's audio values. Each has a
  "Track" row: the audio track it listens to.
- `kt-app`: a background job on the `background` lane computes the series of every file the
  listening generators hear, puts them in the registry and resyncs the player. The render job
  computes a missing one before it renders. `kt-cli` does the same before an export.

## Capabilities

- `generators`: ADDED requirement "An audio-reactive generator listens to a track's files".

## Impact

New crate `src/plugins/generators-reactive`. kt-plugin-api, kt-model (`GenState`), kt-media,
kt-engine, kt-app, kt-cli. No project format change: the kinds are plugin rows.
