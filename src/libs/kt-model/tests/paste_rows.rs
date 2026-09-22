//! Тест 595 (docs/porting-notes.md, п.450, Р.56): координаты дорожек у
//! блока обмена.
//!
//! Претензия владельца дословно: «я копирую ZEF последний в длинной
//! композиции… а он берет и вставляет одну часть сверху а другую
//! часть(аудио) уносит вниз, ну и в самом низу морфер т.к. для него
//! действительно места не было». Причина — клипборд хранил номера
//! РАБОЧЕГО ВИДА, где полоса раскрытия смещена на `base[вид]`, а
//! смещения у видов разные.

use kt_model::*;
use kt_time::Tick;

fn settings() -> ProjectSettings {
    ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    
    default_shape: Default::default(),}
}

fn clip(id: u64, kind: ClipKind, lane: u32) -> ClipEvent {
    ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        id: ClipId(id),
        link_group: Some(GroupId(1)),
        kind,
        lane,
        start: Tick(0),
        duration: Tick(90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

/// Пара видео+аудио, морфер и камера — ровно то, что копировал владелец.
fn block() -> ops::ClipBlock {
    ops::ClipBlock {
        clips: vec![clip(1, ClipKind::Video, 1), clip(2, ClipKind::Audio, 3)],
        morphers: vec![MorpherEvent {
            id: MorphId(1),
            lane: 0,
            start: Tick(0),
            duration: Tick(90_000),
            targets: MorphTargets::Below,
            patch: MorphPatch::default(),
            timed: false,
            strength: 1.0,
            fade_in: Tick::ZERO,
            fade_out: Tick::ZERO,
            enabled: true,
        }],
        cameras: vec![CameraEvent {
            ease: true,
            id: CameraId(1),
            lane: 1,
            start: Tick(0),
            duration: Tick(90_000),
            name: String::new(),
            cam: CameraParams::default(),
            keyframes: vec![],
            auto: vec![],
        }],
        ..Default::default()
    }
}

fn lanes(b: &ops::ClipBlock) -> (u32, u32, u32, u32) {
    (
        b.clips.iter().find(|c| c.kind == ClipKind::Video).unwrap().lane,
        b.clips.iter().find(|c| c.kind == ClipKind::Audio).unwrap().lane,
        b.morphers[0].lane,
        b.cameras[0].lane,
    )
}

#[test]
fn t595_block_is_shifted_to_the_clicked_row() {
    use LaneKind::*;

    // клик по ВЕРХНЕЙ видео-строке: все виды едут на −1, но у морфера и
    // камеры ниже нуля места нет — их сдвиг УРЕЗАН, и это названо
    let (b, clamped) = block().shifted_to_row(Some((Video, 0)));
    assert_eq!(lanes(&b), (0, 2, 0, 0), "видео не совмещено с кликнутой строкой");
    assert_eq!(clamped, vec![Morph], "урезание сдвига не названо");

    // клик по ВЕРХНЕЙ аудио-строке: дельта −3, урезаны три вида
    let (b, clamped) = block().shifted_to_row(Some((Audio, 0)));
    assert_eq!(lanes(&b), (0, 0, 0, 0));
    assert_eq!(clamped, vec![Video, Morph, Camera]);

    // клик по камерной строке: дельта +3, пара сохраняет расстояние
    let (b, clamped) = block().shifted_to_row(Some((Camera, 4)));
    assert_eq!(lanes(&b), (4, 6, 3, 4), "взаимное положение видов поехало");
    assert!(clamped.is_empty());

    // вида в блоке нет — не двигаем вовсе: угадывать нечего
    let mut only_video = block();
    only_video.cameras.clear();
    let (b, clamped) = only_video.shifted_to_row(Some((Camera, 4)));
    assert_eq!(b.clips[0].lane, 1, "блок без камер поехал от клика по камерной строке");
    assert!(clamped.is_empty());

    // строки нет (Ctrl+V мимо таймлайна) — блок как есть
    let (b, _) = block().shifted_to_row(None);
    assert_eq!(lanes(&b), (1, 3, 0, 1));
}

#[test]
fn t595b_localize_converts_a_band_block_into_reel_coordinates() {
    use LaneKind::*;
    // полоса раскрытия: у видов РАЗНЫЕ базы — ровно то, из-за чего пара
    // разъезжалась при вставке
    let base = LaneBases { video: 3, audio: 2, morph: 1, camera: 0 };
    let len = LaneBases { video: 2, audio: 2, morph: 1, camera: 2 };
    let mut b = ops::ClipBlock {
        clips: vec![
            clip(1, ClipKind::Video, 4), // полоса: локально 1
            clip(2, ClipKind::Audio, 2), // полоса: локально 0
            clip(3, ClipKind::Video, 0), // КОРЕНЬ — чужое
        ],
        morphers: block().morphers,
        cameras: block().cameras,
        ..Default::default()
    };
    b.morphers[0].lane = 1; // полоса: локально 0
    b.cameras[0].lane = 1; // полоса: локально 1
    let dropped = b.localize(base, len);
    assert_eq!(dropped, 1, "корневое событие не выброшено из блока ленты");
    assert_eq!(lanes(&b), (1, 0, 0, 1), "перевод в координаты ленты неверен");

    // и обратно — тождество
    b.offset_lanes(base);
    assert_eq!(lanes(&b), (4, 2, 1, 1), "обратный перевод не совпал");

    // сколько дорожек нужно под блок
    let need = b.lanes_needed();
    assert_eq!((need.video, need.audio, need.morph, need.camera), (5, 3, 2, 2));
}

/// Вставка в пустой проект заводит дорожки ЯВНО и одним шагом истории.
#[test]
fn t595c_paste_grows_lanes_in_one_step() {
    let mut p = Project::new(settings());
    p.media.push(MediaRef { id: MediaId(1), path: "synth:sine:440".into() });
    let mut m = TimelineModel::new(p);
    let (b, _) = block().shifted_to_row(Some((LaneKind::Camera, 4)));

    let need = b.lanes_needed();
    let mut cmds: Vec<Command> = Vec::new();
    for (k, n) in [
        (LaneKind::Video, need.video),
        (LaneKind::Audio, need.audio),
        (LaneKind::Morph, need.morph),
        (LaneKind::Camera, need.camera),
    ] {
        let have = match k {
            LaneKind::Video => m.project().video_lanes,
            LaneKind::Audio => m.project().audio_lanes,
            LaneKind::Morph => m.project().morph_lanes,
            LaneKind::Camera => m.project().camera_lanes,
        };
        for at in have..n {
            cmds.push(match k {
                LaneKind::Video => Command::InsertLane { kind: ClipKind::Video, at },
                LaneKind::Audio => Command::InsertLane { kind: ClipKind::Audio, at },
                LaneKind::Morph => Command::InsertMorphLane { at },
                LaneKind::Camera => Command::InsertCameraLane { at },
            });
        }
    }
    let (mut c, mut g, mut t, mut mo, mut ca) = (100u64, 100u64, 100u64, 100u64, 100u64);
    let (paste, _end, _skipped) = ops::paste_block(
        &b,
        Tick(0),
        ops::PasteIds {
            clip: &mut c,
            group: &mut g,
            transition: &mut t,
            morph: &mut mo,
            camera: &mut ca,
        },
        &ops::PasteNames::default(),
    );
    cmds.extend(paste);
    let depth = m.undo_depth();
    m.apply_batch(cmds).expect("вставка с ростом дорожек");
    assert_eq!(m.undo_depth(), depth + 1, "вставка стоила больше одного шага истории");

    let p = m.project();
    assert_eq!((p.video_lanes, p.audio_lanes, p.morph_lanes, p.camera_lanes), (5, 7, 4, 5));
    let v = p.clips.iter().find(|c| c.kind == ClipKind::Video).unwrap();
    let a = p.clips.iter().find(|c| c.kind == ClipKind::Audio).unwrap();
    assert_eq!((v.lane, a.lane), (4, 6), "пара приехала не на свои дорожки");
    assert_eq!(v.link_group, a.link_group, "link-группа пары распалась");
    assert_eq!(p.morphers[0].lane, 3);
    assert_eq!(p.cameras[0].lane, 4);
}
