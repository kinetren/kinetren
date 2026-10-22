//! R.59, stage 2: the generator's parameters live ON THE EVENT.
//!
//! Tests t658, t659, t663 and t670. All four are about one thing: the values have moved from
//! the media to the event, so each event has its OWN source address —
//! and everything that sits on “order == read” (invariant 4) must ask
//! this address identically: the plan, prefetch, composite, export and bars.

use kt_engine::{VideoRenderer, render_raw_video};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::Tick;

type Sources = Vec<(MediaId, Box<dyn MediaSource>)>;

const Q: Quality = Quality { width: 96, height: 54 };

fn base() -> Project {
    Project::new(ProjectSettings {
        fps: 30,
        width: 96,
        height: 54,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    })
}

fn title(text: &str) -> GeneratorKind {
    let mut k = GeneratorKind::default_text();
    if let GeneratorKind::Text { text: t, size, .. } = &mut k {
        *t = text.into();
        *size = 0.4;
    }
    k
}

fn clip(id: u64, lane: u32, start: i64, dur: i64, media: u64) -> ClipEvent {
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
        duration: Tick(dur),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

/// Two titles of ONE kind (one media) with different text.
fn two_titles() -> Project {
    let mut p = base();
    p.media.push(MediaRef::new(MediaId(1), gen_kind_path(&title(""))));
    let mut a = clip(1, 1, 0, 90_000, 1);
    a.r#gen = Some(title("AAAA"));
    let mut b = clip(2, 0, 0, 90_000, 1);
    b.r#gen = Some(title("BB"));
    // the upper layer is shifted down so that the letters do not overlap each other
    if let Some(GeneratorKind::Text { pos_y, .. }) = &mut b.r#gen {
        *pos_y = 0.8;
    }
    p.clips = vec![a, b];
    p.normalize(); // tracks fitted to the content (R.62)
    p.validate().expect("two titles on one view are legal");
    p
}

fn gen_srcs(p: &Project) -> std::collections::HashMap<MediaId, Box<dyn MediaSource>> {
    kt_engine::gen_sources(p)
        .into_iter()
        .map(|(addr, st)| (addr, kt_media::factory::open_gen(&st)))
        .collect()
}

