//! Generator of the demo projects of the concatenation of transforms (R.44/R.45, item 315):
//! `cargo test -p kt-cli --test gen_demo -- --ignored` overwrites
//! `fixtures/demo-concat/*.kt` (R.70: the project file is its own format). The media —
//! `gen-media.sh` there as well.
//! What to look at in each project — fixtures/demo-concat/README.md.

use kt_model::*;
use kt_time::Tick;

const SEC: i64 = 90_000;

fn project() -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 1280,
        height: 720,
        sample_rate: 48000,
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(MediaRef::new(MediaId(1), "detail.mp4"));
    p.media.push(MediaRef::new(MediaId(2), "green.mp4"));
    p
}

fn clip(id: u64, start_s: i64, dur_s: i64, media: u64, fx: Vec<FxInstance>) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(id),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(start_s * SEC),
        duration: Tick(dur_s * SEC),
        source_in: Tick(0),
        media: MediaId(media),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx,
        opacity: 1.0,
        fx_off: false,
    }
}

fn grid2() -> FxInstance {
    FxInstance::EventGrid { cols: 2, rows: 2, center: true }
}

fn pan3d(scale: f64) -> FxInstance {
    FxInstance::Pan3d {
        pos_x: 0.0,
        pos_y: 0.0,
        pos_z: 0.0,
        rot_x: 0.0,
        rot_y: 0.0,
        rot_z: 0.0,
        anchor_x: 0.5,
        anchor_y: 0.5,
        scale,
        camera: None,
    }
}

fn zoom(z: f64) -> FxInstance {
    FxInstance::geometry_zoom(z)
}

fn save(p: &mut Project, name: &str) {
    p.validate().expect(name);
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/demo-concat");
    let text = kt_model::project_file::to_text(p, None);
    std::fs::write(dir.join(name), text).unwrap();
    println!("written {name}");
}

