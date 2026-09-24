//! Генератор демо-проектов конкатенации трансформов (Р.44/Р.45, п.315):
//! `cargo test -p kt-cli --test gen_demo -- --ignored` перезаписывает
//! `fixtures/demo-concat/*.otio`. Медиа — `gen-media.sh` там же.
//! Что смотреть в каждом проекте — fixtures/demo-concat/README.md.

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
    let text = kt_otio::save_project(p, &Default::default(), None);
    std::fs::write(dir.join(name), text).unwrap();
    println!("написан {name}");
}

#[test]
#[ignore]
fn gen_demo_concat() {
    // 01: нетто-единица — цепочка, суммарно возвращающая масштаб к 1:1,
    // обязана быть НЕОТЛИЧИМА от события без FX (до Р.44 — мыло).
    let mut p = project();
    p.clips = vec![
        clip(1, 0, 4, 1, vec![]),
        clip(2, 4, 4, 1, vec![grid2(), pan3d(1.0), grid2(), pan3d(4.0)]),
        clip(3, 8, 4, 1, vec![grid2(), zoom(2.0)]),
    ];
    save(&mut p, "01-net-identity.otio");

    // 02: сценарий пользователя Р.44 с кривыми — параллаксы приходят в
    // норму, зум в конце устраняет сетки; финал обязан быть резким.
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
        // первый pan3d: параллакс (поворот+сдвиг) уходит в норму к 4 c
        vec![
            curve("rot_y", vec![key(0.0, 28.0), key(4.0, 0.0)]),
            curve("pos_x", vec![key(0.0, 90.0), key(4.0, 0.0)]),
        ],
        vec![],
        // второй pan3d: лёгкий параллакс до 4 c, затем зум ×4 убирает сетки
        vec![
            curve("rot_y", vec![key(0.0, -14.0), key(4.0, 0.0)]),
            curve("scale", vec![key(0.0, 1.0), key(4.0, 1.0), key(7.5, 4.0)]),
        ],
    ];
    p.clips = vec![c];
    save(&mut p, "02-user-parallax.otio");

    // 03: K×-барьеры (Р.45) — фактура VHS/edges переживает зум.
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
    save(&mut p, "03-barrier.otio");

    // 04: поточечные внутри серии (Р.45) — ключ и зерно сквозь зум.
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
    save(&mut p, "04-pointwise.otio");

    // 05: ступень декода (Р.46). Клипы 1-2 на detail-hi.mp4 (2560x1440),
    // клип 3 — тот же зум на detail.mp4 (1280x720, запаса нет): рядом
    // видно, что резкость берётся ИЗ ФАЙЛА, а не из зума.
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
    save(&mut p, "05-decode-scale.otio");

    // 06: цвет (Р.47 Кв2) — поточечные соло и ВНУТРИ гео-серии
    // (поглощение тапом: ключ и зум не ломают формулу цвета).
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
    save(&mut p, "06-color.otio");

    // 07: барьеры (Р.47 Кв3) — резкость/мозаика/аберрация/лучи/тень/растр.
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
        // тень видна на вырезанном ключе green.mp4
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
    save(&mut p, "07-fx-barriers.otio");

    // 08: варпы (Р.47 Кв4) — нелинейные гео в конкатенации; Ripple с
    // кривой phase (волна бежит), CornerPin с кривыми углов (экран
    // «устанавливается» на место).
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
        // конкатенация: калейдоскоп и цилиндр С ЗУМОМ — один слитый ран
        clip(5, 16, 4, 1, vec![FxInstance::kaleidoscope(), zoom(1.5)]),
        clip(6, 20, 4, 1, vec![FxInstance::Cylinder3d { curvature: 0.8 }, zoom(1.3)]),
    ];
    save(&mut p, "08-warp.otio");
}
