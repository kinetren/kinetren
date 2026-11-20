# Proposal

## Why

The owner, kkt-26.11.18b line 29 (registry U8), asked "VHS Tape, Datamosh, Camera ISP на видеодорожке без
звука — как быть?", answered "Дать звук отдельно" — option (a) of fx-graph's Q5: "the track's device
node gets a sound lane wired to the audio track you pick in its form". Today a device on an EVENT
puts its sound half on the linked audio event (sim-audio-pairs); the same device on a whole VIDEO
TRACK is silent, because a track has no linked audio. The root rule (CLAUDE.md): "A simulated device
that records picture AND sound simulates both … driven by the SAME device state".

## What Changes

- **A video track names its sound track** (`Track.sound: Option<TrackId>`, edited with the track's
  properties): one audio track of the same reel, the sound of at most one video track. When a video
  track gets its first paired device and has no sound track, the model links the audio track of the
  same number if it is free (where linked media puts the file's sound); the person changes or clears
  it in the track window, and a cleared link stays cleared.
- **The device's sound half is a real slot of the sound track's chain**, by the event rule
  (`mirror_halves`): presence, values shared both ways, each half's switch its own. Changing the
  link moves the halves; clearing it or removing the sound track takes them off.
- **The track stage plays it**: the sound half runs in the audio track's stage over the track's sum,
  seated at timeline frame 0 with the project's frame grid (`AudioState::place`), so it reads the
  device at the timeline moment the picture half reads — a dropout is seen and heard at once.
- **The track window shows the pair as one graph** (fx-graph D10's pair editor): the video track's
  chain on the picture lane, its sound track's chain on the sound lane, the device's halves in one
  column with their own switches; a "Device sound" row picks the track, shown only while the chain
  holds a device.
- **Format by content**: a project with a sound link is project format 6 and `metadata["kt"]` v17;
  other projects keep their version and bytes. A row in "What OTIO export loses".

## Capabilities

### Modified Capabilities

- `audio-fx`: a simulated device is one across a video track and its sound track.
- `project-format`: the track's sound link raises the file by content.
- `ui-reach`: the track window edits a video track and its sound track in one graph.

## Impact

- kt-model (`Track.sound`, `TrackProps.sound`, the pair rule for tracks, removal, validation, the file
  version), kt-fx (`TrackProcessor` seats its nodes), kt-otio (track envelope), kt-app (track window,
  i18n ×5).
- Tests reserved: t3230–t3249 (shared notes `fx-graph.md`).
- Feature note F-FX-016 (`docs/features/fx/simulate/track-device-sound.md`).
