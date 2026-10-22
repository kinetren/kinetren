//! Test 748 (R.63 item 2): the END OF THE MATERIAL is not starvation.
//!
//! The owner's event Rolling Stones lasted 316.967 s with a 226.268 s file:
//! at rate 0.75 it consumes 237.7 s of material, and the last 11.5 s
//! read past the end of the file. There was silence there — correct — but each such frame
//! counted as an underrun (180 480 frames per five seconds), and a region loop on the tail
//! looked like “the loop breaks the sound”.
//!
//! The CONSEQUENCE is asserted: past the edge of the material there is silence, zero underruns,
//! while inside the material a cache miss is still counted — otherwise the test
//! would “fix” the counter rather than the engine.

use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};

/// The material is 2 seconds, the event is 4: half is past the edge.
const MEDIA_SECS: i64 = 2;
const CLIP_SECS: i64 = 4;

fn project_kind(rate: Option<f64>, kind: ClipKind) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::with_duration(
        MediaId(1),
        "synth:1",
        Tick(MEDIA_SECS * 90_000),
    ));
    let mut c = ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind,
        lane: 0,
        start: Tick(0),
        duration: Tick(CLIP_SECS * 90_000),
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
    if let Some(r) = rate {
        c.fx = vec![FxInstance::slowed_reverb(r, 0.0, 120.0)];
        c.fx_bypass = vec![false];
    }
    p.clips.push(c);
    p.normalize();
    p.validate().unwrap();
    p
}

fn project(rate: Option<f64>) -> Project {
    project_kind(rate, ClipKind::Audio)
}

/// A source with a REAL end, like the file's.
fn fetcher_with_eof() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(
        MediaId(1),
        Box::new(
            SyntheticSource::new(Tick(MEDIA_SECS * 90_000), AudioPattern::Ramp).with_eof(),
        ),
    );
    f
}

fn render(p: &Project, f: &mut dyn AudioFetcher, from_s: f64, len_s: f64) -> (Vec<f32>, u64, u64) {
    let r = AudioRenderer::new(p);
    let frames = (len_s * 48_000.0) as usize;
    let mut out = vec![0f32; frames * AUDIO_CHANNELS];
    let st = r.render_frames(f, (from_s * 48_000.0) as i64, &mut out);
    (out, st.underruns, st.warmup_misses)
}

