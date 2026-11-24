## ADDED Requirements

### Requirement: Two instances of a reel give its content their own outer speeds
The outer-speed channels of a reel's content SHALL be read per instance. The picture SHALL read them at the moment of the outermost timeline that is shown, so a value goes on changing while the instance stands still; the instances
SHALL go on sharing one sub-renderer, and a kept composite SHALL be keyed by the channel values its layers used. The sound SHALL read them at the first outer moment that plays each inner moment, and an instance's sub-mix SHALL have a renderer and a
scene of its own only when the reel's content reads them. A stream stage's rows SHALL read them as the sound does, with a history per instance.
Why: the owner's "двоякость-композиционность" — the same t0–t1 plain in one instance and fisheyed in another.

#### Scenario: One reel, two instances, two looks
- **WHEN** a reel with a Lens following the outer speed change is placed twice, one instance at speed 1, the other braking from 2 to 1 over the same reel range
- **THEN** the first instance shows no fisheye over t0–t1, the second shows it during the braking; both instances share one sub-renderer, and a reel without such a curve keeps its keys and bytes
- **TEST** t3950 (`src/libs/kt-engine/tests/motion_links.rs`)

#### Scenario: The value dies away while the composition stands
- **WHEN** an instance brakes from 1 to 0 over half a second and stands for two seconds, its reel holding a Lens that follows the speed change and a Datamosh whose rate follows the speed, and a second instance plays the reel at 1
- **THEN** the Lens strength falls to zero within the release while the reel's moment does not move, and resumes from zero; no kept composite of the stop's first frame is shown at its last; the Datamosh reads the first play of each moment, and the two instances' histories are separate
- **TEST** t4146 (`src/libs/kt-engine/tests/motion_links.rs`)

#### Scenario: Two instances sound by their own speeds
- **WHEN** a reel with a Low-pass whose cutoff follows the outer speed is placed twice at speeds 1 and 0.5, played until kept, and the second instance's speed is edited
- **THEN** the sub-mixes differ as the cutoff does, only the second instance's bursts are forgotten, and an export's samples equal the play's
- **TEST** t4147 (`src/libs/kt-engine/tests/motion_links.rs`)
