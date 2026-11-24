//! gen-audio-reactive: the audio-reactive series of a file.

use kt_media::index::MediaStore;
use kt_media::listen::{self, SeriesStore};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::MediaSource;
use kt_plugin_api::audio_series::{self, at, REC};
use kt_time::Tick;

fn sine(secs: i64) -> Box<dyn MediaSource> {
    Box::new(SyntheticSource::new(Tick(secs * 90_000), AudioPattern::Sine { hz: 1000.0 }))
}

/// Test 3714 (gen-audio-reactive): a file's series is a pure function of its sound — one record per
/// burst, the same bytes computed twice; a 0.5 sine reads its level and a held level; the cache folder keeps
/// it by the file and a changed file loses it; a stop asked keeps nothing.
#[test]
fn t3714_the_series_is_pure_and_kept_by_the_file() {
    let never = || false;
    let a = listen::analyze_until(sine(2).as_mut(), &never).expect("a series");
    let b = listen::analyze_until(sine(2).as_mut(), &never).expect("a series");
    assert_eq!(a.bursts(), (2 * 48_000usize).div_ceil(1024), "one record per burst");
    assert_eq!(a.bytes().len(), a.bursts() * REC);
    assert_eq!(a, b, "computed twice, the same bytes");
    let mut v = [0f32; at::LEN];
    audio_series::frame_values(&a, 45_000, &mut v);
    // the RMS of a 0.5 sine is −9 dBFS: (60 − 9) / 60 of the scale
    assert!((v[at::LEVEL] - 0.85).abs() < 0.01, "level {}", v[at::LEVEL]);
    assert!(v[at::LEVEL + 3] >= v[at::LEVEL], "held never below now");
    assert!(v[at::WAVE..].iter().any(|w| w.abs() > 0.4), "the waveform carries the sine");
    audio_series::frame_values(&a, 10 * 90_000, &mut v);
    assert!(v.iter().all(|x| *x == 0.0), "past the series: silence");

    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("music.wav");
    std::fs::write(&file, b"first content").unwrap();
    let mut db = MediaStore::open(&dir.path().join("kinetren-cache")).unwrap();
    assert!(db.get(&file).is_none(), "nothing stored yet");
    let mut opened = 0;
    let s = listen::series_for(&file, Some(&mut db), || { opened += 1; Some(sine(2)) }, &never).unwrap();
    assert_eq!((s.clone(), opened), (a.clone(), 1));
    let again = listen::series_for(&file, Some(&mut db), || panic!("a stored series opens nothing"), &never).unwrap();
    assert_eq!(again.hash(), a.hash(), "the store gives the same series");
    std::fs::write(&file, b"another content, longer").unwrap();
    assert!(db.get(&file).is_none(), "a changed file has no series");
    assert!(listen::analyze_until(sine(2).as_mut(), &|| true).is_none(), "stopped: nothing");
}
