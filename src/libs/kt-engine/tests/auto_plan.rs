//! Tests 644-645 (R.59, item 467): the cost of automation.
//!
//! The owner's claim that this was written for: “if the project is simple
//! with no tricks — it must not lag at all… if physics was added
//! to only one source — that is no reason to slow down the whole project.”
//! Both tests assert exactly this, at the level of the OBSERVABLE consequence:
//! the neighbour's chain is BORROWED, not copied.

use kt_engine::VideoRenderer;
use kt_media::Quality;
use kt_model::*;
use kt_time::Tick;

const SEC: i64 = 90_000;

fn clip(
    id: u64,
    lane: u32,
    start: i64,
    fx: Vec<FxInstance>,
    auto: Vec<Vec<ParamCurve>>,
) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane,
        start: Tick(start),
        duration: Tick(2 * SEC),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx,
        fx_bypass: vec![],
        fx_auto: auto,
        opacity: 1.0,
        fx_off: false,
    }
}

fn curve(param: &str, a: f64, b: f64) -> ParamCurve {
    ParamCurve {
        param: param.into(),
        keys: vec![AutoKey::at(Tick(0), a), AutoKey::at(Tick(2 * SEC), b)],
        source: CurveSource::Keys,
    }
}

fn project(clips: Vec<ClipEvent>, lanes: u32) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "synth:1"));
    p.clips = clips;
    p.ensure_tracks(kt_model::LaneKind::Video, lanes);
    p.validate().expect("fixture is legal");
    p
}

#[test]
fn t644_a_clip_without_curves_borrows_its_chain() {
    // neighbour with a LIVE curve, a neighbour without curves, and a neighbour with one ORPHAN
    let p = project(
        vec![
            clip(1, 0, 0, vec![FxInstance::vignette()], vec![vec![curve("strength", 0.1, 0.9)]]),
            clip(2, 1, 0, vec![FxInstance::vignette()], vec![]),
            clip(3, 2, 0, vec![FxInstance::vignette()], vec![vec![curve("no_such", 0.0, 1.0)]]),
        ],
        3,
    );
    let v = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = Quality { width: 64, height: 36 };
    let plan = v.frame_plan(&cache, Tick(SEC), q);

    // layer order — the background first (track 2, 1, 0), that is the
    // orphan, “without curves” and the live curve
    let owned: Vec<bool> = plan
        .layers
        .iter()
        .filter_map(|l| match l {
            kt_engine::video_plan::PlanLayer::One { src, .. } => {
                Some(matches!(src.fx, std::borrow::Cow::Owned(_)))
            }
            _ => None,
        })
        .collect();
    assert_eq!(owned.len(), 3, "the fixture must give three layers");
    assert_eq!(
        owned,
        vec![false, false, true],
        "only the clip with a LIVE curve may clone its chain; a clip without curves and a clip \
         with an orphan curve must borrow"
    );
}

#[test]
fn t645_curves_on_one_clip_do_not_tax_the_others() {
    // the frame cost of the NEIGHBOURS does not depend on how many curves there are on the
    // first clip: the neighbours' plan must be literally the same.
    let plain = |auto: Vec<Vec<ParamCurve>>| {
        project(
            vec![
                clip(1, 0, 0, vec![FxInstance::vignette()], auto),
                clip(2, 1, 0, vec![FxInstance::vignette()], vec![]),
            ],
            2,
        )
    };
    let bare = plain(vec![]);
    let loaded = plain(vec![vec![
        curve("strength", 0.1, 0.9),
        curve("softness", 0.1, 0.9),
        curve("radius", 0.3, 0.9),
    ]]);
    let q = Quality { width: 64, height: 36 };
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    // the neighbour goes as the FIRST layer (the background is the larger track)
    let neighbour = |p: &Project| -> (bool, u64, Vec<FxInstance>) {
        let v = VideoRenderer::new(p);
        let plan = v.frame_plan(&cache, Tick(SEC), q);
        let kt_engine::video_plan::PlanLayer::One { src, .. } = &plan.layers[0] else {
            panic!("the first layer must be an ordinary one");
        };
        (matches!(src.fx, std::borrow::Cow::Owned(_)), src.fx_hash, src.fx.to_vec())
    };
    assert_eq!(
        neighbour(&bare),
        neighbour(&loaded),
        "curves on the first clip changed what the SECOND clip costs"
    );
}
