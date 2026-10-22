//! Single FX registry + popup picker with search (item 3 of v1.4): one place to choose effects
//! for all four add points (clip FX editor ×2 kinds, morpher
//! FX batch ×2 chains), scalable to hundreds of effects —
//! a search row (name + keywords, case-insensitive) and grouping by categories. The templates are the same
//! as were in the buttons/combo (test 48).

use crate::fx_editor::video_fx_templates;
use crate::i18n::{Key, k, tr, tr_en};
use eframe::egui;
use kt_model::FxInstance;

pub struct FxEntry {
    /// The effect name is NOT translated: it is an industry term, by it the effect is searched
    /// in any locale (and it is checked against `fx_display_name`, test 48).
    pub name: &'static str,
    pub category: Key,
    /// Which picker to offer it in. Usually exactly one of the two; the only two-sided one is the time warp
    /// (R.37): it is not a pixel filter but a rule for choosing
    /// the source frame, and it makes sense on both kinds.
    pub audio: bool,
    pub video: bool,
    /// Search keywords. A catalog key, not text: the registry does not depend on the language
    /// and `OnceLock` is not rebuilt on switching.
    pub keywords: Option<Key>,
    pub template: FxInstance,
    /// Built-in VARIANTS (R.54). Empty for audio entries: a thumbnail of sound cannot be drawn,
    /// and a strip without pictures would be a second list of names.
    pub variants: Vec<FxVariant>,
}

/// A built-in effect variant (R.54): the same kind, brought to
/// a meaningful state, with a name by KIND, not by parameter.
///
/// A variant is exactly an `FxInstance`, not a separate entity: a click on its thumbnail goes by the same path
/// `Picked::Fx` as a click on the effect name, so no new command and no new timeline entity appears (t252/t253).
/// The library of user presets (R.48) is something else:
/// there is a CHAIN block, it does not belong to an effect kind
/// and lives in a file.
///
/// The variant’s name IS translated, unlike the effect
/// name: “Ink”, “Blueprint”, “Large” are not industry terms.
/// A double policy in one list is a deliberate price, analysed in F-HLP-031 (“Cons”).
pub struct FxVariant {
    pub name: Key,
    pub fx: FxInstance,
}

/// The category and keywords of a video effect by name from video_fx_templates.
fn video_meta(name: &str) -> (Key, Option<Key>) {
    // Full paths are deliberate: test 125 looks up the catalog key by the path `k::…`
    // and a shortened `use` would hide the usage.
    match name {
        "Geometry" => (k::fxreg::cat::MOTION, Some(k::fxreg::kw::GEOMETRY)),
        "Mirror" => (k::fxreg::cat::GEOMETRY, Some(k::fxreg::kw::MIRROR)),
        "Blur" => (k::fxreg::cat::BLUR, Some(k::fxreg::kw::BLUR)),
        "Greenscreen" => (k::fxreg::cat::KEY, Some(k::fxreg::kw::GREENSCREEN)),
        "Edges" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::EDGES)),
        "Noise" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::NOISE)),
        "VHS" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::VHS)),
        "Glow" => (k::fxreg::cat::GLOW, Some(k::fxreg::kw::GLOW)),
        "EventGrid" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::EVENTGRID)),
        "Pan 3D" => (k::fxreg::cat::THREE_D, Some(k::fxreg::kw::PAN3D)),
        // R.47 Q2: color/style/perspective
        "Color Grade" => (k::fxreg::cat::COLOR, Some(k::fxreg::kw::COLOR_GRADE)),
        "Duotone" => (k::fxreg::cat::COLOR, Some(k::fxreg::kw::DUOTONE)),
        "Posterize" => (k::fxreg::cat::COLOR, Some(k::fxreg::kw::POSTERIZE)),
        "Vignette" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::VIGNETTE)),
        "Corner Pin" => (k::fxreg::cat::GEOMETRY, Some(k::fxreg::kw::CORNER_PIN)),
        // R.47 Q3: barriers
        "Sharpen" => (k::fxreg::cat::BLUR, Some(k::fxreg::kw::SHARPEN)),
        "Pixelate" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::PIXELATE)),
        "Chromatic Aberration" => {
            (k::fxreg::cat::DISTORT, Some(k::fxreg::kw::CHROMATIC_ABERRATION))
        }
        "Radial Blur" => (k::fxreg::cat::BLUR, Some(k::fxreg::kw::RADIAL_BLUR)),
        "Drop Shadow" => (k::fxreg::cat::KEY, Some(k::fxreg::kw::DROP_SHADOW)),
        "Halftone" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::HALFTONE)),
        // R.47 Q4: the Warp family and the cylinder
        "Lens Distort" => (k::fxreg::cat::DISTORT, Some(k::fxreg::kw::LENS_DISTORT)),
        "Twirl" => (k::fxreg::cat::DISTORT, Some(k::fxreg::kw::TWIRL)),
        "Ripple" => (k::fxreg::cat::DISTORT, Some(k::fxreg::kw::RIPPLE)),
        "Kaleidoscope" => (k::fxreg::cat::DISTORT, Some(k::fxreg::kw::KALEIDOSCOPE)),
        "Cylinder 3D" => (k::fxreg::cat::THREE_D, Some(k::fxreg::kw::CYLINDER3D)),
        _ => (k::fxreg::cat::VIDEO, None),
    }
}

/// A variant from a template: the values are set BY NAME via `set_param`, not as a structure literal.
/// This way a new effect parameter does not touch fifty
/// places (the kt-fx rule), and a value outside the spec
/// is impossible by construction — `set_param` clamps to the range (t543 checks this separately).
fn var(tmpl: &FxInstance, name: Key, sets: &[(&str, f64)]) -> FxVariant {
    let mut fx = tmpl.clone();
    for (p, v) in sets {
        // a typo in a parameter name would otherwise pass silently: set_param would return false,
        // and the variant would turn out to be a copy of the template
        debug_assert!(fx.param_spec(p).is_some(), "{}: no such param {p}", fx.tag());
        fx.set_param(p, *v);
    }
    FxVariant { name, fx }
}

