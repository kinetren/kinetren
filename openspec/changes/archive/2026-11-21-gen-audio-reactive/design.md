# Design

## D1. Option (a): a series per file, the generator listens to a track

The orchestrator chose (a) on 2026-11-20. A frame of a generator must stay a function of (address,
tick, quality) (invariant 4), and no generator may own a decoder (invariant 5). So the sound is
analysed ahead, once per file, and the generator reads a ready series. The mix after effects (b) would
tie the picture to the audio render thread and make a render render its sound first. It is the owner's
question Q55.

The generator names a TRACK (its "Hears track" row), not one event. A music bed cut into pieces drives
one visualizer across the cuts. Copy and paste need no id remapping, and the row is a passport number,
so the format does not move. This refines "names an audio event": the events it hears are those on that
track that overlap it in time.

## D2. The series

`kt_plugin_api::audio_series`: one 51-byte record per burst (1920 ticks). It holds the RMS of each
channel and the peak (dB over −60..0 as a byte), 16 bands (the loudest bin of a Hann-windowed
1024-point FFT of the mono sum between log edges from 40 Hz to 16 kHz, dB over −72..0) and 32 waveform
points (the sample of the largest magnitude in each 32-frame bucket, i8). It is about 8.6 MB per hour.
Each burst is analysed alone with the frozen FFT, so a series is the same bytes on every run and every
machine (invariant 11). The hold is a fold over the last 24 records, falling linearly: a frame is a
function of its time, so seek, preview and render agree.

## D3. The store seam

`kt_media::listen::SeriesStore` (`get`/`put` by file). The sidecar implements it with an additive table
`audio_series(media_id, version, data)`, and the purge of a changed file clears it. The caches' move into
a project subfolder (kkt-26.11.20 A14, spec-engine's `project-cache-folder`) moves it by implementing the
trait; nothing else names the table.

## D4. The feed is part of the state

`GenState.feed`: the pieces of series in the generator's material time, compared by the series' hash.
`gen_sources` builds it from the reel's clips and the registry (`kt_engine::listen`, by the project's
media path), so the player, export and kt-cli build it with one code. A value edit keeps the feed
(`update_gen` copies it), and a change of the heard track goes to the builder. The video builder hashes
the same feed into the event memo's key and into the generator's `src_rev`, so the disk prerender keys
move with it. A kind that hears nothing keeps its former `src_rev` byte for byte.

## D5. Who computes

The app has one background service (`listen.rs`, lane class `Background`). After an edit and at an
opening it asks the files the listening generators hear, once per file per project. A new series sets
`fresh`, and the next frame resyncs the player's generators. Nothing runs at rest or on a pointer move.
A render — the app's render job and kt-cli's loader — calls `listen::ensure` before raising any
source, so its bytes never depend on whether the background job had finished.

## D6. Drawing

Each kind is a compute shader over one storage buffer of the frame's 100 audio values
(`shader_frame_audio`: bindings 0 the uniform, 1 the values, 2 the frame), named for the prewarm
(`Kind::with_listen`; t2123 runs the family). A kind drawn without a host feed (a bin card, a
thumbnail) draws silence: a flat line, empty bars, rings at rest.

## D7. Not followed (recorded)

- A warped or reversed audio event is heard at its plain material time; a reversed one is skipped.
- Overlapping events on the heard track: the earlier one is heard.
- The registry is process-wide by media path and cleared when another project opens; it holds at
  most 64 series.

## D8. The class walk

The mechanism: a generator's picture now depends on something outside its own values and time. Where
else a source's picture comes from outside its event was checked:
- the preview, export, kt-cli and the reel strips all raise generator sources through `gen_sources`, so
  they share the feed;
- the bin card, the kind gallery's cells and the thumbnails draw a kind without a feed — silence, not
  a stale sound;
- the render-cache's durable key reads `src_rev`, so it carries the feed's key;
- the kept composites move because a retune bumps the content version and the source's serials;
- the reel sub-composite memo: a listening generator inside a reel hears that reel's tracks, and its
  retune goes through the same `update_gen_at` with the reel chain.
