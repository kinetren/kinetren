//! gen-audio-reactive: what an audio-reactive generator hears — its feed, built by `gen_sources`.

use std::sync::Arc;

use kt_model::*;
use kt_plugin_api::audio_series::AudioSeries;
use kt_time::Tick;

const SEC: i64 = 90_000;

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

fn clip(id: u64, kind: ClipKind, lane: u32, media: u64, start: i64, dur: i64, source_in: i64) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        masks: Default::default(),
        id: ClipId(id),
        link_group: None,
        kind,
        lane,
        start: Tick(start),
        duration: Tick(dur),
        source_in: Tick(source_in),
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

/// A waveform on V1 over 0..8 s hearing track `track`; a music file on A1 at 2..6 s from its 1 s.
fn project(track: f64) -> Project {
    kt_plugins_builtin::install();
    let mut p = Project::new(settings());
    let wave = GeneratorKind::plugin_with("waveform", &[("track", track)]).expect("the waveform kind");
    p.media.push(MediaRef::new(MediaId(1), gen_kind_path(&wave)));
    p.media.push(MediaRef::new(MediaId(2), "t3715-music.wav"));
    let mut g = clip(1, ClipKind::Video, 0, 1, 0, 8 * SEC, 0);
    g.r#gen = Some(wave);
    p.clips.push(g);
    p.clips.push(clip(2, ClipKind::Audio, 0, 2, 2 * SEC, 4 * SEC, SEC));
    p
}

fn feed(p: &Project) -> GenFeed {
    kt_engine::gen_sources(p)
        .into_iter()
        .find(|(a, _)| *a == kt_engine::gen_source_addr(ClipId(1)))
        .map(|(_, s)| s.feed)
        .expect("the waveform's source")
}

/// Test 3715 (gen-audio-reactive): a waveform hearing audio track 1 gets the music event's piece of
/// series, in its own material time (2..6 s, heard at the file's 1..5 s); no series known — nothing;
/// moving the event moves the piece; another track hears nothing; the feed's key moves with it.
#[test]
fn t3715_the_feed_follows_the_track_it_hears() {
    let p = project(1.0);
    assert!(feed(&p).0.is_empty(), "no series known yet: silence");
    let bytes = vec![7u8; kt_plugin_api::audio_series::REC * 400];
    kt_engine::listen::put("t3715-music.wav", Arc::new(AudioSeries::from_bytes(bytes).unwrap()));
    let f = feed(&p);
    assert_eq!(f.0.len(), 1, "{f:?}");
    assert_eq!((f.0[0].from, f.0[0].to, f.0[0].offset), (Tick(2 * SEC), Tick(6 * SEC), -SEC));
    let (_, t) = f.at(Tick(3 * SEC)).expect("heard at 3 s");
    assert_eq!(t, 2 * SEC, "at 3 s the file plays its 2 s");
    assert!(f.at(Tick(SEC)).is_none() && f.at(Tick(7 * SEC)).is_none(), "outside the event: silence");

    let mut moved = p.clone();
    moved.clips[1].start = Tick(3 * SEC);
    let m = feed(&moved);
    assert_eq!((m.0[0].from, m.0[0].offset), (Tick(3 * SEC), -2 * SEC), "the piece follows the event");
    assert_ne!(m.key(), f.key(), "the key moves with it");
    assert!(feed(&project(2.0)).0.is_empty(), "track 2 hears nothing");
}
