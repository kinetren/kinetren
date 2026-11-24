//! t4111 (durable-moment-key D1): what a source answers for its identity.

use kt_media::generator::{GeneratorSource, gen_rev};
use kt_media::placeholder::PlaceholderSource;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_media::{MediaSource, SourceIdent};
use kt_model::{FeedPiece, GenFeed, GenState, GeneratorKind};
use kt_time::Tick;

/// t4111: a generator's identity is its values and what it hears, it moves with them and returns with
/// them, and it is the renderer's `src_rev`; a synthetic source's is its parameters; a placeholder has
/// none, and a number of the process is no one's twice.
#[test]
fn t4111_what_a_source_answers_for_its_identity() {
    kt_plugins_builtin::install();
    let a = GeneratorKind::solid(1, 2, 3);
    let mut g = GeneratorSource::new(a.clone());
    let first = g.identity().unwrap();
    assert!(first.durable);
    assert_eq!(first.key, gen_rev(&Some(a.clone()), &Vec::new(), 0), "the renderer's src_rev of the event");
    assert_eq!(GeneratorSource::new(a.clone()).identity(), Some(first), "another source of the same values");
    assert!(g.retune_gen(&GenState::new(GeneratorKind::solid(9, 2, 3))));
    assert_ne!(g.identity(), Some(first));
    assert!(g.retune_gen(&GenState::new(a.clone())));
    assert_eq!(g.identity(), Some(first), "back with its values");
    // what it hears
    let series = |n: u8| {
        let bytes = vec![n; kt_plugin_api::audio_series::REC * 4];
        std::sync::Arc::new(kt_plugin_api::audio_series::AudioSeries::from_bytes(bytes).unwrap())
    };
    let hears = |n: u8| GenState {
        kind: a.clone(),
        auto: Vec::new(),
        feed: GenFeed(vec![FeedPiece { from: Tick(0), to: Tick(90_000), offset: 0, series: series(n) }]),
    };
    assert!(g.retune_gen(&hears(1)));
    let heard = g.identity();
    assert_ne!(heard, Some(first));
    assert!(g.retune_gen(&hears(2)));
    assert_ne!(g.identity(), heard, "another series");

    let synth = |s: i64, p| SyntheticSource::new(Tick(s * 90_000), p).identity();
    assert_eq!(synth(2, AudioPattern::Ramp), synth(2, AudioPattern::Ramp));
    assert_ne!(synth(2, AudioPattern::Ramp), synth(3, AudioPattern::Ramp));
    assert_ne!(synth(2, AudioPattern::Ramp), synth(2, AudioPattern::Silence));

    assert_eq!(PlaceholderSource::new("gone.mp4").identity(), None);
    let (x, y) = (SourceIdent::once(), SourceIdent::once());
    assert!(x != y && !x.durable && !y.durable);
}

/// The file half: a file opened twice agrees; a copy under another name and the file touched and opened
/// again differ; a file removed after it was opened keeps its answer.
#[cfg(feature = "ffmpeg")]
#[test]
fn t4111_a_file_is_its_name_size_and_time_as_it_was_opened() {
    let Ok(st) = std::process::Command::new("bash")
        .arg(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/gen.sh"))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
    else {
        return;
    };
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/out/muxed_av.mp4");
    if !st.success() || !fixture.exists() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("a.mp4");
    std::fs::copy(&fixture, &file).unwrap();
    let open = |p: &std::path::Path| kt_media::ffmpeg::FfmpegSource::open(&p.to_string_lossy()).unwrap();
    let first = open(&file);
    let ident = first.identity();
    assert!(ident.is_some_and(|i| i.durable));
    assert_eq!(open(&file).identity(), ident);
    // the project's folder moved: the same file, the same name
    let moved = dir.path().join("elsewhere");
    std::fs::create_dir(&moved).unwrap();
    let there = moved.join("a.mp4");
    std::fs::copy(&file, &there).unwrap();
    let t = std::fs::metadata(&file).unwrap().modified().unwrap();
    std::fs::File::options().write(true).open(&there).unwrap().set_modified(t).unwrap();
    assert_eq!(open(&there).identity(), ident, "a moved folder keeps it");
    let copy = dir.path().join("b.mp4");
    std::fs::copy(&file, &copy).unwrap();
    std::fs::File::options().write(true).open(&copy).unwrap().set_modified(t).unwrap();
    assert_ne!(open(&copy).identity(), ident, "another name");
    std::fs::File::options().write(true).open(&file).unwrap().set_modified(t - std::time::Duration::from_secs(60)).unwrap();
    assert_ne!(open(&file).identity(), ident, "touched and opened again");
    assert_eq!(first.identity(), ident, "the one opened before still decodes what it opened");
    std::fs::remove_file(&file).unwrap();
    assert_eq!(first.identity(), ident);

    // the file is replaced under its path: a key behind the decoder would open a second one by the path,
    // and that is another file's (found-defects 1065)
    let long = fixture.with_file_name("testsrc_12s.mp4");
    let b = dir.path().join("b.mp4");
    std::fs::copy(&long, &b).unwrap();
    let q = Some(kt_media::Quality { width: 80, height: 44 });
    let (mut src, mut old) = (open(&b), open(&long));
    src.read_video_frame(Tick(9 * 90_000), q).unwrap();
    std::fs::copy(&fixture, dir.path().join("new.mp4")).unwrap();
    std::fs::rename(dir.path().join("new.mp4"), &b).unwrap();
    let t = Tick(90_000);
    assert_eq!(src.read_video_frame(t, q).unwrap().rgba, old.read_video_frame(t, q).unwrap().rgba, "the new file's frame");
    assert_eq!(src.decoders(), 1, "a decoder of the new file was kept");
}
