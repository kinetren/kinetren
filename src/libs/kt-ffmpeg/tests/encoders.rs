//! t858 (windows-build, capability export): the encoder cascade and its settings per OS.

use kt_ffmpeg::encoders::{HwKind, PixFmt, video_encoder_candidates};
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
            // hw-encode-context D2: VAAPI surfaces are uploaded from NV12 too
            let want = if e.name == "h264_qsv" || e.name == "h264_vaapi" { PixFmt::Nv12 } else { PixFmt::Yuv420p };
            assert_eq!(e.pix, want, "{os:?} {}", e.name);
            if e.name != "mpeg4" {
                assert!(!e.quality_options(23).is_empty(), "{os:?} {} has a quality setting", e.name);
            }
        }
    }
    let mf = video_encoder_candidates(Os::Windows).into_iter().find(|e| e.name == "h264_mf").unwrap();
    assert!(mf.quality_options(23).contains(&("quality", "54".to_string())), "the CRF scale maps to MF's 0..100");
}

/// t960 (hw-encode-context D2): each candidate declares the context it needs. `h264_vaapi` needs a
/// VAAPI device and a frames context of the export's (even) size, uploaded from NV12 — without
/// them it refused every open (−22), so Intel and AMD on Linux always fell back to libx264.
#[test]
fn t960_each_candidate_declares_its_context() {
    for os in [Os::Linux, Os::Windows] {
        for e in video_encoder_candidates(os) {
            if e.name == "h264_vaapi" {
                assert_eq!((e.ctx, e.pix, e.hardware), (HwKind::Vaapi, PixFmt::Nv12, true));
                assert_eq!(e.frames_size(1281, 721), Some((1280, 720)), "the frames context is the encoder's size");
                assert_eq!(e.quality_options(23), vec![("qp", "23".to_string())], "VAAPI's knob is QP");
                assert!(
                    e.bitrate_options(8_000_000).contains(&("rc_mode", "VBR".to_string())),
                    "the bitrate mode names a rate control"
                );
            } else {
                // QSV keeps system NV12 frames (design D2, unless the stage-1 measurement revises it)
                assert_eq!(e.ctx, HwKind::None, "{os:?} {} needs no device context", e.name);
                assert_eq!(e.frames_size(1280, 720), None, "{os:?} {}", e.name);
            }
        }
    }
}

/// The values of a C enum in a vendored header: `name` → its value, counted the way C does.
fn header_enum(header: &str, enum_name: &str) -> std::collections::HashMap<String, i64> {
    let text = std::fs::read_to_string(header).unwrap();
    let start = text.find(&format!("enum {enum_name} {{")).expect("enum in header");
    let body = &text[start..];
    let body = &body[body.find('{').unwrap() + 1..body.find("};").unwrap()];
    // comments out, preprocessor lines out
    let mut clean = String::new();
    let mut rest = body;
    while let Some(i) = rest.find("/*") {
        clean.push_str(&rest[..i]);
        rest = &rest[i + rest[i..].find("*/").unwrap() + 2..];
    }
    clean.push_str(rest);
    let clean: String = clean
        .lines()
        .map(|l| l.split("//").next().unwrap())
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let mut out = std::collections::HashMap::new();
    let mut next = 0i64;
    for item in clean.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        let (name, value) = match item.split_once('=') {
            Some((n, v)) => (n.trim(), v.trim().parse::<i64>().ok()),
            None => (item, None),
        };
        let v = value.unwrap_or(next);
        out.insert(name.to_string(), v);
        next = v + 1;
    }
    out
}

/// t961 (hw-encode-context D5, media-index): the hardware functions are an OPTIONAL table — a
/// resolver that lacks one gives no table and names the function, and the struct layouts come
/// from each major's own vendored `hwcontext.h`. Needs no FFmpeg.
///
/// Before, every symbol in `api.rs` was required: one more name would have turned a machine with
/// a trimmed FFmpeg into "FFmpeg not found".
#[test]
fn t961_hardware_functions_are_an_optional_table() {
    unsafe extern "C" fn dummy() {}
    let all = |_: &str| Some(dummy as *mut core::ffi::c_void);
    let lacking = |n: &str| (n != "av_hwframe_transfer_data").then_some(dummy as *mut core::ffi::c_void);
    macro_rules! major {
        ($m:ident, $dir:literal) => {{
            use kt_ffmpeg::$m::{HwApi, ffi};
            assert!(unsafe { HwApi::load_with(all) }.is_ok(), "{}: a full resolver gives the table", $dir);
            assert_eq!(
                unsafe { HwApi::load_with(lacking) }.err().as_deref(),
                Some("av_hwframe_transfer_data"),
                "{}: the missing function is named",
                $dir
            );
            let inc = concat!(env!("CARGO_MANIFEST_DIR"), "/vendor/", $dir, "/include/libavutil/");
            let dev = header_enum(&format!("{inc}hwcontext.h"), "AVHWDeviceType");
            let pix = header_enum(&format!("{inc}pixfmt.h"), "AVPixelFormat");
            assert_eq!(ffi::AV_HWDEVICE_TYPE_VAAPI as i64, dev["AV_HWDEVICE_TYPE_VAAPI"], "{}", $dir);
            assert_eq!(ffi::AV_PIX_FMT_VAAPI as i64, pix["AV_PIX_FMT_VAAPI"], "{}", $dir);
            assert_eq!(ffi::AV_PIX_FMT_NV12 as i64, pix["AV_PIX_FMT_NV12"], "{}", $dir);
            assert!(core::mem::size_of::<ffi::AVHWFramesContext>() > 0, "{}", $dir);
            HwApi::NAMES.to_vec()
        }};
    }
    let names = [major!(v6, "ffmpeg-6"), major!(v7, "ffmpeg-7"), major!(v8, "ffmpeg-8"), major!(v9, "ffmpeg-9")];
    // one table for every major, and none of its names is in the required one
    assert!(names.iter().all(|n| n == &names[0]), "the majors differ: {names:?}");
    assert_eq!(names[0].len(), 7, "{:?}", names[0]);
    assert!(names[0].iter().all(|n| n.starts_with("av_hw") || n.starts_with("av_buffer_")), "{:?}", names[0]);
}
