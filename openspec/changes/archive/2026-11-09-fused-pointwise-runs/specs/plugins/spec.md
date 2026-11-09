## ADDED Requirements

### Requirement: A point-wise kernel declares that it fuses, and the registry checks the text
`kt_plugin_api::fuses_in_runs(plugin)` SHALL be the one answer to "does this stage fuse into a
point-wise run": a video plugin of class `PointWise` whose GPU side is a `tap_<id>` WGSL and that has
no barrier kernel (a point-wise plugin whose barrier position is a kernel of its own runs that kernel
there, and ends a run). The planner and the GPU executor SHALL ask it and nothing else. `Registry::register`
SHALL refuse a point-wise plugin whose WGSL does not define `fn tap_<id>(`, or that reads a texture
(`textureLoad`, `textureSample`, `textureDimensions`) or the host's uniform `P`: the tap is a pure
function of its value, its numbers and its coordinate, so the host can paste it into any pass.
Why: fused-pointwise-runs — a run's pass is generated from the stages' tap functions; a tap that
reads a neighbourhood or the host's bindings would compile in its own pass and break in a run.

#### Scenario: The registry refuses a tap that is not a pure point function
- **WHEN** point-wise plugins register with a WGSL lacking its `tap_<id>`, with a `textureLoad`, and with a read of `P`
- **THEN** each is refused with the reason; every built-in point-wise plugin registers, and `fuses_in_runs` is true for posterize, color grade, duotone and vignette and false for noise, greenscreen, every barrier and geometry plugin and every audio kind
- **TEST** t2194 (`src/libs/kt-plugin-api/tests/registry.rs`), t2195 (`src/libs/kt-plugins-builtin/tests/fuses.rs`)
