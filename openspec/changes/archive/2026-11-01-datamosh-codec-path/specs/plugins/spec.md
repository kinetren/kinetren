# Spec Delta

## ADDED Requirements

### Requirement: A stream kind is a source stage with a declared history, not video state
A video plugin MAY declare the planner class `Stream` and provide a `StreamKernel`. Such a kernel
SHALL declare no state, tail or look-ahead; its output at frame N SHALL be a pure function of its
rows — read per position WITH their curves through the layout (`rows_at`) — and of the source frames
of the HISTORY it names for N through the stream's layout. The layout SHALL also carry a REGION OF
INTEREST per frame (`region_ident`, `with_region`: a coverage plane at the stream's size with an
identity), the bridge for masks and object detectors; no region means the whole frame. The host
SHALL own the memo the kernel keeps results in (`StreamMemo`, bounded, keyed by what determines
the result) and SHALL hand it to the kernel; the bytes MUST NOT depend on what the memo holds.
The registry SHALL refuse: a `Stream` class without a stream kernel, a stream kernel on another
class, and a stream plugin offering any GPU side. A built-in stream plugin is exempt from the GPU
default (the codec is a serial loop; its output reaches both executors as a pre-FX frame).

Why: F-FX-007 named the temporal family's way through the purity rule — read the PAST SOURCE,
do not keep state; `datamosh-codec-path` design D2–D4.

#### Scenario: The registry refuses a malformed stream plugin
- **WHEN** a plugin declares the `Stream` class without a stream kernel, or with a WGSL side, or a point-wise plugin offers a stream kernel
- **THEN** registration is refused naming the declaration, and a well-formed stream plugin registers
- **TEST** t1240 (`src/libs/kt-plugin-api/tests/registry.rs`)

#### Scenario: The datamosh keeps its file text
- **WHEN** a slot `datamosh` with default rows and with every row moved is written and read again
- **THEN** the text is the frozen one, the rows at their defaults are omitted where declared, and the golden table holds its row
- **TEST** t1247 (`src/libs/kt-model/tests/plugin_format.rs`), t328 (`src/libs/kt-fx/tests/video_fx.rs`)