/// Built-in variants of a video effect (R.54).
///
/// The order in the strip is from weak to strong: the first
/// cell must be NOT an identity, because for five kinds
/// (Pan, Pan 3D, Color Grade, Corner Pin, Drop Shadow) the
/// template itself is an identity, and its thumbnail shows
/// the source frame. It is precisely the first variant that serves them as an icon (t543 holds this condition).
///
/// Full paths to the keys are deliberate — the same reason as for `video_meta`: t125 looks up the catalog key
/// by path, a shortened `use` would hide it.
fn video_variants(name: &str, tmpl: &FxInstance) -> Vec<FxVariant> {
    match name {
        // R.62: the additional “Geometry” node — three variants
        // (the identity default cannot serve as an icon, t543)
        "Geometry" => vec![
            var(tmpl, k::fxreg::var::CLOSEUP, &[("zoom", 1.25)]),
            var(tmpl, k::fxreg::var::PUNCH, &[("zoom", 2.0)]),
            var(tmpl, k::fxreg::var::OFFCENTER, &[("zoom", 1.4), ("pos_x", -0.12)]),
        ],
        // the mirror has no numeric parameters at all — only two checkboxes, so there are exactly two variants:
        // the third state is the template itself
        "Mirror" => vec![
            FxVariant {
                name: k::fxreg::var::VERTICAL,
                fx: FxInstance::Mirror { horizontal: false, vertical: true },
            },
            FxVariant {
                name: k::fxreg::var::BOTH,
                fx: FxInstance::Mirror { horizontal: true, vertical: true },
            },
        ],
        "Blur" => vec![
            var(tmpl, k::fxreg::var::SOFT, &[("radius", 3.0)]),
            var(tmpl, k::fxreg::var::HEAVY, &[("radius", 24.0)]),
            // blur along one axis: radius_y = 1 (not 0 — zero means “as X”)
            var(tmpl, k::fxreg::var::HORIZONTAL, &[("radius", 28.0), ("radius_y", 1.0)]),
        ],
        "Greenscreen" => vec![
            var(tmpl, k::fxreg::var::TIGHT, &[("tolerance", 40.0), ("softness", 10.0)]),
            var(tmpl, k::fxreg::var::WIDE, &[("tolerance", 120.0), ("softness", 45.0)]),
            var(tmpl, k::fxreg::var::BLUE, &[("key_g", 0.0), ("key_b", 255.0)]),
        ],
        "Edges" => vec![
            var(
                tmpl,
                k::fxreg::var::INK,
                &[("threshold", 90.0), ("color_r", 0.0), ("color_g", 0.0), ("color_b", 0.0)],
            ),
            var(
                tmpl,
                k::fxreg::var::NEON,
                &[
                    ("threshold", 36.0),
                    ("thickness", 2.0),
                    ("color_r", 0.0),
                    ("color_g", 255.0),
                    ("color_b", 160.0),
                ],
            ),
            var(
                tmpl,
                k::fxreg::var::BLUEPRINT,
                &[("threshold", 60.0), ("color_r", 80.0), ("color_g", 160.0), ("color_b", 255.0)],
            ),
        ],
        "Noise" => vec![
            var(tmpl, k::fxreg::var::FILM, &[("amount", 0.05), ("size", 2.0)]),
            var(tmpl, k::fxreg::var::HEAVY, &[("amount", 0.25)]),
            var(tmpl, k::fxreg::var::COARSE, &[("amount", 0.18), ("size", 8.0)]),
        ],
        "VHS" => vec![
            var(
                tmpl,
                k::fxreg::var::SUBTLE,
                &[("rgb_shift", 2.0), ("bleed", 0.2), ("scan_step", 3.0), ("jitter", 0.03)],
            ),
            var(tmpl, k::fxreg::var::CLASSIC, &[("bleed", 0.5), ("jitter", 0.12)]),
            var(
                tmpl,
                k::fxreg::var::TRASH,
                &[("rgb_shift", 10.0), ("bleed", 0.85), ("scan_step", 6.0), ("jitter", 0.45)],
            ),
        ],
        "Glow" => vec![
            var(
                tmpl,
                k::fxreg::var::SOFT,
                &[("strength", 0.4), ("radius", 8.0), ("threshold", 128.0)],
            ),
            var(
                tmpl,
                k::fxreg::var::DREAM,
                &[("strength", 1.1), ("radius", 16.0), ("threshold", 96.0)],
            ),
            var(
                tmpl,
                k::fxreg::var::GOLDEN,
                &[
                    ("strength", 0.9),
                    ("radius", 12.0),
                    ("threshold", 110.0),
                    ("tint_g", 200.0),
                    ("tint_b", 120.0),
                ],
            ),
        ],
        "Pan 3D" => vec![
            var(tmpl, k::fxreg::var::TILT, &[("rot_x", 15.0)]),
            var(tmpl, k::fxreg::var::TURN, &[("rot_y", -20.0)]),
            var(tmpl, k::fxreg::var::DEPTH, &[("pos_z", -200.0), ("rot_y", 10.0), ("scale", 0.85)]),
        ],
        "EventGrid" => vec![
            var(tmpl, k::fxreg::var::GRID9, &[("cols", 3.0), ("rows", 3.0)]),
            var(tmpl, k::fxreg::var::GRID16, &[("cols", 4.0), ("rows", 4.0)]),
            var(tmpl, k::fxreg::var::STRIP, &[("cols", 4.0), ("rows", 1.0)]),
        ],
        "Color Grade" => vec![
            var(
                tmpl,
                k::fxreg::var::WARM,
                &[("hue", 8.0), ("saturation", 1.15), ("brightness", 0.04)],
            ),
            var(
                tmpl,
                k::fxreg::var::COOL,
                &[("hue", -12.0), ("saturation", 0.9), ("contrast", 0.1)],
            ),
            var(tmpl, k::fxreg::var::BW, &[("saturation", 0.0), ("contrast", 0.15)]),
        ],
        "Duotone" => vec![
            var(
                tmpl,
                k::fxreg::var::SEPIA,
                &[
                    ("shadow_r", 48.0),
                    ("shadow_g", 28.0),
                    ("shadow_b", 12.0),
                    ("high_r", 255.0),
                    ("high_g", 226.0),
                    ("high_b", 180.0),
                ],
            ),
            var(
                tmpl,
                k::fxreg::var::BLUE,
                &[
                    ("shadow_r", 8.0),
                    ("shadow_g", 16.0),
                    ("shadow_b", 64.0),
                    ("high_r", 180.0),
                    ("high_g", 220.0),
                    ("high_b", 255.0),
                ],
            ),
            var(
                tmpl,
                k::fxreg::var::NEON,
                &[
                    ("shadow_r", 32.0),
                    ("shadow_g", 0.0),
                    ("shadow_b", 64.0),
                    ("high_r", 255.0),
                    ("high_g", 64.0),
                    ("high_b", 192.0),
                ],
            ),
        ],
        "Posterize" => vec![
            var(tmpl, k::fxreg::var::HARD, &[("levels", 3.0)]),
            var(tmpl, k::fxreg::var::COMIC, &[("levels", 4.0)]),
            var(tmpl, k::fxreg::var::SOFT, &[("levels", 12.0)]),
        ],
        "Vignette" => vec![
            var(
                tmpl,
                k::fxreg::var::SOFT,
                &[("strength", 0.3), ("radius", 0.9), ("softness", 0.6)],
            ),
            var(
                tmpl,
                k::fxreg::var::STRONG,
                &[("strength", 0.9), ("radius", 0.55), ("softness", 0.35)],
            ),
            var(
                tmpl,
                k::fxreg::var::TIGHT,
                &[("strength", 0.7), ("radius", 0.4), ("softness", 0.2)],
            ),
        ],
        "Corner Pin" => vec![
            // trapezoid: the top edge is narrowed and lowered
            var(
                tmpl,
                k::fxreg::var::PERSPECTIVE,
                &[("x0", 0.12), ("y0", 0.06), ("x1", 0.88), ("y1", 0.06)],
            ),
            // the right edge is shorter than the left
            var(tmpl, k::fxreg::var::TILT, &[("y1", 0.14), ("y3", 0.86)]),
            var(tmpl, k::fxreg::var::SKEW, &[("x0", 0.18), ("x3", 0.82)]),
        ],
        "Sharpen" => vec![
            var(tmpl, k::fxreg::var::SOFT, &[("amount", 0.5), ("radius", 1.0)]),
            var(tmpl, k::fxreg::var::STRONG, &[("amount", 2.0)]),
            var(tmpl, k::fxreg::var::HEAVY, &[("amount", 3.2), ("radius", 4.0)]),
        ],
        "Pixelate" => vec![
            var(tmpl, k::fxreg::var::FINE, &[("size", 5.0)]),
            var(tmpl, k::fxreg::var::COARSE, &[("size", 28.0)]),
            var(tmpl, k::fxreg::var::CENSOR, &[("size", 64.0)]),
        ],
        "Chromatic Aberration" => vec![
            var(tmpl, k::fxreg::var::SUBTLE, &[("amount", 0.004)]),
            var(tmpl, k::fxreg::var::STRONG, &[("amount", 0.028)]),
            // NOT 0.05 (the spec ceiling): the field is f32, and the clamp in `set_param` is computed in f64 —
            // the exact ceiling when written to f32 rounds UP and
            // the value ends up OUTSIDE the spec (found by t543).
            // A variant is a kind, not a limit check
            var(tmpl, k::fxreg::var::GLITCH, &[("amount", 0.045)]),
        ],
        "Radial Blur" => {
            let mut v = vec![
                var(tmpl, k::fxreg::var::ZOOM, &[("amount", 0.5)]),
                var(tmpl, k::fxreg::var::HEAVY, &[("amount", 0.85)]),
            ];
            // rotation instead of zoom — the only non-numeric parameter among all variants:
            // `spin` has no spec, it is a bool
            let mut spin = var(tmpl, k::fxreg::var::SPIN, &[("amount", 0.4)]);
            if let FxInstance::RadialBlur { spin: s, .. } = &mut spin.fx {
                *s = true;
            }
            v.insert(1, spin);
            v
        }
        "Drop Shadow" => vec![
            var(
                tmpl,
                k::fxreg::var::SOFT,
                &[("offset_x", 6.0), ("offset_y", 6.0), ("radius", 12.0), ("opacity", 0.45)],
            ),
            var(
                tmpl,
                k::fxreg::var::HARD,
                &[("offset_x", 4.0), ("offset_y", 4.0), ("radius", 1.0), ("opacity", 0.8)],
            ),
            var(
                tmpl,
                k::fxreg::var::FAR,
                &[("offset_x", 24.0), ("offset_y", 24.0), ("radius", 16.0), ("opacity", 0.5)],
            ),
        ],
        "Halftone" => vec![
            var(tmpl, k::fxreg::var::FINE, &[("size", 3.0), ("angle", 0.0)]),
            var(tmpl, k::fxreg::var::PRINT, &[("size", 5.0), ("angle", 15.0)]),
            var(tmpl, k::fxreg::var::COMIC, &[("size", 12.0)]),
        ],
        "Lens Distort" => vec![
            var(tmpl, k::fxreg::var::BARREL, &[("k1", 0.6)]),
            var(tmpl, k::fxreg::var::PINCUSHION, &[("k1", -0.45)]),
            var(tmpl, k::fxreg::var::FISHEYE, &[("k1", 0.9), ("k2", 0.3)]),
        ],
        "Twirl" => vec![
            var(tmpl, k::fxreg::var::SUBTLE, &[("angle", 45.0)]),
            var(tmpl, k::fxreg::var::STRONG, &[("angle", 360.0)]),
            var(tmpl, k::fxreg::var::REVERSE, &[("angle", -240.0)]),
        ],
        "Ripple" => vec![
            var(tmpl, k::fxreg::var::WATER, &[("amount", 0.05), ("freq", 18.0)]),
            var(tmpl, k::fxreg::var::RINGS, &[("amount", 0.15), ("freq", 6.0)]),
            var(tmpl, k::fxreg::var::STRONG, &[("amount", 0.3), ("freq", 12.0)]),
        ],
        "Kaleidoscope" => vec![
            var(tmpl, k::fxreg::var::MIRROR, &[("segments", 2.0)]),
            var(tmpl, k::fxreg::var::STAR, &[("segments", 10.0)]),
            var(tmpl, k::fxreg::var::MANDALA, &[("segments", 20.0), ("angle", 15.0)]),
        ],
        "Cylinder 3D" => vec![
            var(tmpl, k::fxreg::var::SUBTLE, &[("curvature", 0.2)]),
            var(tmpl, k::fxreg::var::STRONG, &[("curvature", 0.85)]),
            var(tmpl, k::fxreg::var::TUBE, &[("curvature", 1.0)]),
        ],
        // an empty list = a forgotten entry; t543 turns
        // red on it just as t48 does on a fallback category
        _ => Vec::new(),
    }
}

