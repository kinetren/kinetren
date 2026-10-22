// debug: which key falls
fn main() {
    let path = std::env::args().nth(1).unwrap();
    let mut src = kt_media::ffmpeg::FfmpegSource::open(&path).unwrap();
    use kt_media::MediaSource;
    let step = 90_000 / 30;
    for i in 0..60i64 {
        let k = kt_time::Tick(i * step);
        match src.read_video_frame(k, Some(kt_media::Quality { width: 160, height: 90 })) {
            Ok(f) => {
                if i % 20 == 0 {
                    eprintln!("key {i}: pts {:?} ok", f.pts);
                }
            }
            Err(e) => {
                eprintln!("key {i}: FAIL {e}");
                std::process::exit(1);
            }
        }
    }
    eprintln!("all ok");
}
