# Design

## Context

A time warp is the `rate` row of `slowed_reverb` (−32…32, R.57); its curve is in the material-tick domain anchored at
`source_in`, linear in timeline time; `avg_rate` and `rate_profile_keys` are what every reader uses. The event's length
does not change by a curve (R.37).

## Decisions

### D1. Shapes are fractions and multipliers
`speed_ramp(kind, src_in, len, avg) -> Vec<AutoKey>`: keys at fractions `f` of the event's length with multipliers `m` and
an ease segment, then every value is multiplied by `c` so that the curve's mean over the event equals `avg` (the rate
mean the event already has, 1 for a fresh slot): the event reads exactly the same span of material, so its last frame does
not move and no neighbour does. The mean is computed on the expanded profile (`rate_profile_keys` → `avg_rate`) with two
Newton steps for ease segments' drift. Kinds: Montage (1.6 → 0.5 → 1.6, three bumps), Hero (2 → 0.25 in the middle third
→ 2), Bullet (3 → 0.15 → 3, short), Jump Cut (alternating 2.5 and 0.6 by Hold), Flash In (4 → 1 over the first
third), Flash Out (1 → 4 over the last third).
A shape that would pass |32| after scaling is clamped and the mean then differs: the writer refuses the click with the
reason (an event longer than its material at that mean). No reversed ramps: a negative mean keeps its sign on all keys.

### D2. Where a click lands
In the automation lane of `rate` (and the slot's own row menu) an icon opens the six with a small drawn curve each. On
an event with no warp slot the same icon in the event's menu adds the slot (`slowed_reverb` at its defaults, video only
for a linked pair, the pair's sound follows by `sync_warp_to_link_group`) and the curve in one undo step.
A selection of several events applies to each with its own length.

### D3. Sound
The pair's mirrored speed is as for any curve; `slowed_reverb`'s reverb stays at its own rows. A linked sound that goes
above 2× is heard shifted: the ramp's tip says so.

### D4. Class walk
Other ramp-like presets: opacity and zoom ramps are chain presets (camera, `fx-lens-and-camera`); a composition instance
has no own warp slot (its speed is the instance's: the same row), so it works the same. Reverse and freeze (R.57) are
the row's values, already there.
