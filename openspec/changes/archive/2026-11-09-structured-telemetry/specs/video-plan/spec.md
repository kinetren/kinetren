## ADDED Requirements

### Requirement: A kept moment is reached without the work that made it
The preview SHALL ask the display executor for a moment's kept composite BEFORE it asks anything of the
cache, the decoders or a stream stage: the key comes from a probe plan in which every stream stage gives
the key its kernel names for the frame (`StreamKernel::key`, computed from the rows and the source
addresses, nothing rendered, the memo not read), and a held key is shown by the display pass alone
(`present_kept`). The kept-moment recheck SHALL use the same probe plan. A cache entry evicted and
decoded again from an unchanged source SHALL take back its content serial (invalidating the source
forgets it). A complete composite of the slow-chain device SHALL be kept on the display's executor, and
a moment shown from it does not re-time the route. The look-ahead SHALL ask neither decodes nor stream
history for a moment the bars show as kept. Applies to every effect with history and every way of
reaching a moment: pause, slow rate, loop, scrub.
Why: the owner, kkt-26.11.09 I2/I5/I8: "если хоть сто раз пройтись по небольшому куску - он все
равно что-то loading делает". The trace of the heavy loop (Camera ISP → Datamosh → VHS Tape) showed
every kept composite costing a full plan with its Datamosh stage (15 ms per composite against 0.3 ms of
replay), laps composed again for 16 s because the stage showed held frames, and — where the memo had
dropped the history — a kept frame planned short and shown as "Loading".

#### Scenario: A kept moment needs no history
- **WHEN** a paused frame under a cut mosh is composed whole, then the stream memo is cleared and both sources' cache entries are invalidated, and the frame is presented again
- **THEN** it is complete at once from the kept composite: no stream stage ran for it, no decode was asked, it waited for nothing, and the bars show the moment as kept
- **TEST** t2145 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: The look-ahead skips kept moments
- **WHEN** the look-ahead is asked with ten moments kept
- **THEN** none of their keys is asked and every other key is asked as before
- **TEST** t2150 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: The slow route keeps its composites
- **WHEN** a moment of a chain slower than `SLOW_CHAIN` is composed on the slow device as bytes, then asked again, then a neighbouring moment is asked
- **THEN** the second ask is shown on the display from the kept composite (one replay), and the neighbour still goes to the slow device
- **TEST** t2151 (`src/libs/kt-engine/tests/decode_storm.rs`)

#### Scenario: A re-decoded entry keeps its serial
- **WHEN** an entry is evicted and inserted again, then overwritten, then its source is invalidated and it is inserted again
- **THEN** the re-insert has the serial it had, the overwrite and the insert after the invalidation have new ones
- **TEST** t2152 (`src/libs/kt-cache/src/lib.rs`)

#### Scenario: A short loop under a heavy chain plays from what is kept
- **PATH** open our showcase with the owner's chain (Camera ISP → Datamosh gop 66, 720 lines, search 64 → VHS Tape) on the longest footage event and a 0.7 s loop over it → the paused frame is shown → press Space → the first laps compose, then the strips over the loop turn bright → no frame is composed, no burst runs the effects, nothing is decoded → press Space → the paused frame inside the loop is there, no "Loading"
- **TEST** t2153 live scenario `src/scripts/live/heavy_loop.py`
