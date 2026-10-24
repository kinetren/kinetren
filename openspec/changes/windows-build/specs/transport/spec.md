# Spec Delta

## ADDED Requirements

### Requirement: The audio output edge converts to the device's rate and channel layout

The engine's output stays f32 48 kHz/2ch (invariant 1). When the output device cannot be
opened at 48 kHz stereo (a WASAPI shared-mode device whose mix format is 44.1 kHz, or
5.1/7.1), the conversion SHALL happen only at the output edge. Rate: a band-limited
resampler whose state and buffers are allocated before the stream starts; no allocation
and no lock in the callback (invariant 6). Channels: L into front-left, R into front-right,
silence in every other channel; a mono device gets (L+R)/2. Playhead accounting SHALL stay
in engine frames: the playhead advances by the 48 kHz frames the edge consumed, not by
device frames. A 48 kHz stereo device MUST take the direct path with no conversion,
byte for byte as today.
Why: the fallback repeated or skipped samples positionally, so 44.1 kHz output aliased
audibly. Multichannel devices got the right channel copied into centre, LFE and every
surround. Both setups are rare on PipeWire and common on Windows, where shared mode
reports only the mix format.

#### Scenario: 44.1 kHz and 6-channel devices
- **WHEN** the edge converts 1 s of a 1 kHz stereo sine (L) and silence (R) for a 44.1 kHz stereo device, a 48 kHz 6-channel device and a 48 kHz stereo device, filling in callback-sized chunks of 441, 512 and 1024 frames under an allocation-counting allocator
- **THEN** 44 100 device frames consume between 48 000 and 48 000 + one resampler chunk + the kernel's two priming frames; the spectrum of the 44.1 kHz output peaks at 1 kHz and every other component is at least 60 dB down; the 6-channel output has the sine in channel 0 and exact zeros in channels 1–5; the stereo output is the kernel's output as before the edge, bit for bit; zero allocations happen during the fills (asserted when the test has its process to itself, under nextest)
- **TEST** t857 (`src/apps/kt-app/src/audio_out.rs`)
