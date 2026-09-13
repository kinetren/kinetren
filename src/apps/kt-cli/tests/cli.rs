//! Тест 151 (п.151 porting-notes, Р.24): kt-cli на .otio.
//!
//! new/demo пишут валидный OTIO; roundtrip байт-стабилен; convert
//! конвертирует легаси-.ktproj; render/export отвергают .ktproj с
//! подсказкой на convert.

use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_kt-cli"))
}

#[test]
fn t151_cli_otio_workflows() {
    let dir = std::env::temp_dir().join(format!("kt-cli-t151-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    // new → валидный .otio
    let new_path = dir.join("empty.otio");
    let st = cli().args(["new", new_path.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let text = std::fs::read_to_string(&new_path).unwrap();
    assert!(text.starts_with("{\n    \"OTIO_SCHEMA\": \"Timeline.1\""));

    // demo → богатый .otio (переход, метки, регион, морфер)
    let demo_path = dir.join("demo.otio");
    let st = cli().args(["demo", demo_path.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let demo = std::fs::read_to_string(&demo_path).unwrap();
    assert!(demo.contains("\"OTIO_SCHEMA\": \"Transition.1\""));
    assert!(demo.contains("\"OTIO_SCHEMA\": \"Marker.2\""));

    // roundtrip: байт-стабильно
    let rt_path = dir.join("demo-rt.otio");
    let st = cli()
        .args(["roundtrip", demo_path.to_str().unwrap(), rt_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(st.status.success());
    let err = String::from_utf8_lossy(&st.stderr);
    assert!(err.contains("byte-stable: true"), "{err}");
    assert_eq!(std::fs::read_to_string(&rt_path).unwrap(), demo);

    // convert: легаси-фикстура → .otio
    let legacy = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/demo.ktproj");
    let conv_path = dir.join("converted.otio");
    let st = cli().args(["convert", legacy, conv_path.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    assert!(std::fs::read_to_string(&conv_path).unwrap().contains("\"OTIO_SCHEMA\": \"Timeline.1\""));

    // render отвергает .ktproj с подсказкой
    let wav = dir.join("out.wav");
    let st = cli().args(["render", legacy, wav.to_str().unwrap()]).output().unwrap();
    assert!(!st.status.success());
    assert!(
        String::from_utf8_lossy(&st.stderr).contains("convert"),
        "подсказка на convert обязательна"
    );

    std::fs::remove_dir_all(&dir).ok();
}

/// Тест 290 (Р.43, п.298): дымовой тест инструмента `probe`.
///
/// Разбор жалобы пользователя («до проблем там был звук из сурса
/// "далеко" а при проблемах "салют"») занял бы минуты вместо часов, если
/// бы такой ответ можно было получить одной командой. Здесь держится сам
/// инструмент — все четыре его раздела на месте и он не падает на
/// настоящем проекте. САМО ПОВЕДЕНИЕ (слой не дотягивается до события вне
/// своего окна) сторожит t288 в kt-model: тест на печать не может его
/// заменить.
#[test]
fn t290_probe_reports_what_layers_changed() {
    let dir = std::env::temp_dir().join(format!("kt-cli-t290-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("probe.otio");

    // демо-проект несёт морфер; заменяем его на слой ДАЛЕКО от события
    let st = cli().args(["demo", path.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));

    let probe = |from: &str, to: &str| -> String {
        let out = cli()
            .args(["probe", path.to_str().unwrap(), from, to])
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).to_string()
    };

    // у демо-проекта на первой секунде есть события и нет чужого медиа
    let head = probe("0", "2");
    assert!(head.contains("effective clips"), "probe обязан показать события:\n{head}");
    assert!(
        head.contains("no foreign media in the order"),
        "в исправном месте чужого медиа быть не должно:\n{head}"
    );
    assert!(
        head.contains("clips with a time warp"),
        "probe обязан называть число событий с варпом — им и виден чужой слой:\n{head}"
    );

    // слой демо-проекта целится в события 7 и 9; в конце таймлайна их
    // нет, значит правок быть не должно — а события быть должны, иначе
    // «ничего не изменено» выполнялось бы само собой
    let far = probe("95", "99");
    assert!(far.contains("ClipId("), "в этом месте обязаны быть события:\n{far}");
    let changed = far.split("what morph layers changed here").nth(1).unwrap_or("");
    let changed = changed.split("\n\n").next().unwrap_or("");
    assert!(
        changed.contains("(nothing)"),
        "слой не имеет отношения к этому месту, а probe показал правки: {changed:?}"
    );

    // --time отвечает на вопрос «успевает ли движок ЗДЕСЬ» (Р.43, п.301):
    // после п.294 голодание кольца слышно тишиной, а не замедлением, и без
    // числа его не с чем сравнить
    let timed = cli()
        .args(["probe", path.to_str().unwrap(), "0", "2", "--time"])
        .output()
        .unwrap();
    assert!(timed.status.success(), "{}", String::from_utf8_lossy(&timed.stderr));
    let timed = String::from_utf8_lossy(&timed.stdout);
    assert!(
        timed.contains("% of real time"),
        "probe --time обязан назвать долю реального времени:\n{timed}"
    );
    // синтетика заведомо дешевле реального времени — «не успеваем» тут не
    // должно печататься, иначе строка ничего не значит
    assert!(
        !timed.contains("OVER BUDGET"),
        "синтетический демо-проект обязан укладываться в бюджет:\n{timed}"
    );
}

/// Тест 307 (п.316, Р.45): относительные пути медиа НЕ удваиваются.
/// `kt_otio::load_project` уже разрешает target_url от каталога проекта;
/// kt-cli поверх этого джойнил базу ВТОРОЙ раз, и вызов с относительным
/// путём к .otio (`kt-cli export sub/p.otio …`) терял медиа
/// («sub/sub/clip.mp4: decode failed»), тихо рендеря пустые слои.
#[test]
fn t307_relative_media_paths_resolve_once() {
    if !Command::new("ffmpeg").arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
    {
        eprintln!("skipped: ffmpeg binary unavailable");
        return;
    }
    let dir = std::env::temp_dir().join(format!("kt-cli-t307-{}", std::process::id()));
    let sub = dir.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let st = Command::new("ffmpeg")
        .args(["-y", "-v", "error", "-f", "lavfi", "-i", "testsrc2=size=320x180:rate=30"])
        .args(["-t", "1", "-pix_fmt", "yuv420p", "-c:v", "libx264"])
        .arg(sub.join("clip.mp4"))
        .output()
        .unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));

    let mut p = kt_model::Project::new(kt_model::ProjectSettings {
        fps: 30,
        width: 320,
        height: 180,
        sample_rate: 48000,
    });
    p.media.push(kt_model::MediaRef { id: kt_model::MediaId(1), path: "clip.mp4".into() });
    p.clips.push(kt_model::ClipEvent {
        id: kt_model::ClipId(1),
        link_group: None,
        kind: kt_model::ClipKind::Video,
        lane: 0,
        start: kt_time::Tick(0),
        duration: kt_time::Tick(90_000),
        source_in: kt_time::Tick(0),
        media: kt_model::MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx_bypass: vec![],
        fx_auto: vec![],
        fx: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    std::fs::write(
        sub.join("p.otio"),
        kt_otio::save_project(&p, &Default::default(), None),
    )
    .unwrap();

    // ОТНОСИТЕЛЬНЫЙ путь к проекту из каталога НАД ним — до правки медиа
    // резолвилось в sub/sub/clip.mp4 и слой молча пропадал
    let out = cli()
        .current_dir(&dir)
        .args(["export", "sub/p.otio", "sub/out.mp4", "libx264"])
        .output()
        .unwrap();
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(out.status.success(), "{all}");
    assert!(!all.contains("decode failed"), "медиа потеряно: {all}");
    assert!(all.contains("missing layers 0"), "пустые слои: {all}");
    let _ = std::fs::remove_dir_all(&dir);
}
