//! Р.62 (change B): гашение ВИДЕОДОРОЖКИ — mute и solo.
//!
//! Заглушённая дорожка не композится, но её ЗАКАЗ префетча остаётся:
//! иначе снятие mute давало бы чёрный кадр до прогрева (инвариант 4 —
//! греем ровно то, что прочтём).

use kt_engine::VideoRenderer;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality};
use kt_model::LaneKind::Video as V;
use kt_model::*;
use kt_time::Tick;

const Q: Quality = Quality { width: 64, height: 36 };

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: 64,
        height: 36,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    }
}

fn clip(id: u64, lane: u32, media: u64) -> ClipEvent {
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
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

/// Два слоя на ОДНОМ источнике, но с разным `source_in`: синтетический
/// кадр красится НОМЕРОМ тика, поэтому верхний слой (дорожка 0) читает
/// кадр 3000 (r = 184), нижний — кадр 0 (r = 0). Так по одному пикселю
/// видно, какой слой попал в композит.
const TOP_IN: i64 = 3_000;

fn project() -> Project {
    let mut p = Project::new(settings());
    p.media.push(MediaRef { id: MediaId(1), path: "synth:silence".into() });
    let mut top = clip(1, 0, 1);
    top.source_in = Tick(TOP_IN);
    p.clips = vec![top, clip(2, 1, 1)];
    p.normalize();
    p.validate().unwrap();
    p
}

fn sources() -> Vec<(MediaId, Box<dyn MediaSource>)> {
    vec![(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence))
            as Box<dyn MediaSource>,
    )]
}

fn render(p: &Project) -> (Vec<u8>, bool) {
    let mut facts = kt_engine::SourceFacts::default();
    for (id, s) in sources() {
        facts.observe(id, s.info());
    }
    let mut video = VideoRenderer::new_with_facts(p, &facts);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources().into_iter().collect();
    // ЗАКАЗ — то, что просит движок; читаем ровно его
    for (media, pts, dq) in video.layers_at(Tick(0), Q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(pts, Some(dq)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(dq),
            kt_engine::video::video_cache_idx(pts),
            frame.into(),
        );
    }
    video.compose_from_cache(&cache, Tick(0), Q)
}

/// Что ЗАКАЗАНО: пары «источник, кадр» — по ним видно, не пропал ли
/// префетч заглушённой дорожки.
fn ordered(p: &Project) -> Vec<(u64, i64)> {
    let mut facts = kt_engine::SourceFacts::default();
    for (id, s) in sources() {
        facts.observe(id, s.info());
    }
    let video = VideoRenderer::new_with_facts(p, &facts);
    let mut v: Vec<(u64, i64)> =
        video.layers_at(Tick(0), Q).into_iter().map(|(m, t, _)| (m.0, t.0)).collect();
    v.sort();
    v.dedup();
    v
}

fn lum(f: &[u8]) -> u8 {
    f[0]
}

/// Тест 735 (Р.62): mute и solo видеодорожки.
#[test]
fn t735_video_mute_and_solo_leave_the_prefetch_order() {
    // сцена: сверху яркий слой — он и виден
    let base = project();
    let (f, complete) = render(&base);
    assert!(complete);
    let top_px = (TOP_IN & 0xff) as u8;
    assert_eq!(lum(&f), top_px, "сцена: в кадре ВЕРХНИЙ слой");
    assert_eq!(ordered(&base), vec![(1, 0), (1, TOP_IN)], "сцена: заказаны оба кадра");

    // MUTE верхней дорожки — в кадре нижний слой, а ЗАКАЗ прежний
    let mut p = base.clone();
    p.track_at_mut(V, 0).unwrap().mute = true;
    let (f, complete) = render(&p);
    assert!(complete);
    assert_eq!(lum(&f), 0, "заглушённая дорожка попала в композит");
    assert_eq!(
        ordered(&p),
        vec![(1, 0), (1, TOP_IN)],
        "заказ заглушённой дорожки пропал — снятие mute дало бы чёрный кадр"
    );

    // SOLO — множество СВОЕГО вида: в композите ровно соло-дорожки
    let mut p = base.clone();
    p.track_at_mut(V, 1).unwrap().solo = true;
    let (f, _) = render(&p);
    assert_eq!(lum(&f), 0, "в соло обязана остаться нижняя дорожка");
    assert_eq!(ordered(&p), vec![(1, 0), (1, TOP_IN)], "соло не имеет права терять заказ");

    // снятие mute возвращает кадр БЕЗ промаха: заказ всё это время был
    let mut p = base.clone();
    p.track_at_mut(V, 0).unwrap().mute = true;
    let _ = render(&p);
    p.track_at_mut(V, 0).unwrap().mute = false;
    let (f, complete) = render(&p);
    assert!(complete, "после снятия mute кадр обязан быть полным");
    assert_eq!(lum(&f), top_px);
}
