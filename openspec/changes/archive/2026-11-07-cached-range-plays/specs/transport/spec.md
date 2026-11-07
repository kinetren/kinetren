## ADDED Requirements

### Requirement: An edit during playback does not stop the sound
While the transport plays, a model edit SHALL rebuild the audio renderer on a builder thread; the
render thread SHALL keep filling the ring with the renderer it has until the new one arrives, then
carry the FX state over (`sync_fx`, invariant 3). On a pause the rebuild MAY run on the render
thread. A start, the playback look-ahead and the background prefetch SHALL NOT order sources for the
windows the final-mix store holds.
Why: on the owner's project `AudioRenderer::new` takes 412 ms against a 341 ms ring; every edit
during playback stopped the sound (his load report: 12–16 bursts a second instead of 47 at those
moments, kkt-26.11.07b). A covered range must start and play with no decode.

#### Scenario: Edits while a covered loop plays
- **WHEN** a 6000-event project loops a short range for 4 s and four edits arrive
- **THEN** there is no callback underrun and no render underrun, and most bursts come from the kept mix
- **TEST** t1935 (`src/libs/kt-engine/tests/mix_player.rs`)