pub fn registry() -> &'static [FxEntry] {
    static REG: std::sync::OnceLock<Vec<FxEntry>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        let mut v = vec![
            FxEntry {
                name: "Gain",
                category: k::fxreg::cat::VOLUME,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::GAIN),
                template: FxInstance::gain(0.0),
            },
            FxEntry {
                name: "Slowed+Reverb",
                category: k::fxreg::cat::TIMESTRETCH,
                audio: true,
                // TWO-SIDED (R.37 item 2): on a video clip it stretches
                // time, that is, it changes the choice of the source frame
                video: true,
                keywords: Some(k::fxreg::kw::SLOWEDREVERB),
                template: FxInstance::slowed_reverb(0.82, 0.1, 85.0),
                // it has no variants either: on a static frame stretching
                // time is invisible, and a strip of identical pictures
                // is exactly the “wasted space” that rule 13 is about
                variants: Vec::new(),
            },
            FxEntry {
                name: "Filter",
                category: k::fxreg::cat::FILTER,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::FILTER),
                template: FxInstance::filter(),
            },
            FxEntry {
                name: "EQ",
                category: k::fxreg::cat::FILTER,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::EQ),
                template: FxInstance::eq(),
            },
            FxEntry {
                name: "Distortion",
                category: k::fxreg::cat::DISTORT,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::DISTORTION),
                template: FxInstance::distortion(),
            },
            FxEntry {
                name: "Bitcrusher",
                category: k::fxreg::cat::DISTORT,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::BITCRUSHER),
                template: FxInstance::bitcrusher(),
            },
            FxEntry {
                name: "Tremolo",
                category: k::fxreg::cat::MODULATION,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::TREMOLO),
                template: FxInstance::tremolo(),
            },
            FxEntry {
                name: "AutoPan",
                category: k::fxreg::cat::MODULATION,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::AUTOPAN),
                template: FxInstance::auto_pan(),
            },
            FxEntry {
                name: "Vibrato",
                category: k::fxreg::cat::MODULATION,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::VIBRATO),
                template: FxInstance::vibrato(),
            },
            FxEntry {
                name: "Chorus",
                category: k::fxreg::cat::MODULATION,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::CHORUS),
                template: FxInstance::chorus(),
            },
            FxEntry {
                name: "Flanger",
                category: k::fxreg::cat::MODULATION,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::FLANGER),
                template: FxInstance::flanger(),
            },
            FxEntry {
                name: "Phaser",
                category: k::fxreg::cat::MODULATION,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::PHASER),
                template: FxInstance::phaser(),
            },
            FxEntry {
                name: "Delay",
                category: k::fxreg::cat::DELAY,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::DELAY),
                template: FxInstance::delay(),
            },
            FxEntry {
                name: "Compressor",
                category: k::fxreg::cat::DYNAMICS,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::COMPRESSOR),
                template: FxInstance::compressor(),
            },
            FxEntry {
                name: "Gate",
                category: k::fxreg::cat::DYNAMICS,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::GATE),
                template: FxInstance::gate(),
            },
            FxEntry {
                name: "Limiter",
                category: k::fxreg::cat::DYNAMICS,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::LIMITER),
                template: FxInstance::limiter(),
            },
            FxEntry {
                name: "Pitch Shift",
                category: k::fxreg::cat::TIMESTRETCH,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::PITCH_SHIFT),
                template: FxInstance::pitch_shift(),
            },
            FxEntry {
                name: "Stereo Width",
                category: k::fxreg::cat::STEREO,
                audio: true,
                video: false,
                variants: Vec::new(),
                keywords: Some(k::fxreg::kw::STEREOWIDTH),
                template: FxInstance::stereo_width(),
            },
        ];
        for (name, tmpl) in video_fx_templates() {
            let (category, keywords) = video_meta(name);
            let variants = video_variants(name, &tmpl);
            v.push(FxEntry {
                name,
                category,
                audio: false,
                video: true,
                keywords,
                template: tmpl,
                variants,
            });
        }
        v
    })
}

