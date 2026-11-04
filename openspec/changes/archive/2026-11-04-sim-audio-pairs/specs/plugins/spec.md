## ADDED Requirements

### Requirement: A simulated device's two halves are one passport
A video kind MAY declare the audio kind that records the same device's sound (`Decl.pair`). The
two SHALL have the same passport and row forms; the registry SHALL refuse, by name and in either
registration order, a pair whose passports differ, a pair that is not an audio effect, an audio kind
that declares a pair, a pair declared by a kind that is not a video effect, and two video kinds naming
one audio kind. `Registry::counterpart` SHALL answer both directions, and the built-in assembler SHALL
refuse a declared pair whose audio half is not registered.
Why: the owner, kkt-26.11.03b D1 — a simulated device that records picture and sound simulates both,
and the two halves are one device; a row only one half had would be a parameter only one half of the
device obeys.

#### Scenario: The registry holds a pair to one passport
- **WHEN** a video kind naming `tape_sound` is registered, then `tape_sound`; then a second video kind
  naming it, an audio kind declaring a pair, a pair with a different default (both orders), a pair
  naming a video kind, an audio pair declaring a pair
- **THEN** the first two register and name each other as counterparts, `unpaired` empties when the
  sound arrives; every other one is refused naming its reason
- **TEST** t1460 (`src/libs/kt-plugin-api/tests/pairs.rs`)

### Requirement: An audio state knows where its clip sits on the timeline
The host SHALL tell every audio state the timeline frame of its clip's frame 0 and the project frame
rate (`AudioState::place`), when the state is built and whenever the clip is synced, without a reset
(a moved clip keeps its state). An audio half of a simulated device SHALL read the device state at the
TIMELINE position its picture reads, derived from the rows and that position alone — no state is
shared with the picture's executors.
Why: the picture of a tape or a stream reads the absolute timeline tick; an audio state saw only its
clip-local frame, so the sound could not lose the field the picture loses.

#### Scenario: A clogged head's field on an event off the field grid
- **WHEN** an audio event starting 3.01 s into the timeline (half a PAL field off the tape's grid)
  carries the VHS deck's sound with head A clogged, rendered by the engine; then the slot switched off;
  then rendered again
- **THEN** the sound is lost while head A reads on the TIMELINE's tape grid (eight times head B's
  error); switched off it gives the dry bytes; the two renders are byte-equal
- **TEST** t1467 (`src/libs/kt-engine/tests/sim_pairs.rs`)
