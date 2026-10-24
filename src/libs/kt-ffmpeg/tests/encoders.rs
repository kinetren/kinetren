//! t858 (windows-build, capability export): the encoder cascade and its settings per OS.

use kt_ffmpeg::encoders::{PixFmt, video_encoder_candidates};
use kt_ffmpeg::search::Os;

#[test]
fn t858_encoder_cascade_per_os() {
    let names = |os| video_encoder_candidates(os).iter().map(|e| e.name).collect::<Vec<_>>();
    assert_eq!(names(Os::Linux), ["h264_nvenc", "h264_vaapi", "h264_qsv", "libx264", "mpeg4"]);
    assert_eq!(
        names(Os::Windows),
        ["h264_nvenc", "h264_amf", "h264_qsv", "libx264", "h264_mf", "mpeg4"],
        "AMD and Media Foundation on Windows; Media Foundation after libx264"
    );
    for os in [Os::Linux, Os::Windows] {
        for e in video_encoder_candidates(os) {
            let want = if e.name == "h264_qsv" { PixFmt::Nv12 } else { PixFmt::Yuv420p };
            assert_eq!(e.pix, want, "{os:?} {}", e.name);
            if e.name != "mpeg4" {
                assert!(!e.quality_options(23).is_empty(), "{os:?} {} has a quality setting", e.name);
            }
        }
    }
    let mf = video_encoder_candidates(Os::Windows).into_iter().find(|e| e.name == "h264_mf").unwrap();
    assert!(mf.quality_options(23).contains(&("quality", "54".to_string())), "the CRF scale maps to MF's 0..100");
}