/// Registry filter: kind (audio/video) + a substring of the query in the name or keywords, case-insensitive.
/// A pure function — test 48.
///
/// It searches by the keywords of the CURRENT language
/// AND by English ones: otherwise someone who chose Chinese
/// could not find “blur”, although the effect name is exactly
/// that. Keyword values already lie in the catalog in lower case.
///
/// Since R.54 a match on the VARIANT name also shows the row (“blueprint” finds Edges).
/// The match is at the level of the ROW, not the cell: however many variants match, the row stays one,
/// and the filter counters do not depend on the number of variants (t48).
pub fn filter_entries<'a>(entries: &'a [FxEntry], audio: bool, query: &str) -> Vec<&'a FxEntry> {
    let q = query.trim().to_lowercase();
    entries
        .iter()
        .filter(|e| if audio { e.audio } else { e.video })
        .filter(|e| {
            q.is_empty()
                || e.name.to_lowercase().contains(&q)
                || e.keywords.is_some_and(|kw| tr(kw).contains(&q) || tr_en(kw).contains(&q))
                || e.variants.iter().any(|v| {
                    tr(v.name).to_lowercase().contains(&q)
                        || tr_en(v.name).to_lowercase().contains(&q)
                })
        })
        .collect()
}

/// The “+ effect…” button with a popup: search + a list by categories.
/// What was chosen in the picker.
pub enum Picked {
    /// A preset from the user’s library (index in the passed slice).
    Preset(usize),
    /// Management of a user preset from its context menu.
    PresetAction(crate::widgets::PresetAction),
    /// Pinned head of the strip (for the morpher — a source substitution).
    Head,
    Fx(FxInstance),
}

/// Gallery width (R.54). Fixed by the gallery of the
/// longest variant content: rule 3 of the checklist —
/// nothing jumps when typing in the search row and when the number of cells changes.
pub(crate) const GALLERY_W: f32 = 520.0;
/// Scroll height of the effects list and the presets section.
const GALLERY_H: f32 = 450.0;
const PRESETS_H: f32 = 160.0;
/// Drawn cell size (stored twice as large — hidpi).
pub(crate) const CELL_DRAW_W: f32 = 80.0;
const CELL_DRAW_H: f32 = 45.0;

/// The category icon for an effect row. Exhaustive
/// over the registry categories: a new category
/// without an icon must turn red in t543b, and not silently be drawn as emptiness.
fn cat_glyph(cat: Key) -> &'static str {
    // Full paths to the keys — the same reason as for `video_meta`.
    match cat {
        c if c == k::fxreg::cat::MOTION => crate::icons::CAT_MOTION,
        c if c == k::fxreg::cat::GEOMETRY => crate::icons::CAT_GEOMETRY,
        c if c == k::fxreg::cat::BLUR => crate::icons::CAT_BLUR,
        c if c == k::fxreg::cat::KEY => crate::icons::CAT_KEY,
        c if c == k::fxreg::cat::STYLE => crate::icons::CAT_STYLE,
        c if c == k::fxreg::cat::GLOW => crate::icons::CAT_GLOW,
        c if c == k::fxreg::cat::THREE_D => crate::icons::CAT_THREE_D,
        c if c == k::fxreg::cat::COLOR => crate::icons::CAT_COLOR,
        c if c == k::fxreg::cat::DISTORT => crate::icons::CAT_DISTORT,
        c if c == k::fxreg::cat::VOLUME => crate::icons::CAT_VOLUME,
        c if c == k::fxreg::cat::FILTER => crate::icons::CAT_FILTER,
        c if c == k::fxreg::cat::STEREO => crate::icons::CAT_STEREO,
        c if c == k::fxreg::cat::MODULATION => crate::icons::CAT_MODULATION,
        c if c == k::fxreg::cat::DELAY => crate::icons::CAT_DELAY,
        c if c == k::fxreg::cat::DYNAMICS => crate::icons::CAT_DYNAMICS,
        c if c == k::fxreg::cat::TIMESTRETCH => crate::icons::CAT_TIMESTRETCH,
        _ => crate::icons::CAT_VIDEO,
    }
}

/// How many intermediate steps between neighbouring points of the strength axis (R.54, item 433).
/// The axis is QUANTIZED, because each position is its
/// own recipe and its own render: without quantization
/// dragging with the mouse would produce a new frame for every pixel.
const AXIS_STEPS: i32 = 8;

/// The strength-axis point by the cursor shift. The axis is a row `[template, variant₀, variant₁, …]`;
/// the variant cell `cell` sits at position `cell + 1`,
/// and one cell size of shift = one step along the axis.
///
/// Returns `(j, t)`: blend `axis[j]` with `axis[j + 1]` at fraction `t`.
/// A pure function — t551 checks it without any egui.
pub(crate) fn axis_at(cell: usize, dx_px: f32, points: usize) -> (usize, f64) {
    let last = points.saturating_sub(1).max(1);
    let raw = (cell + 1) as f32 + dx_px / CELL_DRAW_W;
    // we quantize and clamp to the bounds of the row: beyond the edges the axis stops,
    // rather than “pulling” non-existent values
    let q = ((raw * AXIS_STEPS as f32).round() as i32).clamp(0, last as i32 * AXIS_STEPS);
    let j = (q / AXIS_STEPS) as usize;
    if j >= last {
        // the right edge of the row is expressed as “previous point, fraction 1”
        return (last - 1, 1.0);
    }
    (j, (q % AXIS_STEPS) as f64 / AXIS_STEPS as f64)
}

