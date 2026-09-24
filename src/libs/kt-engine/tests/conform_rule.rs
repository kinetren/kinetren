//! Тест 718 (Р.62): правило приведения — у ПРОЕКТА с исключением у
//! события. Смена умолчания проекта переписывает событие «следовать
//! проекту» и не трогает событие с явным правилом (правило UI 20).

use kt_engine::{SourceFacts, VideoRenderer};
use kt_media::Quality;
use kt_model::*;
use kt_time::Tick;

fn project(rules: &[Conform], project_rule: ConformRule) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1920,
        height: 1080,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: project_rule,
        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "vertical"));
    for (i, r) in rules.iter().enumerate() {
        let mut c = ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            id: ClipId(1 + i as u64),
            link_group: None,
            kind: ClipKind::Video,
            lane: i as u32,
            start: Tick(0),
            duration: Tick(90_000),
            source_in: Tick(0),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        };
        c.geometry.rule = *r;
        p.clips.push(c);
    }
    p.normalize(); // дорожки под содержимое (Р.62)
    p.validate().unwrap();
    p
}

fn decodes(p: &Project) -> Vec<(u64, (u32, u32))> {
    let facts = SourceFacts {
        natives: std::collections::HashMap::from([(MediaId(1), (1080, 1920))]),
        procedural: Default::default(),
        views: std::collections::HashMap::from([(MediaId(1), (1080.0, 1920.0))]), ..Default::default() };
    let vr = VideoRenderer::new_with_facts(p, &facts);
    let mut v: Vec<(u64, (u32, u32))> = vr
        .video_decode_qualities(Quality { width: 1920, height: 1080 })
        .into_iter()
        .map(|(id, q)| (id.0, (q.width, q.height)))
        .collect();
    v.sort();
    v
}

#[test]
fn t718_project_rule_rewrites_followers_and_spares_explicit() {
    const FIT: (u32, u32) = (608, 1080);
    const FILL: (u32, u32) = (1920, 3412);
    const AS_IS: (u32, u32) = (1080, 1920);
    // событие 1 следует проекту, событие 2 — явное «заполнить»
    let p = project(&[Conform::Project, Conform::Fill], ConformRule::Fit);
    assert_eq!(decodes(&p), vec![(1, FIT), (2, FILL)]);
    // умолчание проекта сменилось — следующий за ним меняется, явный нет
    let p = project(&[Conform::Project, Conform::Fill], ConformRule::AsIs);
    assert_eq!(decodes(&p), vec![(1, AS_IS), (2, FILL)]);
    // явное «вписать» стоит и при проекте «заполнить»
    let p = project(&[Conform::Fit, Conform::Project], ConformRule::Fill);
    assert_eq!(decodes(&p), vec![(1, FIT), (2, FILL)]);
    // «растянуть» — холст целиком; проект «вписать» по умолчанию не пишется
    let p = project(&[Conform::Stretch], ConformRule::Fit);
    assert_eq!(decodes(&p), vec![(1, (1920, 1080))]);
    assert_eq!(p.settings.conform, ConformRule::Fit);
}