#[test]
fn t748_material_end_is_silence_not_starvation() {
    // ---- 1. direct event: the second half is past the material ----------------
    let p = project(None);
    let mut f = fetcher_with_eof();
    let (tail, under, warm) = render(&p, &mut f, 2.5, 1.0);
    assert!(
        tail.iter().all(|s| *s == 0.0),
        "there must be silence past the end of the material"
    );
    assert_eq!(
        (under, warm),
        (0, 0),
        "silence past the end of the material was counted as starvation (in the previous code — 48000 frames)"
    );

    // inside the material there is sound and no starvation either
    let (body, under, _) = render(&p, &mut f, 0.5, 0.5);
    assert!(body.iter().any(|s| *s != 0.0), "there must be sound inside the material");
    assert_eq!(under, 0, "there must be no misses inside the material");

    // ---- 2. SLOWDOWN: the edge moves right along the timeline -----------------
    // rate 0.5 — 2 s of material stretched to 4 s: the material's edge is exactly at the
    // end of the event, and there must be no silence at all
    let p = project(Some(0.5));
    let mut f = fetcher_with_eof();
    let (all, under, warm) = render(&p, &mut f, 0.0, 3.5);
    assert!(all.iter().any(|s| *s != 0.0), "a slowed event must sound");
    assert_eq!((under, warm), (0, 0), "a slowed event has no starvation");

    // ---- 2b. AS THE OWNER HAS IT: slowdown 0.75 and the tail past the material ------
    // 2 s of material at rate 0.75 is consumed in 2.67 s, and the event lasts 4 s:
    // the last 1.33 s read past the end of the file THROUGH THE FX CHAIN (the warp), not by the
    // direct path — this is exactly the Rolling Stones case.
    let p = project(Some(0.75));
    let mut f = fetcher_with_eof();
    let (tail, under, warm) = render(&p, &mut f, 3.0, 0.9);
    assert!(
        tail.iter().all(|s| s.abs() < 1e-6),
        "past the end of the material on the warp path there must be silence"
    );
    assert_eq!(
        (under, warm),
        (0, 0),
        "the tail of a warp event past the material was counted as starvation (the owner’s case)"
    );

    // ---- 3. starvation INSIDE the material is still counted -------------
    // an empty cache fetcher: there are no bursts, but there is material — this is a real
    // underrun, and it must remain visible
    struct Empty {
        end: Option<i64>,
    }
    impl AudioFetcher for Empty {
        fn audio_burst(
            &mut self,
            _m: MediaId,
            _i: i64,
        ) -> Option<std::sync::Arc<kt_media::AudioBurst>> {
            None
        }
        fn material_end_frames(&mut self, _m: MediaId) -> Option<i64> {
            self.end
        }
    }
    let p = project(None);
    let (_, under, _) = render(&p, &mut Empty { end: Some(MEDIA_SECS * 48_000) }, 0.0, 1.0);
    assert!(
        under >= 48_000 - AUDIO_BURST_FRAMES as u64,
        "a miss inside the material must count as an underrun, got {under}"
    );
    // and past the edge — zero, with the same fetcher
    let (_, under_tail, _) = render(&p, &mut Empty { end: Some(MEDIA_SECS * 48_000) }, 2.5, 1.0);
    assert_eq!(under_tail, 0, "there are no misses past the edge of the material");

    // ---- 3b. the FETCHER WRAPPER must pass the question about the edge ------------
    // A trap that cost half the task: `NestFetcher` wraps the fetcher
    // on EVERY project (it also drives `begin_clip`), and the trait's default is
    // “there is no edge”. If its wrapper did not override it, the mechanism would be silent entirely:
    // on the owner's project the tail counted as starvation for exactly this reason.
    {
        struct Wrap<'a>(&'a mut dyn AudioFetcher);
        impl AudioFetcher for Wrap<'_> {
            fn audio_burst(
                &mut self,
                m: MediaId,
                i: i64,
            ) -> Option<std::sync::Arc<kt_media::AudioBurst>> {
                self.0.audio_burst(m, i)
            }
            // deliberately does NOT override material_end_frames
        }
        let p = project(None);
        let mut inner = fetcher_with_eof();
        let (_, under, _) = render(&p, &mut Wrap(&mut inner), 2.5, 1.0);
        assert!(
            under > 0,
            "the scene is degenerate: a wrapper without delegation must lose the edge"
        );
        // but the engine's real wrapper — delegates
        let mut direct = fetcher_with_eof();
        assert_eq!(
            direct.material_end_frames(MediaId(1)),
            Some(MEDIA_SECS * 48_000),
            "DirectFetcher must know the end of the material"
        );
    }

    // ---- 4. without a known length — the former behaviour -----------------------
    let (_, under_unknown, _) = render(&p, &mut Empty { end: None }, 2.5, 1.0);
    assert!(
        under_unknown > 0,
        "without a known length the edge is not invented: we count as before"
    );
}

/// The same policy for VIDEO: past the end of the material the last frame is held, and
/// the prefetch order does not go past the edge (invariant 4 — the order equals the read).
#[test]
fn t748e_video_holds_the_last_frame_past_the_material_end() {
    use kt_engine::{SourceFacts, VideoRenderer};
    use kt_media::Quality;

    let p = project_kind(None, ClipKind::Video);
    let mut facts = SourceFacts::default();
    {
        use kt_media::MediaSource;
        let src =
            SyntheticSource::new(Tick(MEDIA_SECS * 90_000), AudioPattern::Ramp).with_eof();
        facts.observe(MediaId(1), src.info());
    }
    let vr = VideoRenderer::new_with_facts(&p, &facts);
    let q = Quality { width: 640, height: 360 };
    let last_inside = Tick(MEDIA_SECS * 90_000 - 3_000);
    let inside: Vec<Tick> =
        vr.layers_at(last_inside, q).into_iter().map(|(_, pts, _)| pts).collect();
    let beyond: Vec<Tick> =
        vr.layers_at(Tick(3 * 90_000), q).into_iter().map(|(_, pts, _)| pts).collect();
    assert_eq!(inside.len(), 1, "scene: one layer inside the material");
    assert_eq!(beyond.len(), 1, "past the edge the layer must stay (last frame)");
    assert!(
        beyond[0].0 < MEDIA_SECS * 90_000,
        "past the end of the material we ask for a NON-existent frame {:?}: the order went past the edge",
        beyond[0]
    );
    assert_eq!(beyond[0], inside[0], "the last frame of the material is held");

    // and the prefetch order on the tail does not go beyond the material
    let wanted =
        vr.wanted_frames(kt_time::TickRange::new(Tick(0), Tick(CLIP_SECS * 90_000)), q);
    assert!(
        wanted.iter().all(|(_, pts, _)| pts.0 < MEDIA_SECS * 90_000),
        "the order asks for frames past the end of the material"
    );
}