/// A point of the row: zero is the effect’s
/// TEMPLATE (strength “nothing”), then the variants.
/// The row is not materialized: an allocation per row in the draw path is invariant No. 12.
fn axis_point(e: &FxEntry, k: usize) -> &FxInstance {
    if k == 0 { &e.template } else { &e.variants[k - 1].fx }
}

fn axis_name(e: &FxEntry, k: usize) -> &str {
    if k == 0 { e.name } else { tr(e.variants[k - 1].name) }
}

/// A strip cell: the picture (or a placeholder while it is being computed) and the caption under it,
/// ONE interaction for all of it. A click in the centre
/// without movement must give an action (the t254 philosophy),
/// and the harness finds the cell by its caption. `prefix` is the effect name for the ACCESSIBLE name of the cell.
/// The visible caption stays short (“Soft”), while the accessible
/// name must be unique: the variant dictionary is shared
/// across the registry, and “Soft” exists for six kinds — without a prefix neither a screen reader nor the harness
/// could tell the Blur cell from the Glow cell (found when writing t547). The string is concatenated INSIDE
/// `widget_info`, and its egui closure is called only when
/// accessibility is on — in a normal run it is zero allocations.
#[allow(clippy::too_many_arguments)]
fn preview_cell(
    ui: &mut egui::Ui,
    preview: &mut Option<&mut crate::fx_thumbs::PickerCtx<'_>>,
    extra: &[FxInstance],
    prefix: &str,
    label: &str,
    frame: egui::Color32,
    // the strength axis: a registry entry and the cell number in its strip. `None` — a cell without an axis
    // (a library preset: CHAINS cannot be blended)
    axis: Option<(&FxEntry, usize)>,
) -> (egui::Response, Option<(usize, f64, FxInstance)>) {
    // the caption is drawn UNDER the picture; an empty caption = the name stands next to it (a preset row),
    // and no space for text is reserved (rule 13)
    let cap_h =
        if label.is_empty() { 0.0 } else { ui.text_style_height(&egui::TextStyle::Small) + 2.0 };
    let size = egui::vec2(CELL_DRAW_W, CELL_DRAW_H + cap_h);
    // click_and_drag: a click without movement = exactly this variant (t254), and a horizontal drag leads
    // the STRENGTH AXIS between neighbours (item 433).
    // The repository idiom is one interaction with a split
    // of clicked()/dragged(), not two nested widgets (the dnd_drag_source ban, t254b).
    let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
    let img_rect = egui::Rect::from_min_size(rect.min, egui::vec2(CELL_DRAW_W, CELL_DRAW_H));
    // BELOW THE SCROLL FOLD a cell is not computed and not drawn: the list is laid out in full
    // (the scroll needs the full size), and without this guard one
    // opening of the gallery ordered a hundred and fifty thumbnails —
    // of which fifteen were visible. The live scenario found
    // it: clicking on an invisible cell went PAST the popup,
    // into the timeline (its rectangle honestly lay where nobody can see it).
    if !ui.is_rect_visible(rect) {
        return (resp, None);
    }
    // The STRENGTH AXIS (item 433): while dragging horizontally,
    // the cell shows a mix of neighbouring points of the row —
    // “how much is put here” is visible frame by frame, and not as a declared “main parameter”
    // (VHS and Color Grade have four such parameters, and choosing one would be disputable).
    //
    // The count is from the CENTRE of its own cell by the absolute
    // pointer position, and not by accumulating `drag_delta`:
    // accumulation drifts by hand on every skipped frame. `drag_stopped` is also read — on the frame of release
    // `dragged()` is already false, and what was shown must be applied.
    let pick = axis.and_then(|(e, cell)| {
        if !(resp.dragged() || resp.drag_stopped()) {
            return None;
        }
        let pos = resp.interact_pointer_pos()?;
        let (j, t) = axis_at(cell, pos.x - rect.center().x, e.variants.len() + 1);
        Some((j, t, axis_point(e, j).lerp(axis_point(e, j + 1), t)))
    });
    let shown: &[FxInstance] =
        pick.as_ref().map(|(_, _, fx)| std::slice::from_ref(fx)).unwrap_or(extra);
    let p = ui.painter();
    // the backdrop is ONE rectangle, not a checkerboard: transparent areas (Greenscreen, Pan3d, CornerPin)
    // show on it as they are, and a checkerboard of tiles would
    // give thousands of primitives on opening (invariant No. 12)
    p.rect_filled(img_rect, 2.0, crate::theme::EXTREME);
    if let Some(ctx) = preview.as_deref_mut()
        && let Some(tex) = ctx.texture(ui.ctx(), shown)
    {
        p.image(
            tex,
            img_rect,
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );
    }
    let stroke = if resp.hovered() || pick.is_some() { crate::theme::ACCENT } else { frame };
    p.rect_stroke(img_rect, 2.0, egui::Stroke::new(1.0, stroke), egui::StrokeKind::Inside);
    resp.widget_info(|| {
        let name = match (prefix.is_empty(), label.is_empty()) {
            (_, true) => prefix.to_string(),
            (true, _) => label.to_string(),
            _ => format!("{prefix} {label}"),
        };
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), name)
    });
    if label.is_empty() {
        return (resp, pick);
    }
    // the caption is truncated, not expanding the cell (rule 7 of the checklist)
    let galley = ui.fonts_mut(|f| {
        let mut job = egui::text::LayoutJob::single_section(
            label.to_string(),
            egui::TextFormat {
                font_id: egui::TextStyle::Small.resolve(ui.style()),
                color: ui.visuals().weak_text_color(),
                ..Default::default()
            },
        );
        job.wrap = egui::text::TextWrapping {
            max_width: CELL_DRAW_W,
            max_rows: 1,
            overflow_character: Some('…'),
            ..Default::default()
        };
        f.layout_job(job)
    });
    let cap_x = img_rect.center().x - galley.size().x / 2.0;
    ui.painter().galley(egui::pos2(cap_x, img_rect.bottom() + 2.0), galley, egui::Color32::WHITE);
    (resp, pick)
}

