//! Тесты 406-407 (Р.49 Б5): громкость считается на проходе экспорта.

use kt_engine::export::render_raw_audio;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

fn project_with(gain_db: f32, secs: f64) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    
    default_shape: Default::default(),});
    p.media.push(MediaRef { id: MediaId(1), path: "synth:sine:1000".into() });
    p.clips.push(ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Audio,
        lane: 0,
        start: Tick(0),
        duration: Tick((90_000.0 * secs) as i64),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.audio_lanes = 1;
    p.normalize();
    p
}

fn sources() -> Vec<(MediaId, Box<dyn kt_media::MediaSource>)> {
    vec![(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 1000.0 }))
            as Box<dyn kt_media::MediaSource>,
    )]
}

/// Тест 406 (Р.49 Б5): экспорт сообщает громкость, и она слышит громкость
/// СОБЫТИЯ. Метр кормится готовым буфером чанка — ни одного нового декода.
#[test]
fn t406_export_reports_loudness_of_the_mix() {
    let mut frames = 0u64;
    let loud = render_raw_audio(&project_with(0.0, 12.0), sources(), |b| {
        frames += (b.len() / AUDIO_CHANNELS) as u64;
        Ok(())
    })
    .unwrap();
    let a = loud.loudness.expect("экспорт обязан сообщить громкость").lufs.expect("не тишина");
    assert!(frames > 0, "аудио вообще не рендерилось");

    // то же событие на 10 dB тише — ровно на 10 LU тише
    let quiet = render_raw_audio(&project_with(-10.0, 12.0), sources(), |_| Ok(())).unwrap();
    let b = quiet.loudness.unwrap().lufs.unwrap();
    assert!(
        ((a - b) - 10.0).abs() <= 0.15,
        "громкость экспорта не слышит уровень события: {a:.2} против {b:.2} LUFS"
    );

    // истинный пик — про тот же материал: у `synth:sine` амплитуда 0.5,
    // то есть −6.02 dBFS, и истинный пик обязан лечь рядом
    let dbtp = loud.loudness.unwrap().dbtp;
    assert!(
        (dbtp + 6.02).abs() < 0.5,
        "истинный пик разошёлся с амплитудой источника: {dbtp:.2} dBTP при −6.02 dBFS"
    );
}

/// Тест 407 (Р.49 Б5): мастер-гейн в измерении НЕ участвует — и не может.
///
/// Он живёт только в `Player`, его нет ни в `Project`, ни в .otio, экспорт его
/// не применяет, а ставит его фейдер панели метров: это громкость КОНТРОЛЯ.
/// Следствие, которое и проверяется: число в превью и число в файле совпадают
/// ПО ПОСТРОЕНИЮ, потому что считает их один и тот же код по одному и тому же
/// материалу.
#[test]
fn t407_master_gain_is_not_part_of_the_measurement() {
    let p = project_with(0.0, 12.0);
    let a = render_raw_audio(&p, sources(), |_| Ok(())).unwrap().loudness.unwrap().lufs.unwrap();

    // мастер-гейн — свойство плеера; в модели проекта его нет вовсе, и
    // записать его туда нечем. Сторож: поле с таким смыслом не должно
    // появиться в настройках проекта незаметно.
    let settings = format!("{:?}", p.settings);
    assert!(
        !settings.contains("master"),
        "мастер-гейн просочился в настройки проекта ({settings}) — тогда экспорт обязан \
         его применять, и «громкость контроля» перестанет быть громкостью контроля"
    );

    // повтор того же экспорта даёт то же число: измерение детерминировано
    let b = render_raw_audio(&p, sources(), |_| Ok(())).unwrap().loudness.unwrap().lufs.unwrap();
    assert_eq!(a, b, "измерение громкости недетерминировано");
}
