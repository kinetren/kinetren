//! The built-in plugin set (plugin-api-stage1, D6, D8): ONE closed, ordered list of ids, and the
//! registry assembled from the family crates. The references (goldens, parity cycles) iterate this
//! list and nothing else: a plugin registered through another door never appears in them.

use std::sync::Arc;

use kt_plugin_api::Registry;

/// The built-in plugin ids, in a fixed order. It grows by one stage at a time (the pilot first).
pub const BUILTIN_IDS: &[&str] =
    &[
    "color_grade", "duotone", "posterize", "vignette", "filter", "eq", "distortion", "bitcrusher", "stereo_width", "gain", "gate", "limiter", "compressor",
    "tremolo", "auto_pan", "vibrato", "chorus", "flanger", "phaser", "pitch_shift", "delay",
    "lens_distort", "twirl", "ripple", "kaleidoscope", "cylinder3d", "corner_pin", "mirror", "event_grid",
];

/// The registry of the built-in set; panics on a refusal or when the families do not give exactly
/// `BUILTIN_IDS` in that order (a built-in is never silently missing).
pub fn builtin() -> Registry {
    let mut reg = Registry::new();
    let families = [
        fx_video_color::plugins(),
        fx_audio_tone::plugins(),
        fx_audio_dynamics::plugins(),
        fx_audio_modulation::plugins(),
        fx_video_distort::plugins(),
    ];
    for p in families.into_iter().flatten() {
        let id = p.manifest().id.to_string();
        if let Err(e) = reg.register(p) {
            panic!("built-in plugin {id}: {e}");
        }
    }
    let ids: Vec<String> = reg.ids().iter().map(|s| s.to_string()).collect();
    assert_eq!(ids, BUILTIN_IDS, "the families give another set than BUILTIN_IDS");
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
