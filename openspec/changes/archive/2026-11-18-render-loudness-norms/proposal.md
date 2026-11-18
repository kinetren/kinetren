# Proposal

## Why

The owner (`docs/ktprompts/v26.10.txt` line 56, registry T14): "в окне рендера нет акцента у loudless и
второй строки to если я например прорендерю отрывок с сильным неприятным gain на одном канале или
например на тишине у другого или всё вместе..., на цифры вообще внимание не обратишь такие. а еще не
понятно что является нормой у каждого параметра.. ну и оформить это получше надо". The render result
prints one grey line "Loudness: … LUFS, true peak … dBTP, range … LU" and "to −23 LUFS". A render with
the right channel silent measures −23.0 LUFS — exactly the R 128 target — because BS.1770 sums the
channels' energies: the number that should alarm reads as perfect (feature note F-EXP-009).

## What Changes

- `kt_fx::audio::loudness::Loudness` gains `channels: [ChannelLoudness; 2]` — each channel's level
  over the blocks the summed gates keep, and its own true peak; the true-peak gate becomes per channel,
  or a quiet channel's peak would never be computed. The audio is not touched; the summed values are
  the same (t403–t407).
- The render result's loudness becomes a table: Integrated (norm −23 EBU R 128 · −14 streaming), True
  peak (≤ −1 dBTP), Range (≤ 20 LU), Channels L / R (within 3 LU of each other). A value out of its norm
  is amber; a peak over the ceiling, a silent or 6 LU-apart channel, or louder than −9 LUFS is in the
  error colour. "Reference only — nothing is applied to the file" stays under it.
- The progress window opens 600 points tall (was 520) so the table's last row is not under the fold.

## Capabilities

- `export`: ADDED requirement "The render's loudness stands against its norms, per channel".

## Impact

`src/libs/kt-fx/src/audio/loudness.rs`, `src/apps/kt-app/src/render_dialog.rs`, five catalogs (two keys
replaced by fifteen). Tests t3106 (kt-fx), t3107 (kt-app); live scenario `render_loudness`.
