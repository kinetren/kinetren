// Measurement of the video path on a real file: a span of 60 frames of preview quality.
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let src0 = std::time::Instant::now();
    let mut src = kt_media::ffmpeg::FfmpegSource::open(&path).unwrap();
    eprintln!("open: {:?}", src0.elapsed());
    use kt_media::MediaSource;
    let step = 90_000 / 30;
    let q = Some(kt_media::Quality { width: 640, height: 360 });

    // span of 60 frames (2 s of the timeline)
    let keys: Vec<kt_time::Tick> = (0..60).map(|i| kt_time::Tick(i * step)).collect();
    let t = std::time::Instant::now();
    let mut n = 0;
    src.read_video_span(&keys, q, &mut |_, _| n += 1).unwrap();
    let el = t.elapsed();
    let (reopens, decoded) = src.decode_debug_stats();
    eprintln!(
        "span 60@640x360: {:?} ({:.1}x realtime), frames {n}, reopens {reopens}, decoded {decoded}",
        el,
        2.0 / el.as_secs_f64()
    );

    // a jump back to the middle plus one more span (the scenario “selected a region earlier”)
    let keys2: Vec<kt_time::Tick> = (30..90).map(|i| kt_time::Tick(i * step)).collect();
    let t = std::time::Instant::now();
    let mut n2 = 0;
    src.read_video_span(&keys2, q, &mut |_, _| n2 += 1).unwrap();
    eprintln!(
        "span back+60: {:?} ({:.1}x realtime), frames {n2}",
        t.elapsed(),
        2.0 / t.elapsed().as_secs_f64()
    );
}
