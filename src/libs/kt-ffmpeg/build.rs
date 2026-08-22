//! Генерация ТОЛЬКО типов и констант FFmpeg для мажоров 6/7/8 из
//! вендоренных заголовков (vendor/ffmpeg-N/include).
//!
//! Функции НЕ генерируем: их адреса резолвятся в рантайме через dlsym
//! (см. src/api.rs) — поэтому бинарь не линкуется с libav* и работает
//! с той версией FFmpeg, что стоит в системе.

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

    for major in MAJORS {
        let inc = root.join(format!("vendor/ffmpeg-{major}/include"));
        println!("cargo:rerun-if-changed={}", inc.display());

        let mut builder = bindgen::Builder::default()
            .clang_arg(format!("-I{}", inc.display()))
            // типы и константы: всё из libav*/libsw*, без системного мусора
            .allowlist_type("AV.*")
            .allowlist_type("Sws.*")
            .allowlist_type("Swr.*")
            .allowlist_var("AV_.*")
            .allowlist_var("AVERROR.*")
            .allowlist_var("SWS_.*")
            .allowlist_var("SWR_.*")
            .allowlist_var("FF_.*")
            .allowlist_var("LIB.*_VERSION_MAJOR")
            // функции не нужны — вызываем через vtable
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
