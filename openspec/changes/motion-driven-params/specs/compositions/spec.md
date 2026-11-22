## ADDED Requirements

### Requirement: Two instances of a reel give its content their own outer speeds
The outer-speed channels of a reel's content SHALL be resolved per instance from that instance's speed; an instance's
sub-plan and sub-mix SHALL be keyed by its outer profile only when the reel's content reads the outer speed, and reels
that do not SHALL keep sharing as before.
Why: the owner's "двоякость-композиционность" — the same t0–t1 plain in one instance and fisheyed in another.

#### Scenario: One reel, two instances, two looks
- **WHEN** a reel with a Lens following the outer speed change is placed twice, one instance at speed 1, the other braking from 2 to 1 over the same reel range
- **THEN** the first instance shows no fisheye over t0–t1, the second shows it during the braking; a reel without such a link placed twice shares one sub-plan (one build in the trace)
- **TEST** t3950 (`src/libs/kt-engine/tests/motion_links.rs`)
