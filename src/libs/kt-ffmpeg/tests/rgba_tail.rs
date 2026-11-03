//! t1450 (export-throughput D1, capability media-index): the decode's RGBA conversion may write
//! past the frame — libswscale's x86 yuv2rgb kernels convert in pixel blocks and overrun the last
//! row when the width is not a multiple of the block. The showcase export (a 1080×1080 source at
//! its own size) died of it with `malloc(): corrupted top size`. Every decoded frame carries
//! `RGBA_TAIL_PAD` spare bytes; this sweep through the system FFmpeg holds the overrun under it.

#[test]
fn t1450_rgba_conversion_overrun_fits_the_tail_pad() {
    if kt_ffmpeg::backend_info().is_none() {
        eprintln!("t1450: no system FFmpeg, skipped");
        return;
    }
    let mut worst = (0usize, (0u32, 0u32), (0u32, 0u32));
    // the owner's size first, then widths around every block size, unscaled and scaled
    let mut cases = vec![((1080, 1080), (1080, 1080)), ((1920, 1080), (1080, 608))];
    for w in (2..=130u32).step_by(2) {
        cases.push(((w, 36), (w, 36)));
        cases.push(((w * 3, 72), (w, 36)));
    }
    for (src, dst) in cases {
        let over = kt_ffmpeg::rgba_conversion_overrun(src, dst).expect("FFmpeg loaded").expect("conversion");
        if over > worst.0 {
            worst = (over, src, dst);
        }
        assert!(
            over <= kt_ffmpeg::RGBA_TAIL_PAD,
            "{src:?} → {dst:?}: the conversion writes {over} bytes past the frame, the pad is {}",
            kt_ffmpeg::RGBA_TAIL_PAD
        );
    }
    eprintln!("t1450: worst overrun {} B ({:?} → {:?})", worst.0, worst.1, worst.2);
}
