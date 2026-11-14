//! t2439 (gpu-throughput D6, capability `telemetry`, found-defects 486): the player's edit record
//! names the model's revision it took.

use kt_model::{Project, ProjectSettings};
use kt_time::trace::{self, Ev};

/// t2439: `set_project_at` records the model's revision in `a` and the player's version in `b`.
#[test]
fn t2439_an_edit_names_its_revision() {
    let p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    let (player, _rx) = kt_engine::Player::new(p.clone(), vec![]);
    let items = trace::testing::capture(|| {
        player.set_project_at(p.clone(), 41);
        player.set_project_at(p, 42);
    });
    let edits: Vec<_> = trace::testing::recs(&items).into_iter().filter(|r| r.kind() == Some(Ev::Edit)).collect();
    assert_eq!(edits.len(), 2, "an edit record per project handed");
    assert_eq!((edits[0].a, edits[1].a), (41, 42), "the edit records do not carry the model's revision");
    assert_eq!(edits[1].b, edits[0].b + 1, "the player's version follows");
}
