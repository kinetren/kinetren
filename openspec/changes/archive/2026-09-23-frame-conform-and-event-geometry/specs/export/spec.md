# Spec Delta

## ADDED Requirements

### Requirement: Export in a foreign resolution arrives at the same fractions
Export at a resolution different from the project's raster SHALL compute the layers' geometry in fractions of the
export canvas by the same function as the preview: the export frame is similar to the preview composite within the
tolerance of the resampling class; the conform rule and the node's position are not recomputed by hand. Golden on matching proportions MUST remain byte for byte without regeneration.
Why: before R.62, "Max (W×H)" added up the maximum width and height independently and offered a resolution that no source had, and the result was stretched.

#### Scenario: Similarity of preview and export at a foreign resolution
- **WHEN** a 1920×1080 project with a vertical source is exported at 1280×720
- **THEN** the layer's rectangle is proportional to the 1920×1080 preview within tolerance, the margins are in place; t715 is green
- **TEST** t728b (`src/libs/kt-engine/tests/golden_export.rs`)