#[test]
#[ignore]
fn gen_demo_concat() {
    // 01: net unit — a chain that in total returns the scale to 1:1,
    // must be INDISTINGUISHABLE from an event without FX (before R.44 — blur).
    let mut p = project();
    p.clips = vec![
        clip(1, 0, 4, 1, vec![]),
        clip(2, 4, 4, 1, vec![grid2(), pan3d(1.0), grid2(), pan3d(4.0)]),
        clip(3, 8, 4, 1, vec![grid2(), zoom(2.0)]),
    ];
    save(&mut p, "01-net-identity.kt");

    // 02: the user scenario of R.44 with curves — the parallaxes come to
    // normal, the zoom at the end removes the grids; the finale must be sharp.
    let mut p = project();
    let mut c = clip(1, 0, 8, 1, vec![grid2(), pan3d(1.0), grid2(), pan3d(1.0)]);
    let key = |t_s: f64, v: f64| AutoKey::at(Tick((t_s * SEC as f64) as i64), v);
    let curve = |param: &str, keys: Vec<AutoKey>| ParamCurve {
        param: param.into(),
        keys,
        source: Default::default(),
    };
    c.fx_auto = vec![
        vec![],
        // the first pan3d: the parallax (rotation + shift) returns to normal by 4 s
        vec![
            curve("rot_y", vec![key(0.0, 28.0), key(4.0, 0.0)]),
            curve("pos_x", vec![key(0.0, 90.0), key(4.0, 0.0)]),
        ],
        vec![],
        // the second pan3d: a light parallax up to 4 s, then a zoom ×4 removes the grids
        vec![
            curve("rot_y", vec![key(0.0, -14.0), key(4.0, 0.0)]),
            curve("scale", vec![key(0.0, 1.0), key(4.0, 1.0), key(7.5, 4.0)]),
        ],
    ];
    p.clips = vec![c];
    save(&mut p, "02-user-parallax.kt");

    // 03: K× barriers (R.45) — the VHS/edges texture survives the zoom.
    let mut p = project();
    p.clips = vec![
        clip(
            1,
            0,
            4,
            1,
            vec![
                grid2(),
                FxInstance::Vhs { rgb_shift: 3, bleed: 0.35, scan_step: 4, jitter: 0.0 },
                zoom(2.0),
            ],
        ),
        clip(2, 4, 4, 1, vec![grid2(), FxInstance::edges(48), zoom(2.0)]),
        clip(
            3,
            8,
            4,
            1,
            vec![FxInstance::Vhs { rgb_shift: 3, bleed: 0.35, scan_step: 4, jitter: 0.0 }],
        ),
    ];
    save(&mut p, "03-barrier.kt");

    // 04: pointwise within a series (R.45) — the key and the grain through the zoom.
    let mut p = project();
    p.clips = vec![
        clip(1, 0, 4, 2, vec![FxInstance::greenscreen([0, 255, 0], 90, 20)]),
        clip(2, 4, 4, 2, vec![grid2(), FxInstance::greenscreen([0, 255, 0], 90, 20), zoom(2.0)]),
        clip(
            3,
            8,
            4,
            1,
            vec![grid2(), FxInstance::Noise { amount: 0.35, size: 2, color: true }, zoom(2.0)],
        ),
    ];
    save(&mut p, "04-pointwise.kt");

    // 05: decode step (R.46). Clips 1-2 on detail-hi.mp4 (2560x1440),
    // clip 3 — the same zoom on detail.mp4 (1280x720, no margin): side by side
    // it is visible that the sharpness is taken FROM THE FILE, not from the zoom.
    let mut p = project();
    p.media.push(MediaRef::new(MediaId(3), "detail-hi.mp4"));
    p.clips = vec![
        clip(1, 0, 4, 3, vec![]),
        clip(2, 4, 4, 3, vec![zoom(2.0)]),
        clip(3, 8, 4, 1, vec![zoom(2.0)]),
        clip(
            4,
            12,
            4,
            3,
            vec![
                FxInstance::Vhs { rgb_shift: 3, bleed: 0.35, scan_step: 4, jitter: 0.0 },
                zoom(2.0),
            ],
        ),
    ];
    save(&mut p, "05-decode-scale.kt");

    // 06: color (R.47 Kv2) — pointwise solos and INSIDE the geometry series
    // (absorption by tap: the key and the zoom do not break the color formula).
    let mut p = project();
    p.clips = vec![
        clip(
            1,
            0,
            4,
            1,
            vec![FxInstance::ColorGrade {
                brightness: 0.05,
                contrast: 0.35,
                saturation: 1.5,
                hue: -20.0,
            }],
        ),
        clip(2, 4, 4, 1, vec![FxInstance::duotone()]),
        clip(3, 8, 4, 1, vec![FxInstance::Posterize { levels: 4 }]),
        clip(4, 12, 4, 1, vec![grid2(), FxInstance::vignette(), zoom(2.0)]),
    ];
    save(&mut p, "06-color.kt");

    // 07: barriers (R.47 Kv3) — sharpness/mosaic/aberration/rays/shadow/raster.
    let mut p = project();
    p.clips = vec![
        clip(1, 0, 4, 1, vec![FxInstance::Sharpen { amount: 2.0, radius: 3 }]),
        clip(2, 4, 4, 1, vec![FxInstance::Pixelate { size: 16 }]),
        clip(3, 8, 4, 1, vec![FxInstance::ChromaticAberration { amount: 0.02 }]),
        clip(
            4,
            12,
            4,
            1,
            vec![FxInstance::RadialBlur { amount: 0.5, center_x: 0.0, center_y: 0.0, spin: false }],
        ),
        // the shadow is visible on the cut-out key green.mp4
        clip(
            5,
            16,
            4,
            2,
            vec![
                FxInstance::greenscreen([0, 255, 0], 90, 20),
                FxInstance::DropShadow {
                    offset_x: 14,
                    offset_y: 14,
                    radius: 6,
                    opacity: 0.7,
                    tint_r: 0,
                    tint_g: 0,
                    tint_b: 0,
                },
            ],
        ),
        clip(6, 20, 4, 1, vec![FxInstance::Halftone { size: 10, angle: 45.0 }]),
    ];
    save(&mut p, "07-fx-barriers.kt");

    // 08: warps (R.47 Kv4) — nonlinear geo in the concatenation; Ripple with
    // a phase curve (the wave runs), CornerPin with the corner curves (the screen
    // “settles” into place).
    let mut p = project();
    let key = |t_s: f64, v: f64| AutoKey::at(Tick((t_s * SEC as f64) as i64), v);
    let curve = |param: &str, keys: Vec<AutoKey>| ParamCurve {
        param: param.into(),
        keys,
        source: Default::default(),
    };
    let mut ripple =
        clip(1, 0, 4, 1, vec![FxInstance::Ripple { amount: 0.12, freq: 12.0, phase: 0.0 }]);
    ripple.fx_auto = vec![vec![curve("phase", vec![key(0.0, 0.0), key(4.0, 1080.0)])]];
    let mut pin = clip(2, 4, 4, 1, vec![FxInstance::corner_pin()]);
    pin.fx_auto = vec![vec![
        curve("x0", vec![key(0.0, 0.35), key(3.0, 0.0)]),
        curve("y0", vec![key(0.0, 0.25), key(3.0, 0.0)]),
        curve("x3", vec![key(0.0, 0.8), key(3.0, 1.0)]),
        curve("y3", vec![key(0.0, 0.75), key(3.0, 1.0)]),
    ]];
    p.clips = vec![
        ripple,
        pin,
        clip(3, 8, 4, 1, vec![FxInstance::LensDistort { k1: -0.5, k2: 0.0 }]),
        clip(4, 12, 4, 1, vec![FxInstance::twirl()]),
        // the concatenation: kaleidoscope and cylinder WITH A ZOOM — one merged run
        clip(5, 16, 4, 1, vec![FxInstance::kaleidoscope(), zoom(1.5)]),
        clip(6, 20, 4, 1, vec![FxInstance::Cylinder3d { curvature: 0.8 }, zoom(1.3)]),
    ];
    save(&mut p, "08-warp.kt");
}
