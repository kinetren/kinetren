//! Generates ONLY the types and constants of FFmpeg for majors 6/7/8 from the
//! vendored headers (vendor/ffmpeg-N/include).
//!
//! Functions are NOT generated: their addresses are resolved at runtime via dlsym
//! (see src/api.rs) — therefore the binary does not link to libav* and works
//! with the FFmpeg version installed on the system.

use std::path::PathBuf;

const MAJORS: [&str; 3] = ["6", "7", "8"];

const HEADERS: [&str; 8] = [
    "libavutil/avutil.h",
    "libavutil/channel_layout.h",
    "libavutil/imgutils.h",
    "libavutil/opt.h",
    "libavcodec/avcodec.h",
    "libavformat/avformat.h",
    "libswscale/swscale.h",
    "libswresample/swresample.h",
];

fn main() {
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    // The layouts are the TARGET's, never the host's (windows-build): on Windows `long` is
    // 32-bit and a cross-build runs this script on Linux. Decisions here read cargo's
    // TARGET, not `cfg!(...)`, which describes the machine running the build script.
    let target = std::env::var("TARGET").unwrap();

    for major in MAJORS {
        let inc = root.join(format!("vendor/ffmpeg-{major}/include"));
        println!("cargo:rerun-if-changed={}", inc.display());

        let mut builder = bindgen::Builder::default()
            .clang_arg(format!("--target={target}"))
            .clang_arg(format!("-I{}", inc.display()))
            // types and constants: everything from libav*/libsw*, without system clutter
            .allowlist_type("AV.*")
            .allowlist_type("Sws.*")
            .allowlist_type("Swr.*")
            .allowlist_var("AV_.*")
            .allowlist_var("AVERROR.*")
            .allowlist_var("SWS_.*")
            .allowlist_var("SWR_.*")
            .allowlist_var("FF_.*")
            .allowlist_var("LIB.*_VERSION_MAJOR")
            // functions are not needed — we call them through a vtable
            .ignore_functions()
            .layout_tests(false)
            .derive_default(false)
            .derive_debug(false)
            .prepend_enum_name(false)
            .default_enum_style(bindgen::EnumVariation::Consts)
            .generate_comments(false)
            .use_core();

        for h in HEADERS {
            builder = builder.header(inc.join(h).to_string_lossy().to_string());
        }

        let bindings = builder.generate().unwrap_or_else(|e| panic!("bindgen ffmpeg-{major}: {e}"));
        bindings.write_to_file(out.join(format!("types_v{major}.rs"))).expect("write bindings");
    }
}
