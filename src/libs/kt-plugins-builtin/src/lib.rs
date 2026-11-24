//! The built-in plugin set (plugin-api-stage1, D6, D8): ONE closed, ordered list of ids, and the
//! registry assembled from the family crates. The references (goldens, parity cycles) iterate this
//! list and nothing else: a plugin registered through another door never appears in them.

use std::sync::Arc;

use kt_plugin_api::Registry;

/// The built-in plugin ids, in a fixed order. It grows by one stage at a time (the pilot first).
pub const BUILTIN_IDS: &[&str] =
    &[
    "color_grade", "duotone", "posterize", "vignette", "greenscreen", "filter", "eq", "distortion", "bitcrusher", "stereo_width", "gain", "gate", "limiter", "compressor",
    "tremolo", "auto_pan", "vibrato", "chorus", "flanger", "phaser", "pitch_shift", "delay", "slowed_reverb",
    "lens_distort", "twirl", "ripple", "kaleidoscope", "cylinder3d", "corner_pin", "mirror", "event_grid", "pan3d",
    "pixelate", "edges", "halftone", "chromatic_aberration", "vhs", "noise",
    "blur", "glow", "sharpen", "radial_blur", "drop_shadow", "datamosh", "datamosh_sound",
    "camera_isp", "camera_isp_sound",
    "vhs_tape", "vhs_tape_sound",
    "wipe", "iris", "clock", "barn", "luma", "push", "slide", "zoom", "spin", "flash", "dip", "blinds", "checker",
    "blocks", "spiral", "split", "squeeze", "stretch",
    "whip", "blur_dissolve", "pixelate", "glitch", "shake", "film_burn", "light_leaks", "zoom_blur",
    "swap", "flip", "cube", "door", "page_peel", "page_roll",
    "solid", "gradient", "corners", "checker", "noise", "transparent", "rect", "ellipse", "line",
    "polygon", "star", "arrow", "frame", "plate", "progress",
    "meshgrad", "plasma", "particles", "pattern",
    "smpte", "timecode", "leader", "table", "window", "syncflash", "safearea",
    "tone", "audio_noise", "sweep", "beep", "silence",
    "waveform", "spectrum", "levelrings",
];

// the source hash of each family crate (build.rs): `SOURCES`, by the crate's name
include!(concat!(env!("OUT_DIR"), "/sources.rs"));

fn source_of(family: &str) -> u64 {
    SOURCES.iter().find(|(n, _)| *n == family).map(|(_, h)| *h).expect("a family crate build.rs does not know")
}

/// The registry of the built-in set; panics on a refusal or when the families do not give exactly
/// `BUILTIN_IDS` in that order (a built-in is never silently missing).
pub fn builtin() -> Registry {
    let mut reg = Registry::new();
    let families = [
        ("fx-video-color", fx_video_color::plugins()),
        ("fx-audio-tone", fx_audio_tone::plugins()),
        ("fx-audio-dynamics", fx_audio_dynamics::plugins()),
        ("fx-audio-modulation", fx_audio_modulation::plugins()),
        ("fx-video-distort", fx_video_distort::plugins()),
        ("fx-video-stylize", fx_video_stylize::plugins()),
        ("fx-video-blur", fx_video_blur::plugins()),
        ("fx-video-codec", fx_video_codec::plugins()),
        ("fx-video-camera", fx_video_camera::plugins()),
        ("fx-video-tape", fx_video_tape::plugins()),
        ("transitions-basic", transitions_basic::plugins()),
        ("transitions-styled", transitions_styled::plugins()),
        ("transitions-volume", transitions_volume::plugins()),
        ("generators-basic", generators_basic::plugins()),
        ("generators-test", generators_test::plugins()),
        ("generators-audio", generators_audio::plugins()),
        ("generators-reactive", generators_reactive::plugins()),
    ];
    for (crate_name, plugins) in families {
        let source = source_of(crate_name);
        for p in plugins {
            let (id, family) = (p.manifest().id.to_string(), p.manifest().media.family());
            if let Err(e) = reg.register(p) {
                panic!("built-in plugin {id}: {e}");
            }
            reg.set_source(family, &id, source);
        }
    }
    let ids: Vec<String> = reg.ids().iter().map(|s| s.to_string()).collect();
    assert_eq!(ids, BUILTIN_IDS, "the families give another set than BUILTIN_IDS");
    // a simulated device's sound is never silently missing (sim-audio-pairs)
    assert!(reg.unpaired().is_empty(), "a declared pair is not registered: {:?}", reg.unpaired());
    reg
}

/// Assemble and install the built-in set for this process (kt-app, kt-cli and tests call it once
/// at start; a repeated call is a no-op).
pub fn install() -> &'static Arc<Registry> {
    install_with(|_| {})
}

/// The built-in set plus what `extra` registers (an app's test door, D8), installed once.
pub fn install_with(extra: impl FnOnce(&mut Registry)) -> &'static Arc<Registry> {
    if kt_plugin_api::installed().is_none() {
        let mut reg = builtin();
        extra(&mut reg);
        kt_plugin_api::install(Arc::new(reg));
    }
    kt_plugin_api::installed().expect("installed")
}
