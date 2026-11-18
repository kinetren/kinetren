## ADDED Requirements

### Requirement: A sharp datamosh carries the codec's decisions at the layer's size
The kind `datamosh` SHALL have a structural flag row `sharp`, off by default. With it on, the codec SHALL
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
the codec's decisions; the softness is only its picture's size (F-FX-015, design D1).

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
- **TEST** t2975 (`src/plugins/fx-video-codec/tests/sharp.rs`)

#### Scenario: A person sharpens a 144-line mosh in the editor
- **PATH** add two clips back to back → select the second → FX → Datamosh "Cut mosh" → set the encoded height to 144 → the preview shows the soft break → tick "Sharp" → the same break, with the clips' detail
- **WHEN** the live scenario runs on our fixtures
- **THEN** the sharp frame differs from the soft one and has more fine detail; the screenshots of each step are saved
- **TEST** live `datamosh_sharp.py` (`src/scripts/live/datamosh_sharp.py`, via `src/scripts/live-check.sh`) — t2976

### Requirement: A stream's input follows the layer's size
The host SHALL order a stream stage's frames, and run the slots in front of it, at the stream's INPUT size:
the layer's base size when the kernel carries its picture (`StreamKernel::carried`); otherwise the codec's
size (`lines` high, the layer's aspect, `stream::coded_size`), but the layer's base size when `lines` is
above the layer's height. The kernel SHALL resample an input of another size to its codec's size, by the
same integer rule on the device and on the processor, and with the input at the codec's size SHALL read it
as before. In the preview the layer's size is the preview quality's, in an export the project's.

Why: kkt-26.11.18b U2 — "тут бы конечно как-то учитывать настройки превью проекта чтобы поменьше работы
было"; row 734: Camera ISP in front of a 720-line Datamosh cost 59 ms a history position on his iGPU
against ~14 ms at the 360p preview (design D2).

#### Scenario: The input size follows the layer
- **WHEN** a Datamosh event's stream size is asked at a 640×360 preview and at 1920×1080 with `lines` 144, 720 and 1080, soft and sharp
- **THEN** soft: 256×144 at both; 640×360 and 1280×720 for 720 lines; 640×360 and 1920×1080 for 1080 lines; sharp: 640×360 and 1920×1080 at every `lines`
- **TEST** t2974 (`src/libs/kt-engine/src/video_stream.rs`)

#### Scenario: The resample is an identity at the codec's size
- **WHEN** a frame is resampled to its own size, shrunk 4× and grown 2×
- **THEN** the identity returns the bytes, a flat colour stays the colour, and a shrink of a 4×4-pixel checker gives its mean grey
- **TEST** t2973 (`src/plugins/fx-video-codec/tests/sharp.rs`)
