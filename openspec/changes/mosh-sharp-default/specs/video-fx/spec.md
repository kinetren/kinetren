## MODIFIED Requirements

### Requirement: A sharp datamosh carries the codec's decisions at the layer's size
The kind `datamosh` SHALL have a structural flag row `sharp`: ON for a Datamosh added from now on
(`Decl::fresh`, every way a new instance is made), OFF where a saved slot lacks the row — its passport
default — so a project saved before keeps its look. With it on, the codec SHALL
still encode at `lines` and decide there the stream's schedule and breakages, its block grid, its motion
vectors and which blocks are coded; a carrier decoder SHALL execute the same dependency chain at the
stream's input size instead of the broken decoder at `lines`: an intra or refresh node is the source frame;
a hold node the reference; an inter node predicts every carrier pixel from the reference by the applied
vector of the codec block covering it, scaled by the ratio of the visible sizes, and, where that block's
residual is not empty in the plane, adds `gain ×` the source's own difference (the frame minus the previous
frame moved by the packet's own vector, scaled likewise). The carrier SHALL be computed by the batched walk
on the device with the bytes of the per-position reference. The paired sound SHALL not read the row.

Why: kkt-26.11.18b U1 — "на 144p получаются красивые изломы но они мыльные … не пиксилизацию включить а
чтобы именно расчет был такой хитрый чтобы оно в разрешение проекта превращалось если я хочу". The look is
the codec's decisions; the softness is only its picture's size (F-FX-015, design D1). On for new instances: kkt-26.11.18b U7
("«Резко» по умолчанию для новых Datamosh? — Вкл. для новых").

#### Scenario: An intact sharp stream is the source
- **WHEN** a moving picture goes through the codec at 144 lines with `sharp` on and no breakage, the input at 4× the codec's size
- **THEN** every frame of the stream equals the source frame at the input size (converted to the codec's planes and back)
- **TEST** t2971 (`src/plugins/fx-video-codec/tests/sharp.rs`)

#### Scenario: The sharp break is the soft break in detail
- **WHEN** a cut mosh (shot A still, shot B moving) is rendered at 144 lines soft and sharp with the input at 4× the codec's size
- **THEN** the sharp frames after the cut keep A's texture moved by B's motion — the sharp frame shrunk to the codec's size is closer to the soft frame than to B's source — and the sharp frame carries more fine detail than the soft frame enlarged
- **TEST** t2972 (`src/plugins/fx-video-codec/tests/sharp.rs`)

#### Scenario: The batched walk carries the reference's bytes
- **WHEN** frames inside a cut mosh with bloom, a vector edit and macroblock loss are rendered with `sharp` on by the batched walk and by the per-position reference, and soft with the input above the codec's size
- **THEN** the bytes and keys are identical
- **TEST** t2970 (`src/plugins/fx-video-codec/tests/sharp.rs`)

#### Scenario: The stream's sound does not read the carrier
- **WHEN** the stream's sound is rendered over a mosh with `sharp` off and on
- **THEN** the samples are identical
- **TEST** t2975 (`src/plugins/fx-video-codec/tests/sound.rs`)

#### Scenario: A person sharpens a 144-line mosh in the editor
- **PATH** add two clips back to back → select the second → FX → Datamosh "Cut mosh" → set the encoded height to 144 → the preview shows the soft break → tick "Sharp" → the same break, with the clips' detail
- **WHEN** the live scenario runs on our fixtures
- **THEN** the sharp frame differs from the soft one and has more fine detail; the screenshots of each step are saved
- **TEST** live `datamosh_sharp.py` (`src/scripts/live/datamosh_sharp.py`, via `src/scripts/live-check.sh`) — t2976

#### Scenario: A new Datamosh is sharp, a saved one keeps its look
- **WHEN** a slot `{"kind":"datamosh","lines":"144","before":12,"mosh":1.0}` saved before is read, and a new Datamosh (and its sound half) is made
- **THEN** the saved slot reads `sharp` off and writes back the same text; the new one is sharp and its file names `"sharp":true`
- **TEST** t2977 (`src/libs/kt-model/tests/plugin_format.rs`)
