//! Генератор демо-проектов конкатенации трансформов (Р.44/Р.45, п.315):
//! `cargo test -p kt-cli --test gen_demo -- --ignored` перезаписывает
//! `fixtures/demo-concat/*.otio`. Медиа — `gen-media.sh` там же.
//! Что смотреть в каждом проекте — fixtures/demo-concat/README.md.

use kt_model::*;
use kt_time::Tick;

const SEC: i64 = 90_000;

fn project() -> Project {
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 1280, height: 720, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "detail.mp4".into() });
    p.media.push(MediaRef { id: MediaId(2), path: "green.mp4".into() });
    p
}

fn clip(id: u64, start_s: i64, dur_s: i64, media: u64, fx: Vec<FxInstance>) -> ClipEvent {
    ClipEvent {
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
    FxInstance::Pan { mode: PanMode::Stretch, pan_x: 0.0, pan_y: 0.0, zoom: z }
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
    let key = |t_s: f64, v: f64| AutoKey { t: Tick((t_s * SEC as f64) as i64), v };
    let curve = |param: &str, keys: Vec<AutoKey>| ParamCurve {
        param: param.into(),
        ease: true,
        keys,
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
        clip(
            2,
            4,
            4,
            2,
            vec![grid2(), FxInstance::greenscreen([0, 255, 0], 90, 20), zoom(2.0)],
        ),
        clip(
            3,
            8,
            4,
            1,
            vec![grid2(), FxInstance::Noise { amount: 0.35, size: 2, color: true }, zoom(2.0)],
        ),
    ];
    save(&mut p, "04-pointwise.otio");
}
