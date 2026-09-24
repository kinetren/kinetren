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

/// Тест 736 (Р.62, change B): дорожка с цепочкой — ОДИН слой-вложение.
///
/// Утверждение, ради которого раунд затевался: цепочка ложится на
/// КОМПОЗИТ дорожки, а не дописывается в каждое событие. Проверяется на
/// ПЕРЕХОДЕ: в его окне поклипная цепочка считала бы эффект дважды (по
/// разу на каждую половину), а дорожечная — ровно один раз, поверх уже
/// смешанной картинки.
#[test]
fn t736_track_chain_covers_the_whole_composite() {
    use kt_model::{BlendMode, FxInstance};
    // одна дорожка, два события встык с переходом между ними
    let mut p = Project::new(settings());
    p.media.push(MediaRef { id: MediaId(1), path: "synth:silence".into() });
    let mut a = clip(1, 0, 1);
    a.duration = Tick(6_000);
    let mut b = clip(2, 0, 1);
    b.start = Tick(6_000);
    b.duration = Tick(6_000);
    b.source_in = Tick(TOP_IN);
    p.clips = vec![a, b];
    p.normalize();
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(1)),
        b: Some(ClipId(2)),
        in_offset: Tick(3_000),
        out_offset: Tick(3_000),
        kind: TransitionKind::Crossfade,
        audio: Default::default(),
        auto: vec![],
        invert: false,
    });
    p.validate().unwrap();
    let mid = Tick(6_000); // середина окна перехода

    let render_at = |p: &Project, t: Tick| -> Vec<u8> {
        let mut facts = kt_engine::SourceFacts::default();
        for (id, s) in sources() {
            facts.observe(id, s.info());
        }
        let mut video = VideoRenderer::new_with_facts(p, &facts);
        let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
        let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
            sources().into_iter().collect();
        for (media, pts, dq) in video.layers_at(t, Q) {
            let frame = srcs.get_mut(&media).unwrap().read_video_frame(pts, Some(dq)).unwrap();
            cache.insert_video(
                kt_engine::source_id(media),
                Some(dq),
                kt_engine::video::video_cache_idx(pts),
                frame.into(),
            );
        }
        video.compose_from_cache(&cache, t, Q).0
    };

    // ЦЕПОЧКА НА ДОРОЖКЕ
    let mut on_track = p.clone();
    on_track.track_at_mut(V, 0).unwrap().fx = vec![FxInstance::Posterize { levels: 3 }];
    on_track.validate().unwrap();

    // ТА ЖЕ цепочка, дописанная В КАЖДОЕ СОБЫТИЕ
    let mut per_clip = p.clone();
    for c in per_clip.clips.iter_mut() {
        c.fx = vec![FxInstance::Posterize { levels: 3 }];
    }
    per_clip.validate().unwrap();

    let track = render_at(&on_track, mid);
    let clips = render_at(&per_clip, mid);
    assert_ne!(track, clips, "цепочка дорожки совпала с поклипной — значит она не на композите");
    // и это НЕ «ничего не сделала»: против дорожки без цепочки разница есть
    assert_ne!(track, render_at(&p, mid), "цепочка дорожки вообще не применилась");

    // БЕЗ УСЛОВИЯ — путь прежний: дорожка без цепочки, уровня и режима
    // даёт ровно то же, что и до Р.62 (на этом стоит GOLDEN_VIDEO)
    let plain = render_at(&p, mid);
    let mut same = p.clone();
    same.track_at_mut(V, 0).unwrap().opacity = 1.0;
    same.track_at_mut(V, 0).unwrap().blend = BlendMode::Normal;
    assert_eq!(render_at(&same, mid), plain, "дорожка без свойств изменила байты");
}

/// Тест 737 (Р.62, change B): режимы наложения против аналитической
/// формулы. Уровень и режим — РАЗНЫЕ оси, обе после альфы; дыра в
/// композите дорожки оставляет нижние слои нетронутыми.
#[test]
fn t737_blend_modes_match_the_formula() {
    use kt_engine::video::blend_ch;
    use kt_model::BlendMode as M;
    // формулы — ровно те, что записаны в спеке
    for (b, a) in [(0.2f64, 0.6f64), (0.8, 0.3), (0.0, 1.0), (1.0, 0.0)] {
        assert_eq!(blend_ch(M::Normal, b, a), a);
        assert_eq!(blend_ch(M::Multiply, b, a), b * a);
        assert_eq!(blend_ch(M::Screen, b, a), b + a - b * a);
        assert_eq!(blend_ch(M::Add, b, a), (b + a).min(1.0));
        assert_eq!(blend_ch(M::Subtract, b, a), (b - a).max(0.0));
        assert_eq!(blend_ch(M::Darken, b, a), b.min(a));
        assert_eq!(blend_ch(M::Lighten, b, a), b.max(a));
        assert_eq!(blend_ch(M::Difference, b, a), (b - a).abs());
        let want = if b <= 0.5 { 2.0 * b * a } else { 1.0 - 2.0 * (1.0 - b) * (1.0 - a) };
        assert_eq!(blend_ch(M::Overlay, b, a), want);
    }
    // реестр полон: у каждого вида своя формула, и ни один не забыт
    assert_eq!(M::ALL.len(), 9);
}