/// The composite of the moment `t`, warmed EXACTLY by what `layers_at` ordered.
fn compose(p: &Project, t: Tick) -> (Vec<u8>, bool) {
    let mut video = VideoRenderer::new(p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let mut srcs = gen_srcs(p);
    for (addr, pts, dq) in video.layers_at(t, Q) {
        let f = srcs
            .get_mut(&addr)
            .unwrap_or_else(|| panic!("address {addr:?} was requested, but there is no source under it"))
            .read_video_frame(pts, Some(dq))
            .unwrap();
        cache.insert_video(
            kt_engine::source_id(addr),
            Some(dq),
            kt_engine::video::video_cache_idx(pts),
            f.into(),
        );
    }
    video.compose_from_cache(&cache, t, Q)
}

/// Test 658 (R.59, stage 2): ORDER == READ for two titles of ONE kind
/// with different text.
///
/// Before this round such a scene did not exist: the text was part of the media path,
/// and two titles with different text were two DIFFERENT media. Now the media is one,
/// while the sources are two — one per event — and the whole cache
/// arithmetic must hold this: the addresses differ, the composite is complete,
/// the pictures are different.
#[test]
fn t658_order_equals_read_for_two_titles_of_one_kind() {
    let p = two_titles();
    let video = VideoRenderer::new(&p);
    let order = video.layers_at(Tick(0), Q);
    assert_eq!(order.len(), 2, "both layers were requested");
    assert_ne!(order[0].0, order[1].0, "two events must have DIFFERENT source addresses");
    assert_eq!(
        order[0].0,
        kt_engine::gen_source_addr(ClipId(1)),
        "the address is computed from the event, not from the media (order — background first)"
    );

    // the order key contains exactly what the source can give
    let (frame, complete) = compose(&p, Tick(0));
    assert!(complete, "composite incomplete: the order and the read diverged by address");
    assert!(frame.iter().any(|&x| x != 0), "frame is empty — nothing to compare");

    // and the texts are DIFFERENT: one media no longer means one picture
    let mut srcs = gen_srcs(&p);
    let ink = |f: &kt_media::VideoFrame| f.rgba.chunks_exact(4).map(|px| px[3] as u64).sum::<u64>();
    let a = srcs
        .get_mut(&kt_engine::gen_source_addr(ClipId(1)))
        .unwrap()
        .read_video_frame(Tick(0), Some(Q))
        .unwrap();
    let b = srcs
        .get_mut(&kt_engine::gen_source_addr(ClipId(2)))
        .unwrap()
        .read_video_frame(Tick(0), Some(Q))
        .unwrap();
    assert_ne!(a.rgba, b.rgba, "two events on one media gave one picture");
    assert!(ink(&a) > ink(&b), "“AAAA” must take up more ink than “BB”");
}

/// Test 659 (R.59, stage 2): a curve on a generator parameter changes the
/// BYTES, and does so deterministically.
///
/// Curves are resolved by the SOURCE from its own `pts` — the plan does not know
/// about them at all. So here we check the pair “the frame as a function of tick” and
/// “a second run equals the first to the bit” (invariant 11).
#[test]
fn t659_a_curve_on_a_generator_parameter_moves_the_bytes() {
    let mut p = base();
    p.media.push(MediaRef::new(MediaId(1), gen_kind_path(&title(""))));
    let mut c = clip(1, 0, 0, 180_000, 1);
    let mut k = title("ANIMATED");
    // a “typewriter” preset — the same formulas as the migration
    text_anim_values(&mut k, TextAnim::Typewriter);
    c.r#gen = Some(k);
    c.gen_auto = text_anim_curves(TextAnim::Typewriter, 1.0, 2.0);
    p.clips = vec![c];
    p.validate().expect("generator curve is legal");

    let shot = |t: Tick| compose(&p, t).0;
    let ink = |f: &[u8]| f.chunks_exact(4).map(|px| px[3] as u64).sum::<u64>();
    let t0 = shot(Tick(0));
    let t_mid = shot(Tick(45_000));
    let t_end = shot(Tick(90_000));
    assert!(ink(&t_mid) > ink(&t0), "by the middle the curve must reveal part of the text");
    assert!(ink(&t_end) > ink(&t_mid), "by the end — all of it");

    // determinism: the same tick — the same bytes, and this does not depend on
    // how many frames were computed up to it
    assert_eq!(t_mid, shot(Tick(45_000)), "a frame with a curve must be a function of the TICK");

    // without curves the picture does not depend on time at all (zero cost)
    let mut flat = p.clone();
    flat.clips[0].gen_auto.clear();
    let a = compose(&flat, Tick(0)).0;
    let b = compose(&flat, Tick(45_000)).0;
    assert_eq!(a, b, "a non-animated title must not depend on the tick in its frame");
}

/// Test 663 (R.59, stage 2): a REEL does not return a stale frame after a retuning of the
/// generator INSIDE it.
///
/// A hidden defect this closes: an edit of values goes through the retune band and the model
/// does not move, while the sub-composite memoization key is the reel's `content_hash`,
/// computed from the model. So it does not change, and the reel keeps returning the picture
/// with the old text until the first edit of the plan. The answer is invalidation of the
/// ancestors by the same code (`reel_chain_of_clip`).
#[test]
fn t663_a_reel_does_not_serve_a_stale_frame_after_a_retune() {
    use kt_engine::Player;

    let mut p = base();
    let dark = GeneratorKind::Solid { color_r: 16, color_g: 16, color_b: 16 };
    let bright = GeneratorKind::Solid { color_r: 220, color_g: 220, color_b: 220 };
    p.media.push(MediaRef::new(MediaId(1), gen_kind_path(&dark)));
    let mut s = Sequence::new(MediaId(100), "reel");
    let mut inner = clip(1, 0, 0, 90_000, 1);
    inner.r#gen = Some(dark);
    s.clips = vec![inner];
    p.sequences.push(s);
    p.clips = vec![clip(10, 0, 0, 90_000, 100)];
    p.validate().expect("a generator inside a reel is legal");

    let (player, _consumer) = Player::new(p, vec![]);
    player.set_preview_quality(Q);

    // warm: the reel's composite moves to memo by its revision
    let mut buf = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let _ = player.compose_preview_into(Tick(0), &mut buf);
        if buf.first().is_some_and(|&x| x == 16) {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "the dark frame of the reel did not arrive");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert!(player.update_gen(ClipId(1), bright, Vec::new()), "reconfiguration must be applied");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    loop {
        let _ = player.compose_preview_into(Tick(0), &mut buf);
        if buf.first().is_some_and(|&x| x == 220) {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "REEL serves a stale result: the sub-composite is memoized by the model revision, and \
             editing its values does not move it — ancestors must be invalidated explicitly"
        );
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

/// Test 670 (R.59, stage 2, sibling of t11/t479): export opens the
/// generators ITSELF, and the preview equals the export byte for byte (invariant 7).
///
/// The source address is derived from the event — the caller has nowhere to get it,
/// so the list of sources is computed by `gen_sources`, and it is computed by ONE
/// code for the player, export and kt-cli.
#[test]
fn t670_export_opens_its_own_generators_and_equals_preview() {
    let p = two_titles();
    let target = 7usize;
    let run = || {
        let mut captured: Option<Vec<u8>> = None;
        let mut all = Vec::new();
        let mut n = 0usize;
        // NOT ONE source from outside: the generators are raised by export itself
        let sources: Sources = vec![];
        render_raw_video(&p, sources, Q, |rgba| {
            if n == target {
                captured = Some(rgba.to_vec());
            }
            all.extend_from_slice(rgba);
            n += 1;
            Ok(())
        })
        .unwrap();
        (captured.expect("frame captured"), all)
    };
    let (frame_a, all_a) = run();
    let (_, all_b) = run();
    assert_eq!(all_a, all_b, "export with generators on events must be deterministic");
    assert!(frame_a.iter().any(|&x| x != 0), "export gave black — nothing to compare");

    let (preview, complete) = compose(&p, Tick(target as i64 * 3000));
    assert!(complete, "the preview composite is incomplete");
    assert_eq!(preview, frame_a, "preview and export must match byte for byte");
}
