//! Тесты 644-645 (Р.59, п.467): цена автоматизации.
//!
//! Претензия владельца, ради которой это писалось: «если проект простой
//! без выкрутасов — то и лагать он не должен вовсе… если физику добавили
//! только на один сурс — то это не повод утяжелять работу всего проекта».
//! Оба теста утверждают ровно это и на уровне НАБЛЮДАЕМОГО следствия:
//! цепочка соседа ЗАИМСТВУЕТСЯ, а не копируется.

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
    p.media.push(MediaRef { id: MediaId(1), path: "synth:1".into() });
    p.clips = clips;
    p.video_lanes = lanes;
    p.validate().expect("fixture is legal");
    p
}

#[test]
fn t644_a_clip_without_curves_borrows_its_chain() {
    // сосед с ЖИВОЙ кривой, сосед без кривых и сосед с одной СИРОТОЙ
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

    // порядок слоёв — задний план первым (дорожка 2, 1, 0), то есть
    // сирота, «без кривых» и живая кривая
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
    // Цена кадра у СОСЕДЕЙ не зависит от того, сколько кривых на первом
    // клипе: план соседей обязан быть дословно тем же.
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
    // сосед идёт ПЕРВЫМ слоем (задний план — большая дорожка)
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
