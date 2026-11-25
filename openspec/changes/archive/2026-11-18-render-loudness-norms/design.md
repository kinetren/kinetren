# Design

## D1. A channel's level over the summed gate's blocks

`ChannelLoudness.lufs` is the channel's unweighted mean square over the blocks both BS.1770 gates keep
for the SUMMED measurement (`gated_blocks`, the same selection as `gated_lufs`), as LUFS. Gating each
channel by itself would compare the channels over different moments; over the same blocks a balanced
pair reads equal, 3 LU under the sum, and a silent channel has no level (`None`).

## D2. The true-peak gate per channel

The interpolation ran only near samples within −3 dB of the largest sample of EITHER channel; a channel
20 dB quieter never reached it and its true peak read as silence. The gate (`sample_peak`,
`tp_countdown`) is per channel now. On a balanced mix the work is the same; on a lopsided one the quiet
channel is interpolated around its own peaks — what its peak needs. The summed `dbtp` is the channels'
maximum.

## D3. Norms, one place (`loudness_rows`)

Integrated: within 1 LU of −23 (EBU R 128) or −14 (streaming) is plain, between or below amber, louder
than −9 LUFS red (too loud for any delivery). True peak above −1 dBTP red. Range above 20 LU amber
(informative: broadcast speech and music are under it). Channels: 3 LU apart amber, 6 LU or one silent
red. The norms are printed next to each value; no target is chosen for the person (a target that also
normalizes is F-AUD-005's later stage).

## D4. Window height

The progress window's default height 520 was set by `render-window-v1-parity` (an agent's choice);
the four rows pushed the channels row under the fold, so the default is 600. The window stays resizable
and remembers its size.

## Class walk

The mechanism: a measurement shown as a bare number with no norm and no emphasis, so a bad value
passes unseen. Checked: the render result (fixed here); the bin card's loudness hint (`k::bin::LOUDNESS`,
"N LUFS, peak N dBTP") — a hover hint about a SOURCE, not a delivery: no norm applies to a raw file, left;
the render settings' size and time prediction (render-honest) — already says against what; the meters'
level (a live bar with zones and a peak strip — has its emphasis); the Diagnostics load numbers (have
their threshold warnings). Only the render's loudness lacked norms.
