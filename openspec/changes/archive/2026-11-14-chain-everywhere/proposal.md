# Proposal: chain-everywhere

## Why

The owner, 2026-11-14 (`kkt-26.11.14` п.2, registry row M2): "нет, НИГДЕ НЕ ДОПУСТИМО чтобы эффекты
накладывались мимо цепочки, и это ко всему применимо а не только к datamosh". Found-defects 460 is his
example: an effects-picker cell for a chain holding Datamosh laid each candidate over the un-moshed
frame — the picker ran "the event's chain + candidate" on a still source thumbnail through
`chain_plan`, where a stream slot without a history passes the base through, and neither the leading
Geometry node, nor the track's stages, nor a reel around the event ran. The mechanism is wider than
Datamosh: every path that builds an effect's picture or sound outside the one composition path
(`frame_plan`) or the audio render chain can lay the effect past its place. And п.1 (M1) holds inside
this area: a cell that cannot show the effect in its place shows Loading, never a stand-in.

## What Changes

- **Picker, gallery and preset cells render the real frame.** A cell of the effects window is the
  composite of the project the player plays, at the cursor's moment, through `frame_plan` on the
  offline device, with the candidate appended to the anchor event's chain (where the click puts it)
  and the anchor's track soloed: the event's placement, the slots in front, a stream stage's history,
  the track's stages and a reel instance's chain all run as the preview will run them after the click.
  The service opens its own short-lived sources (invariant 5's offline path) only after the misclick
  guard; until a cell's composite is complete it shows Loading and draws nothing else. The built-in
  sample is no longer a stand-in for an event's frame; it stays only where no chain exists.
- **Morpher cells** render the real frame of a target the morpher acts on at the cursor (inside its
  span), with the candidate appended to the patch; a morpher with no target there shows the gallery
  as names (no picture to be in).
- **The slots in front of a stream stage keep their masks and their camera** (found-defects 541): the
  front run gets the slots' mask ops (remapped to the front sub-chain) and the camera the engine
  resolves for them, so a masked slot in front of Datamosh acts only inside its mask and a Pan3d there
  is not skipped.
- **A stream's second input is read through its own chain** (found-defects 540): the frames of the
  event on the track above/below run that event's picture slots (with masks, curves and camera) before
  the kernel reads them; the stream's addresses name that chain, so its edit re-walks the history.
- Class walk of every place an effect's picture or sound is made outside its chain, listed in
  `design.md`; what stays open becomes found-defects rows 580+.

## Capabilities

### Modified Capabilities
- `video-fx`: the effect thumbnail / thumbnail service / picker gallery requirements (a cell is the real
  frame with the candidate in place, Loading until complete); a stream stage's front keeps masks and
  camera; the second input runs its chain.

## Impact

kt-engine (`video_stream.rs`, `video.rs` `ClipMaskPlan`/`link_stages`, `video_plan.rs`
`chain_plan_with`), kt-app (`fx_thumbs.rs`, `fx_editor.rs`, `morpher_editor.rs`, `fx_registry.rs`
`preview_cell`). No format change; OTIO unaffected.
