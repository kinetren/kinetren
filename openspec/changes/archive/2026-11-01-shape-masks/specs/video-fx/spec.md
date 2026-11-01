## ADDED Requirements

### Requirement: The mask rasterizer — shapes, antialiasing, feather with falloff, expansion, modes
One kt-fx function SHALL turn an ordered mask list, an affine "reference-normalized → raster px" and a
raster size into an 8-bit coverage: a rectangle (with corner roundness), an ellipse and a closed cubic
Bézier path are flattened deterministically; the inside is the nonzero winding at pixel centres; a
signed distance to the contour is computed in a band around it; one mask's coverage is a one-pixel
ramp without feather and `falloff(clamp(0.5 + d/feather))` with it (centred on the contour; falloff
linear or smoothstep), where `d` includes the expansion; then invert and opacity. Masks SHALL combine
top to bottom by the soft Boolean forms: add `a+m−am`, subtract `a(1−m)`, intersect `am`, difference
`a+m−2am`, starting from empty — or from the full frame when the first mask subtracts or intersects.
Feather and expansion are in px of the reference frame and scale with the raster, so a mask covers the
same region at every preview quality. Only exact IEEE operations and the frozen math are used, so the
coverage is the same on every platform; a bounded memo keyed by the static masks, the affine and the
size MUST be invisible to the bytes.
Why: F-MSK-001 — feather and expansion are the classic controls; resolution invariance is why the
coordinates are normalized; determinism is invariant 11.

#### Scenario: Shapes cover their area with antialiased edges
- **WHEN** a rectangle, a rotated rectangle, a rounded rectangle, an ellipse and a Bézier path approximating a circle are rasterized without feather
- **THEN** the coverage sum equals the analytic area within 0.5 %; edge pixels take intermediate values, pixels a pixel away from the contour are 0 or 255; the Bézier circle matches the ellipse within one LSB on 99 % of pixels
- **TEST** t1266 (`src/libs/kt-fx/tests/masks.rs`)

#### Scenario: Feather, falloff, expansion and resolution
- **WHEN** a disc is rasterized with feather 0/20/80 px, both falloffs, expansion −10/+10 px, and the same mask at 1/4 and at 1/1 of the raster
- **THEN** the profile across the edge is monotonic, the 50 % level stays on the contour (moved by the expansion exactly), a wider feather is wider, smooth is steeper in the middle than linear; the 1/4 raster equals the 1/1 raster box-downsampled within 3 LSB
- **TEST** t1267 (`src/libs/kt-fx/tests/masks.rs`)

#### Scenario: Modes, invert and opacity
- **WHEN** two overlapping hard rectangles are combined in each mode, in both orders, with the first one inverted, and at 50 % opacity
- **THEN** the four regions (A only, B only, both, none) take the truth-table values of OR, AND NOT, AND, XOR; a first subtract/intersect starts from the full frame; invert swaps 0 and 255; opacity scales the mask's own contribution
- **TEST** t1268 (`src/libs/kt-fx/tests/masks.rs`)

#### Scenario: The coverage is deterministic and the memo is invisible
- **WHEN** a fixed scene of masks is rasterized twice, with and without the memo
- **THEN** the bytes equal a hash frozen in the test, and are equal with and without the memo
- **TEST** t1269 (`src/libs/kt-fx/tests/masks.rs`)
