//! gen-audio-reactive (`v26.10` G5, the audio-reactive group): waveform, spectrum, level rings draw
//! what their feed hears. Test t3716; the contract — `openspec/specs/generators/spec.md`.

use std::sync::Arc;

use kt_media::generator::GeneratorSource;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, Quality};
use kt_model::{FeedPiece, GenFeed, GenState, GeneratorKind};
use kt_time::Tick;

const W: u32 = 160;
const H: u32 = 90;
const SEC: i64 = 90_000;

fn frame(st: &GenState, t: i64) -> Vec<u8> {
    let mut src = GeneratorSource::from_state(st.clone());
    src.read_video_frame(Tick(t), Some(Quality { width: W, height: H })).expect("a frame").rgba.to_vec()
}

fn lit(f: &[u8]) -> usize {
    f.chunks(4).filter(|p| p[3] > 128).count()
}

/// Test 3716 (gen-audio-reactive): each reactive kind hearing a 1 kHz sine for 0..2 s of its material
/// draws differently while it sounds and after (silence); the same moment twice is the same bytes;
/// the spectrum's lit bars and the rings grow with the sound; the waveform moves with it.
#[test]
fn t3716_a_reactive_frame_follows_the_sound_and_only_it() {
    if kt_gpu::testkit::no_gpu(kt_plugins_builtin::install) {
        return;
    }
    let mut src = SyntheticSource::new(Tick(2 * SEC), AudioPattern::Sine { hz: 1000.0 });
    let series = Arc::new(kt_media::listen::analyze_until(&mut src, &|| false).expect("a series"));
    let feed = GenFeed(vec![FeedPiece { from: Tick(0), to: Tick(4 * SEC), offset: 0, series }]);
    for tag in ["waveform", "spectrum", "levelrings"] {
        let mut st = GenState::new(GeneratorKind::default_for_tag(tag).expect(tag));
        st.feed = feed.clone();
        let (loud, quiet) = (frame(&st, SEC), frame(&st, 3 * SEC));
        assert_ne!(loud, quiet, "{tag}: the sound changes nothing");
        assert_eq!(frame(&st, SEC), loud, "{tag}: one moment, two pictures");
        assert!(lit(&quiet) < lit(&loud) || tag == "waveform", "{tag}: silence lights more than sound ({} vs {})", lit(&quiet), lit(&loud));
        let deaf = GenState::new(st.kind.clone());
        assert_eq!(frame(&deaf, SEC), quiet, "{tag}: hearing nothing is the silent picture");
    }
}