/// A picker where the head of the strip stands as the FIRST row. It is deliberately absent from `registry()`:
/// t48 checks the registry against the effect templates, and a pseudo-slot
/// has no template — it is not an effect but a part of the morpher.
///
/// Since R.54 this is a gallery: each video effect has a strip of thumbnails under its name (the built-in variants
/// and the suitable single-effect presets), and library presets have their own thumbnail of the whole block.
/// `preview` is the cell context; `None` means “we do not draw
/// pictures” (the audio picker, harness tests without a service),
/// and then the picker degenerates into the former list of names.
#[allow(clippy::too_many_arguments)]
pub fn fx_picker_with_head(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    audio: bool,
    query: &mut String,
    head: Option<&str>,
    presets: &[crate::user_store::PresetRow],
    hover: &mut Option<String>,
    mut preview: Option<&mut crate::fx_thumbs::PickerCtx<'_>>,
) -> Option<Picked> {
    let mut picked = None;
    // CloseOnClickOutside is mandatory (R.54): the egui menu default is CloseOnClick — the popup closes on ANY click,
    // including on a thumbnail, and the gallery would be stillborn.
    // Our clicks close the popup with an explicit `ui.close()`,
    // Esc works as before.
    let cfg = egui::containers::menu::MenuConfig::new()
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside);
    let (btn, popup) =
        egui::containers::menu::MenuButton::new(tr(k::fxreg::ADD)).config(cfg).ui(ui, |ui| {
            ui.set_min_width(if preview.is_some() { GALLERY_W } else { 230.0 });
            let edit = egui::TextEdit::singleline(query)
                .hint_text(tr(k::fxreg::SEARCH))
                .desired_width(210.0);
            let resp = ui.add(edit);
            // focus in the search row when the menu opens
            if ui.input(|i| i.events.is_empty()) && !resp.has_focus() {
                resp.request_focus();
            }
            ui.separator();
            // the head — in its own colour and FIRST (“raise in the fx
            // list”); it is searched with the same search row as the effects
            if let Some(h) = head
                && (query.is_empty() || h.to_lowercase().contains(&query.to_lowercase()))
            {
                let text = egui::RichText::new(h).color(crate::theme::MORPH_SLOT);
                if ui.button(text).clicked() {
                    picked = Some(Picked::Head);
                    query.clear();
                    ui.close();
                }
                ui.separator();
            }
            // Presets (R.48) — in their OWN section right after the head: they are not effect kinds, and they do not get into
            // the static registry (t48 checks it against the templates).
            // They are searched by the same row — both by the preset name
            // and by the names of the effects inside it.
            let q = query.to_lowercase();
            let shown: Vec<(usize, &crate::user_store::PresetRow)> = presets
                .iter()
                .enumerate()
                .filter(|(_, r)| {
                    q.is_empty()
                        || r.name.to_lowercase().contains(&q)
                        || r.block.slots.iter().any(|sl| {
                            crate::fx_editor::fx_display_name(&sl.fx).to_lowercase().contains(&q)
                        })
                })
                .collect();
            if !shown.is_empty() {
                ui.label(egui::RichText::new(tr(k::fx::PRESETS)).small().weak());
                // Presets live ABOVE the scroll of the effects (the R.53 rule: they are reached for first),
                // but they need their own scroll — otherwise
                // thirty saved chains would push the list of effects
                // past the edge of the screen.
                egui::ScrollArea::vertical().id_salt("fxpreset-rows").max_height(PRESETS_H).show(
                    ui,
                    |ui| {
                        for (i, r) in shown {
                            let resp = if preview.is_some() && r.video_ok {
                                // thumbnail of the WHOLE block: the curves are taken at the entry point of the preset
                                // (keys in the block are rebased to zero)
                                let chain: Vec<FxInstance> = r
                                    .block
                                    .slots
                                    .iter()
                                    .filter(|sl| !sl.bypass)
                                    .map(|sl| {
                                        let mut fx = sl.fx.clone();
                                        kt_model::apply_fx_auto(
                                            &mut fx,
                                            &sl.curves,
                                            kt_time::Tick(0),
                                        );
                                        fx
                                    })
                                    .collect();
                                ui.horizontal(|ui| {
                                    // the caption is NOT duplicated: the name
                                    // stands as a button right next to the thumbnail
                                    // (rule 13). A preset has no axis
                                    // — blending CHAINS makes no sense.
                                    let (c, _) = preview_cell(
                                        ui,
                                        &mut preview,
                                        &chain,
                                        &r.name,
                                        "",
                                        crate::theme::MORPH_SLOT,
                                        None,
                                    );
                                    let lbl = crate::widgets::elided_name(
                                        ui,
                                        &r.name,
                                        ui.visuals().text_color(),
                                        false,
                                    );
                                    let b = ui.button(lbl);
                                    if c.clicked() { c } else { b }
                                })
                                .inner
                            } else {
                                let label = crate::widgets::elided_name(
                                    ui,
                                    &r.name,
                                    ui.visuals().text_color(),
                                    false,
                                );
                                ui.button(label)
                            };
                            if resp.clicked() {
                                picked = Some(Picked::Preset(i));
                                query.clear();
                                ui.close();
                            }
                            // factory presets are not renamed or deleted — and this is visible: they have no menu,
                            // and the hint says why
                            if r.factory {
                                resp.on_hover_text(tr(k::msg::PRESET_FACTORY));
                            } else {
                                let name = r.name.clone();
                                resp.context_menu(|ui| {
                                    if ui.button(tr(k::fx::RENAME_PRESET)).clicked() {
                                        picked = Some(Picked::PresetAction(
                                            crate::widgets::PresetAction::Rename(name.clone()),
                                        ));
                                        ui.close();
                                    }
                                    if ui
                                        .button(tr(k::fx::DELETE_PRESET))
                                        .on_hover_text(tr(k::fx::DELETE_PRESET_HINT))
                                        .clicked()
                                    {
                                        picked = Some(Picked::PresetAction(
                                            crate::widgets::PresetAction::Delete(name.clone()),
                                        ));
                                        ui.close();
                                    }
                                });
                            }
                        }
                    },
                );
                ui.separator();
            }
            let list = filter_entries(registry(), audio, query);
            if list.is_empty() && head.is_none() {
                ui.weak(tr(k::fxreg::NOTHING));
            }
            // CATEGORIES ARE GROUPED WHEN DRAWN (R.54). The
            // registry is sorted by meaning (the “quarters” of R.47),
            // and categories repeat in it: the heading “Stylization” was printed five times, “Blur” — three times.
            // The order of the categories themselves is by first appearance; the sort is stable,
            // so inside a category the registry order is intact.
            let mut cats: Vec<Key> = Vec::new();
            for e in &list {
                if !cats.contains(&e.category) {
                    cats.push(e.category);
                }
            }
            let mut rows = list;
            rows.sort_by_key(|e| cats.iter().position(|c| *c == e.category).unwrap_or(usize::MAX));
            let mut last_cat: Option<Key> = None;
            egui::ScrollArea::vertical().id_salt("fxreg-rows").max_height(GALLERY_H).show(
                ui,
                |ui| {
                    for e in rows {
                        if last_cat != Some(e.category) {
                            last_cat = Some(e.category);
                            ui.label(egui::RichText::new(tr(e.category)).small().weak());
                        }
                        // the name row: category glyph + name
                        // (click = TEMPLATE, as it was before R.54)
                        let mut hit_name = None;
                        ui.horizontal(|ui| {
                            ui.label(cat_glyph(e.category));
                            let r = crate::app::hint_into(
                                hover,
                                ui.button(e.name),
                                tr(k::fxreg::TEMPLATE_HINT),
                            );
                            // the TEMPLATE row also gives a rectangle: a
                            // window without a base frame (a track chain)
                            // has no variant gallery at all, and otherwise
                            // the live scenario would have nothing to aim at
                            crate::widgets::field_probe::note(
                                &format!("fxreg.tpl.{}", e.template.tag()),
                                r.rect,
                            );
                            if r.clicked() {
                                hit_name = Some(e.template.clone());
                            }
                        });
                        if let Some(t) = hit_name {
                            picked = Some(Picked::Fx(t));
                            query.clear();
                            ui.close();
                        }
                        // the strip: built-in variants, then the user’s SINGLE-effect presets of the same kind —
                        // they are about the same thing (“here is how
                        // it is tuned”), and their place is there too
                        if preview.is_some() && !e.variants.is_empty() {
                            let mut hit = None;
                            ui.horizontal_wrapped(|ui| {
                                for (vi, v) in e.variants.iter().enumerate() {
                                    let (c, pick) = preview_cell(
                                        ui,
                                        &mut preview,
                                        std::slice::from_ref(&v.fx),
                                        e.name,
                                        tr(v.name),
                                        crate::theme::OUTLINE,
                                        Some((e, vi)),
                                    );
                                    crate::widgets::field_probe::note(
                                        &format!("fxreg.var.{}.{vi}", e.template.tag()),
                                        c.rect,
                                    );
                                    // the hint goes to the status bar, like everything
                                    // in the program (rule 10): while the axis
                                    // is being dragged, it says BETWEEN
                                    // WHAT the drag is and by how much
                                    let axis_hint = pick.as_ref().map(|(j, t, _)| {
                                        crate::tf!(
                                            k::fxreg::AXIS_HINT,
                                            axis_name(e, *j),
                                            axis_name(e, j + 1),
                                            (t * 100.0).round() as i64
                                        )
                                    });
                                    let hint =
                                        axis_hint.as_deref().unwrap_or(tr(k::fxreg::VAR_HINT));
                                    let c = crate::app::hint_into(hover, c, hint);
                                    // a click without movement — EXACTLY this variant
                                    // (t254); the drag — what is
                                    // shown on the frame of release
                                    if c.clicked() {
                                        hit = Some(Picked::Fx(v.fx.clone()));
                                    } else if c.drag_stopped()
                                        && let Some((_, _, fx)) = pick
                                    {
                                        hit = Some(Picked::Fx(fx));
                                    }
                                }
                                for (i, r) in presets.iter().enumerate() {
                                    let one = match r.block.slots.as_slice() {
                                        [only] => only.fx.tag() == e.template.tag(),
                                        _ => false,
                                    };
                                    if !one || !r.video_ok {
                                        continue;
                                    }
                                    let mut fx = r.block.slots[0].fx.clone();
                                    kt_model::apply_fx_auto(
                                        &mut fx,
                                        &r.block.slots[0].curves,
                                        kt_time::Tick(0),
                                    );
                                    let (c, _) = preview_cell(
                                        ui,
                                        &mut preview,
                                        std::slice::from_ref(&fx),
                                        "",
                                        &r.name,
                                        crate::theme::MORPH_SLOT,
                                        None,
                                    );
                                    if crate::app::hint_into(hover, c, tr(k::fxreg::VAR_HINT))
                                        .clicked()
                                    {
                                        hit = Some(Picked::Preset(i));
                                    }
                                }
                            });
                            if let Some(h) = hit {
                                picked = Some(h);
                                query.clear();
                                ui.close();
                            }
                        }
                    }
                },
            );
            let _ = id_salt;
        });
    // rectangles for live scenarios: modal egui popups are not searched in the accessibility tree,
    // and a scenario must not guess offsets inside the window (R.27).
    // The branch costs one OnceLock check without KT_GEOM_DUMP.
    crate::widgets::field_probe::note("fxreg.add", btn.rect);
    // the window of a linked pair has two pickers and the last one drawn
    // (audio) wins the shared name; the README shot needs the VIDEO gallery
    if !audio {
        crate::widgets::field_probe::note("fxreg.add.video", btn.rect);
    }
    // the expanded picker as a whole: it hangs out past the FX window, and the landing-page screenshot
    // crops by the union of the frames (site/screenshots.py)
    if let Some(p) = &popup {
        crate::widgets::field_probe::note("fxreg.popup", p.response.rect);
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 48 (item 3 of v1.4): completeness of the registry and identity of the templates with the former
    /// buttons/combos; the filter finds by a substring of
    /// the name and by keyword and separates audio from video.
    #[test]
    fn t48_registry_complete_and_filter_works() {
        let reg = registry();
        // all 10 video templates are in place and byte-for-byte equal to the former ones
        let old = video_fx_templates();
        assert_eq!(reg.iter().filter(|e| !e.audio).count(), old.len());
        // Exactly one entry is TWO-SIDED — the time warp (R.37).
        // It is not a pixel filter but a rule for choosing
        // the source frame, so it makes sense on both kinds; everything else must stay in its own bucket,
        // otherwise the division degenerates
        assert_eq!(
            reg.iter().filter(|e| e.audio && e.video).map(|e| e.name).collect::<Vec<_>>(),
            ["Slowed+Reverb"],
            "only time stretching can be two-sided"
        );
        for (name, tmpl) in &old {
            let e = reg.iter().find(|e| e.name == *name).expect("video effect in the registry");
            assert!(!e.audio);
            assert_eq!(
                serde_json::to_string(&e.template).unwrap(),
                serde_json::to_string(tmpl).unwrap(),
                "template {name} is identical to the previous one"
            );
        }
        // the audio templates of the former buttons
        let gain = reg.iter().find(|e| e.name == "Gain").unwrap();
        assert!(gain.audio);
        assert_eq!(
            serde_json::to_string(&gain.template).unwrap(),
            serde_json::to_string(&FxInstance::gain(0.0)).unwrap()
        );
        let sr = reg.iter().find(|e| e.name == "Slowed+Reverb").unwrap();
        assert!(sr.audio);
        assert_eq!(
            serde_json::to_string(&sr.template).unwrap(),
            serde_json::to_string(&FxInstance::slowed_reverb(0.82, 0.1, 85.0)).unwrap()
        );
        // the morph slot does NOT live in the registry and cannot live there (R.35): a pseudo-slot has no template,
        // and this test checks the registry precisely against the
        // templates — “simply add it to the registry” must turn red
        assert!(
            !reg.iter().any(|e| e.name.to_lowercase().contains("source")),
            "source substitution is not an effect: its place is at the head of the strip, not in the registry"
        );
        // R.47 (item 331): the fallback category VIDEO is a sign of a forgotten entry in video_meta;
        // every video effect must live in its own category
        for e in reg.iter().filter(|e| e.video && e.name != "Slowed+Reverb") {
            assert_ne!(
                e.category,
                k::fxreg::cat::VIDEO,
                "{}: fell into the fallback category (add it to video_meta)",
                e.name
            );
        }
        // names are consistent with fx_display_name
        for e in reg {
            assert_eq!(crate::fx_editor::fx_display_name(&e.template), e.name);
        }
        // R.54: audio entries have no variants — a sound
        // thumbnail cannot be drawn, and a strip of captions alone
        // would be a second list of names in the same place (rule 13)
        for e in reg.iter().filter(|e| e.audio) {
            assert!(e.variants.is_empty(), "{}: audio entries carry no variants", e.name);
        }
        // R.47 (item 326): the count is a PROPERTY of the registry FxInstance::all(), not a constant.
        // Every audio kind from all() (except LayerMix — the
        // strength slot, which has no place in the picker)
        // must be in the audio list; every video kind that has a template — in the video list.
        crate::i18n::set_lang(crate::i18n::Lang::Ru);
        let audio_kinds: std::collections::BTreeSet<&str> =
            FxInstance::all().iter().filter(|f| f.is_audio()).map(|f| f.tag()).collect();
        let audio_in_picker: std::collections::BTreeSet<&str> =
            filter_entries(reg, true, "").iter().map(|e| e.template.tag()).collect();
        assert_eq!(audio_in_picker, audio_kinds, "every audio kind of the registry is in the picker");
        // video list = all video templates + the two-sided Slowed+Reverb
        assert_eq!(filter_entries(reg, false, "").len(), old.len() + 1);
        assert_eq!(filter_entries(reg, false, "reverb").len(), 1, "warp is also searched in the video picker");
        assert_eq!(filter_entries(reg, true, "ReVerb").len(), 1, "Cyrillic keyword");
        assert_eq!(filter_entries(reg, false, "vish").len(), 0);
        assert_eq!(filter_entries(reg, false, "vhs").len(), 1);
        // R.47 Q3: “blur” finds Blur AND Radial Blur
        assert_eq!(filter_entries(reg, false, "blur").len(), 2);
        assert_eq!(filter_entries(reg, true, "gain").len(), 1);
        assert!(filter_entries(reg, true, "vhs").is_empty(), "video effect is not in the audio list");
        // R.19: English keywords must find the effect in ANY language — the effect name is not translated,
        // and it will be searched by it
        for lang in crate::i18n::Lang::ALL {
            crate::i18n::set_lang(lang);
            assert_eq!(filter_entries(reg, false, "blur").len(), 2, "blur in {}", lang.code());
            assert_eq!(filter_entries(reg, false, "vhs").len(), 1, "vhs in {}", lang.code());
            assert_eq!(filter_entries(reg, true, "reverb").len(), 1, "reverb in {}", lang.code());
        }
        crate::i18n::set_lang(crate::i18n::Lang::Ru);
    }

    /// Test 543 (R.54, item 427): built-in variants of an effect.
    ///
    /// A variant is a TUNED state of the same kind, and it has exactly three duties: to be the same kind,
    /// to differ from the template and to lie within the
    /// spec bounds. The first trap that the test catches
    /// on old code — a typo in a parameter name: `set_param` returns false, the value is not set,
    /// and the variant silently becomes a copy of the template
    /// (in release `debug_assert!` in `var()` does not fire,
    /// and the strip cell shows the source frame).
    #[test]
    fn t543_builtin_variants_are_distinct_and_in_range() {
        let reg = registry();
        // the order of languages does not matter, but the names are read in a known way
        crate::i18n::set_lang(crate::i18n::Lang::Ru);
        let mut with_variants = 0;
        for e in reg.iter().filter(|e| e.video && e.name != "Slowed+Reverb") {
            // a forgotten entry in video_variants gives an empty
            // list — the same trap as the fallback category in t48
            assert!(
                e.variants.len() >= 2,
                "{}: fewer than two variants (add it to video_variants)",
                e.name
            );
            with_variants += 1;
            let tmpl_repr = format!("{:?}", e.template);
            let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
            for v in &e.variants {
                // the SAME kind: a variant does not replace the effect with another
                assert_eq!(
                    std::mem::discriminant(&v.fx),
                    std::mem::discriminant(&e.template),
                    "{}: variant changes the effect kind",
                    e.name
                );
                let repr = format!("{:?}", v.fx);
                // differs from the template. For Pan/Pan 3D/Color
                // Grade/Corner Pin/Drop Shadow this is also the condition
                // of the ICON: their template is an identity, the icon is drawn by the FIRST variant, and a copy of
                // the template would give an empty picture
                assert_ne!(repr, tmpl_repr, "{}: variant equals the template", e.name);
                // and from the other variants of the same effect
                assert!(seen.insert(repr), "{}: two identical variants", e.name);
                // values within the spec bounds (`set_param`
                // clamps, but Mirror is assembled as a literal,
                // and a future edit may bypass var()).
                //
                // The check is STRICT on purpose, and here is why: for
                // an f32 field the exact spec ceiling after `set_param`
                // ends up OUTSIDE the spec — the clamp is computed in f64, and the write to f32 rounds up
                // (`amount` = 0.05 in Chromatic Aberration gave
                // 0.05000000074505806). A variant is a KIND, not a limit check:
                // if it hits the boundary — step away from it, do not weaken the test with a tolerance.
                for s in v.fx.param_specs() {
                    let x = v.fx.get_param(&s.name).unwrap_or(f64::NAN);
                    assert!(
                        x.is_finite() && x >= s.min && x <= s.max,
                        "{} / {}: {} = {x} out of [{}, {}]",
                        e.name,
                        tr(v.name),
                        s.name,
                        s.min,
                        s.max
                    );
                }
            }
            // the names of variants of ONE effect are distinguishable in
            // all languages: the dictionary is shared across the registry,
            // and “Soft” twice in one strip would be two indistinguishable clickable places
            for lang in crate::i18n::Lang::ALL {
                crate::i18n::set_lang(lang);
                let mut names: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
                for v in &e.variants {
                    let n = tr(v.name);
                    assert!(!n.is_empty(), "{}: empty variant name in {}", e.name, lang.code());
                    assert!(
                        names.insert(n),
                        "{}: duplicate variant name {n:?} in {}",
                        e.name,
                        lang.code()
                    );
                }
            }
            crate::i18n::set_lang(crate::i18n::Lang::Ru);
        }
        // the count is a property of the registry, not a constant (the t48 rule)
        assert_eq!(with_variants, video_fx_templates().len());
        // the head of the variants strip does not have one: it is not an effect (R.35)
        // and it is not in the registry — checked in t48.

        // a search by variant name shows the EFFECT ROW, and
        // in any language — also by the English name (rule R.19)
        for lang in crate::i18n::Lang::ALL {
            crate::i18n::set_lang(lang);
            let by_var = filter_entries(reg, false, "blueprint");
            assert_eq!(
                by_var.iter().map(|e| e.name).collect::<Vec<_>>(),
                ["Edges"],
                "blueprint → Edges in {}",
                lang.code()
            );
        }
        crate::i18n::set_lang(crate::i18n::Lang::Ru);
        assert_eq!(
            filter_entries(reg, false, "blueprint").iter().map(|e| e.name).collect::<Vec<_>>(),
            ["Edges"],
            "the Russian name of the variant is searched too"
        );
        // ...and does not break the former counters: the match is at the level of the ROW
        assert_eq!(filter_entries(reg, false, "blur").len(), 2);
    }
}
