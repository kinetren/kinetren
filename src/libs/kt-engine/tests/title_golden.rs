//! Test 485 (R.52 C2): TITLE GOLDEN — the scene “a kt-text title over a
//! solid generator”. A separate reference `GOLDEN_TITLE_VIDEO`:
//! GOLDEN_VIDEO of the main golden test does not move (its project has no
//! generators), while the title reference is a guard of the determinism discipline of
//! kt-text (the vendored font, the harfrust/ab_glyph pins): their change
//! turns red HERE, and regeneration must be deliberate
//! (kt-text/CLAUDE.md, FONT-MANIFEST.txt).

use kt_engine::{VideoRenderer, render_raw_video};
use kt_media::{MediaSource, Quality};
use kt_model::*;
use kt_time::Tick;

fn fnv(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

type Sources = Vec<(MediaId, Box<dyn MediaSource>)>;

fn title_project() -> (Project, Sources) {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    let mut title = GeneratorKind::default_text();
    if let GeneratorKind::Text { text, bold, size, fill_r, fill_g, fill_b, .. } = &mut title {
        *text = "Kinotrain\nР.52 титр".into();
        *bold = true;
        *size = 0.16;
        (*fill_r, *fill_g, *fill_b) = (255, 136, 0);
    }
    let solid = GeneratorKind::Solid { color_r: 16, color_g: 32, color_b: 64 };
    // R.59, stage 2 (t662): on the media path the KIND stays, the values moved
    // to the event. ONLY THE PROJECT CONSTRUCTION was rewritten — the reference
    // `GOLDEN_TITLE_VIDEO` must stay the same to the bit.
    p.media.push(MediaRef::new(MediaId(1), gen_kind_path(&solid)));
    p.media.push(MediaRef::new(MediaId(2), gen_kind_path(&title)));
    let clip = |id: u64, lane: u32, media: u64| ClipEvent {
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
        duration: Tick(60_000),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    let mut back = clip(1, 1, 1);
    back.r#gen = Some(solid);
    let mut front = clip(2, 0, 2);
    front.r#gen = Some(title);
    p.clips = vec![back, front];
    p.normalize(); // tracks for the content (R.62)
    p.validate().unwrap();
    // the generator sources are opened by the export ITSELF: their addresses are derived
    // from the events, and the caller has nowhere to take them from
    let sources: Sources = vec![];
    (p, sources)
}

fn run_hash() -> (u64, Vec<u8>) {
    let (p, sources) = title_project();
    let q = Quality { width: 320, height: 180 };
    let mut all = Vec::new();
    let mut frame7 = Vec::new();
    let mut n = 0usize;
    render_raw_video(&p, sources, q, |rgba| {
        if n == 7 {
            frame7 = rgba.to_vec();
        }
        all.extend_from_slice(rgba);
        n += 1;
        Ok(())
    })
    .unwrap();
    (fnv(&all), frame7)
}

#[test]
fn t485_title_export_is_deterministic() {
    let (a, frame) = run_hash();
    let (b, _) = run_hash();
    assert_eq!(a, b, "the export of a title must be byte for byte between runs");
    // the title is really in the frame: there are pixels of the fill color over the background
    let orange = frame
        .chunks_exact(4)
        .filter(|px| px[0] > 200 && px[1] > 90 && px[1] < 180 && px[2] < 60)
        .count();
    assert!(orange > 100, "the title is not visible in the frame: {orange} pixels of fill");
    let bg = frame.chunks_exact(4).filter(|px| px == &[16, 32, 64, 255]).count();
    assert!(bg > 1000, "the solid background under the title disappeared: {bg}");
}

/// The reference is fixed on linux x86_64 (like GOLDEN_VIDEO). A deliberate
/// change of font/pins = regeneration with a record in FONT-MANIFEST.txt.
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
fn t485_title_matches_committed_golden() {
    let (v, _) = run_hash();
    assert_eq!(v, GOLDEN_TITLE_VIDEO, "the title stream diverged from the reference: 0x{v:016x}");
}

// REGENERATED in C3 of the same round: the fill went through a single
// premult composite of the decoration layers — at the edge (antialiased) pixels of the
// colored fill the low-order bits shifted. The previous (C2, fill-only
// path): 0x4cea52ff97ff15a5.
const GOLDEN_TITLE_VIDEO: u64 = 0x95267f57405a8c95;

#[test]
#[ignore = "reference generator: cargo test -p kt-engine --test title_golden -- --ignored print"]
fn t485_print_golden() {
    let (v, _) = run_hash();
    println!("GOLDEN_TITLE_VIDEO = 0x{v:016x}");
}

/// Preview == export on the title scene (invariant 7): frame No. 7 of the export
/// is byte-for-byte equal to the preview composite of the same moment.
#[test]
fn t485_title_preview_equals_export() {
    let (_, export_frame) = run_hash();
    let (p, sources) = title_project();
    let q = Quality { width: 320, height: 180 };
    let mut video = VideoRenderer::new(&p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let t = Tick(7 * 3000);
    let mut srcs: std::collections::HashMap<MediaId, Box<dyn MediaSource>> =
        sources.into_iter().collect();
    // the preview raises the generator sources with the same list as the export
    for (addr, state) in kt_engine::gen_sources(&p) {
        srcs.insert(addr, kt_media::factory::open_gen(&state));
    }
    for (media, src_pts, q) in video.layers_at(t, q) {
        let frame = srcs.get_mut(&media).unwrap().read_video_frame(src_pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video::video_cache_idx(src_pts),
            frame.into(),
        );
    }
    let (preview, complete) = video.compose_from_cache(&cache, t, q);
    assert!(complete, "the title composite must be complete");
    assert_eq!(preview, export_frame, "preview and export must match byte for byte");
}

/// Test 662 (R.59, stage 2): moving the generator's values to the EVENT
/// does NOT TOUCH THE BYTES.
///
/// Here only the project construction was rewritten (the values moved from the media
/// path to the clip), while the reference must stay the same to the bit. A red
/// test means that the round computed something differently, not that the reference
/// needs updating (round rule: GOLDEN_* are not regenerated).
#[test]
fn t662_moving_values_to_the_event_did_not_move_the_golden() {
    let (p, sources) = title_project();
    assert!(sources.is_empty(), "the export itself raises the generator sources");
    // the media path carries ONLY THE KIND
    assert_eq!(p.media[0].path, "gen:solid");
    assert_eq!(p.media[1].path, "gen:text", "title text does not live in the media path");
    // and the values — on the events, and each has its own
    assert!(matches!(p.clips[0].r#gen, Some(GeneratorKind::Solid { .. })));
    assert!(matches!(p.clips[1].r#gen, Some(GeneratorKind::Text { .. })));
    // the source address is derived from the EVENT
    assert_eq!(kt_engine::clip_source_addr(&p.clips[1]), kt_engine::gen_source_addr(p.clips[1].id));
    let (v, _) = run_hash();
    assert_eq!(v, 0x95267f57405a8c95, "the title reference shifted: 0x{v:016x} — this is a regression");
    assert_eq!(v, GOLDEN_TITLE_VIDEO);
}
