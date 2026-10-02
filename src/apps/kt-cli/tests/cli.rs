//! Тест 151 (п.151 porting-notes, Р.24; Р.70 — свой формат): kt-cli на
//! файле проекта `.kt` и двух дверях обмена.
//!
//! new/demo пишут файл проекта своего формата; export-otio даёт
//! самодостаточный OTIO, roundtrip на нём байт-стабилен; import читает
//! легаси-.ktproj и .otio в `.kt`; render/probe отвергают файл обмена и
//! легаси с подсказкой на import.

use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_kt-cli"))
}

#[test]
fn t151_cli_otio_workflows() {
    let dir = std::env::temp_dir().join(format!("kt-cli-t151-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    // new → файл проекта СВОЕГО формата: признак и версия первой строкой
    let new_path = dir.join("empty.kt");
    let st = cli().args(["new", new_path.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let text = std::fs::read_to_string(&new_path).unwrap();
    assert!(text.starts_with("{\n  \"kinotrain\": 1"), "{text:.80}");
    assert!(!text.contains("OTIO_SCHEMA"), "файл проекта — не файл обмена");

    // demo → богатый проект (переход, метки, регион, морфер)
    let demo_path = dir.join("demo.kt");
    let st = cli().args(["demo", demo_path.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let demo = std::fs::read_to_string(&demo_path).unwrap();
    assert!(demo.contains("\"transitions\""));
    assert!(demo.contains("\"markers\""));

    // export-otio → самодостаточный файл обмена
    let otio_path = dir.join("demo.otio");
    let st = cli()
        .args(["export-otio", demo_path.to_str().unwrap(), otio_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let otio = std::fs::read_to_string(&otio_path).unwrap();
    assert!(otio.starts_with("{\n    \"OTIO_SCHEMA\": \"Timeline.1\""));
    assert!(otio.contains("\"OTIO_SCHEMA\": \"Transition.1\""));
    assert!(otio.contains("\"OTIO_SCHEMA\": \"Marker.2\""));

    // roundtrip двери обмена: байт-стабильно
    let rt_path = dir.join("demo-rt.otio");
    let st = cli()
        .args(["roundtrip", otio_path.to_str().unwrap(), rt_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(st.status.success());
    let err = String::from_utf8_lossy(&st.stderr);
    assert!(err.contains("byte-stable: true"), "{err}");
    assert_eq!(std::fs::read_to_string(&rt_path).unwrap(), otio);

    // import: файл обмена → файл проекта, и он поле-в-поле равен исходному
    let back_path = dir.join("back.kt");
    let st = cli()
        .args(["import", otio_path.to_str().unwrap(), back_path.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let back = kt_model::project_file::from_text(&std::fs::read_to_string(&back_path).unwrap())
        .unwrap()
        .project;
    let orig = kt_model::project_file::from_text(&demo).unwrap().project;
    assert_eq!(back.clips, orig.clips, "круг kt → otio → kt теряет события");
    assert_eq!(back.transitions, orig.transitions);
    assert_eq!(back.morphers, orig.morphers);

    // import: легаси-фикстура → файл проекта
    let legacy = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/demo.ktproj");
    let conv_path = dir.join("converted.kt");
    let st = cli().args(["import", legacy, conv_path.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    assert!(std::fs::read_to_string(&conv_path).unwrap().starts_with("{\n  \"kinotrain\": "));

    // render/probe отвергают легаси и файл обмена с подсказкой на import
    let wav = dir.join("out.wav");
    for (what, input) in [("legacy", legacy.to_string()), ("otio", otio_path.display().to_string())] {
        let st = cli().args(["render", &input, wav.to_str().unwrap()]).output().unwrap();
        assert!(!st.status.success(), "{what}: render обязан отказать");
        let said = String::from_utf8_lossy(&st.stderr);
        assert!(said.contains("kt-cli import"), "{what}: подсказка на import обязательна: {said}");
        assert!(!said.contains("convert"), "{what}: прежняя подсказка «convert» мертва: {said}");
    }
    // import файла проекта — «уже проект», а не тихий успех
    let st = cli()
        .args(["import", demo_path.to_str().unwrap(), dir.join("x.kt").to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!st.status.success());
    assert!(String::from_utf8_lossy(&st.stderr).contains("already a Kinotrain project"));

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
    let path = dir.join("probe.kt");

    // демо-проект несёт морфер; заменяем его на слой ДАЛЕКО от события
    let st = cli().args(["demo", path.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));

    let probe = |from: &str, to: &str| -> String {
        let out = cli().args(["probe", path.to_str().unwrap(), from, to]).output().unwrap();
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
    // Тест 318 (Р.46, п.322): по жалобе «зум всё равно мылит» смотреть
    // нужно на ступень декода, и probe обязан её называть — вместе с
    // нативом файла, из-за которого ступени может не быть вовсе
    assert!(
        head.contains("video decode order"),
        "probe обязан показывать заказ ВИДЕО, не только звук:\n{head}"
    );
    assert!(head.contains("stage x"), "в заказе видео обязана быть ступень декода:\n{head}");

    // Тест 452b (Р.50, Б6): по вложенному монтажу «войди в N уровней,
    // чтобы понять, откуда кадр» — это наш минус. Дерево лент и ПУТЬ
    // события печатаются сразу.
    let nested = dir.join("nested.kt");
    {
        let text = std::fs::read_to_string(&path).unwrap();
        let mut pr = kt_model::project_file::from_text(&text).unwrap().project;
        let mut n = kt_model::ops::NestCounters::of(&pr);
        let ids: Vec<kt_model::ClipId> =
            pr.clips.iter().filter(|c| c.start.0 < 2 * 90_000).map(|c| c.id).collect();
        let (cmds, rep) = kt_model::ops::collapse_into_sequence(&pr, &ids, "Заставка", n.ids());
        let mut m = kt_model::TimelineModel::new(pr);
        m.apply_batch(cmds).expect("сворачивание на демо-проекте");
        assert!(rep.seq.is_some());
        pr = m.project().clone();
        std::fs::write(&nested, kt_model::project_file::to_text(&pr, None)).unwrap();
    }
    let out = cli().args(["probe", nested.to_str().unwrap(), "0", "2"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let nest_head = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(nest_head.contains("nested reels"), "probe обязан печатать дерево лент:\n{nest_head}");
    assert!(
        nest_head.contains("Заставка") && nest_head.contains("places"),
        "в дереве лент обязаны быть имя и число мест:\n{nest_head}"
    );
    assert!(
        nest_head.contains("is a NESTED REEL"),
        "probe обязан называть ПУТЬ события вложения:\n{nest_head}"
    );

    // Р.56 (п.448): у РАСТЯНУТОГО экземпляра окно содержимого не равно его
    // длине на таймлайне — печатать длину значит врать ровно в том месте,
    // ради которого probe и запускают («тут кадр из другого места»).
    let warped = dir.join("nested_warped.kt");
    {
        let text = std::fs::read_to_string(&nested).unwrap();
        let mut pr = kt_model::project_file::from_text(&text).unwrap().project;
        let id = pr
            .clips
            .iter()
            .find(|c| pr.sequence(c.media).is_some() && c.kind == kt_model::ClipKind::Video)
            .map(|c| c.id)
            .expect("экземпляр ленты есть");
        let c = pr.clips.iter_mut().find(|c| c.id == id).unwrap();
        c.reversed = true;
        c.fx.push(kt_model::FxInstance::slowed_reverb(0.5, 0.0, 85.0));
        pr.validate().expect("зеркальный замедленный экземпляр законен");
        std::fs::write(&warped, kt_model::project_file::to_text(&pr, None)).unwrap();
    }
    let out = cli().args(["probe", warped.to_str().unwrap(), "0", "2"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let warp_head = String::from_utf8_lossy(&out.stdout).to_string();
    let line = warp_head
        .lines()
        .find(|l| l.contains("is a NESTED REEL") && l.contains("REVERSED"))
        .unwrap_or_else(|| panic!("зеркальный экземпляр не назван зеркальным:\n{warp_head}"));
    assert!(
        line.contains("warped") && line.contains("consumed"),
        "у растянутого экземпляра probe обязан печатать ФАКТИЧЕСКИ потреблённое \
         окно ленты, а не длину события: {line}"
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
    let timed = cli().args(["probe", path.to_str().unwrap(), "0", "2", "--time"]).output().unwrap();
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
        pitch_engine: Default::default(),

        default_shape: Default::default(),

        conform: Default::default(),

        settings_explicit: false,
    });
    p.media.push(kt_model::MediaRef::new(kt_model::MediaId(1), "clip.mp4"));
    p.clips.push(kt_model::ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
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
    std::fs::write(sub.join("p.kt"), kt_model::project_file::to_text(&p, None)).unwrap();

    // ОТНОСИТЕЛЬНЫЙ путь к проекту из каталога НАД ним — до правки медиа
    // резолвилось в sub/sub/clip.mp4 и слой молча пропадал
    let out = cli()
        .current_dir(&dir)
        .args(["export", "sub/p.kt", "sub/out.mp4", "libx264"])
        .output()
        .unwrap();
    let all =
        format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    assert!(out.status.success(), "{all}");
    assert!(!all.contains("decode failed"), "медиа потеряно: {all}");
    assert!(all.contains("missing layers 0"), "пустые слои: {all}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 387 (Р.49 А11): kt-cli не затирает существующий файл молча. Звёздочки
/// и диалога у него нет — помешать некому; поэтому перезапись только по
/// явному `--force`.
#[test]
fn t387_cli_does_not_overwrite_without_force() {
    let dir = std::env::temp_dir().join(format!("kt-cli-t387-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("p.kt");
    std::fs::write(&path, "работа пользователя").unwrap();

    for cmd in [["new"], ["demo"]] {
        let st = cli().args([cmd[0], path.to_str().unwrap()]).output().unwrap();
        assert!(!st.status.success(), "{} затёр существующий файл", cmd[0]);
        let err = String::from_utf8_lossy(&st.stderr);
        assert!(err.contains("--force"), "{}: не сказано, чем разрешить: {err}", cmd[0]);
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "работа пользователя",
            "{} всё-таки переписал файл",
            cmd[0]
        );
    }

    // с --force пишет, и пишет АТОМАРНО: временного файла не остаётся
    let st = cli().args(["new", path.to_str().unwrap(), "--force"]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    assert!(std::fs::read_to_string(&path).unwrap().contains("\"kinotrain\""));
    let leftovers: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".tmp"))
        .collect();
    assert!(leftovers.is_empty(), "остался временный файл: {leftovers:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 388 (Р.49 А11): `demo` и `render` работают подряд БЕЗ единого
/// медиафайла. Восемь путей `media/*`, на которые демо ссылалось, не
/// существуют нигде в поставке — то есть демо не рендерилось ни у кого, кто
/// не собрал эти файлы сам. `synth:` — полноправный источник во всей
/// программе, и теперь он в демо по умолчанию.
#[test]
fn t388_demo_renders_without_any_media_files() {
    let dir = std::env::temp_dir().join(format!("kt-cli-t388-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let proj = dir.join("demo.kt");
    let wav = dir.join("demo.wav");

    let st = cli().args(["demo", proj.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let text = std::fs::read_to_string(&proj).unwrap();
    assert!(text.contains("synth:"), "демо по умолчанию ссылается не на синтетику");
    assert!(!text.contains("media/"), "в демо остались несуществующие файлы: {text:.400}");

    let st =
        cli().args(["render", proj.to_str().unwrap(), wav.to_str().unwrap()]).output().unwrap();
    assert!(
        st.status.success(),
        "демо не рендерится из коробки: {}",
        String::from_utf8_lossy(&st.stderr)
    );
    let len = std::fs::metadata(&wav).unwrap().len();
    assert!(len > 1_000_000, "звук подозрительно короткий: {len} байт");

    // прежние имена файлов остаются доступны явным флагом
    let files = dir.join("demo-files.kt");
    let st = cli().args(["demo", files.to_str().unwrap(), "--files"]).output().unwrap();
    assert!(st.status.success());
    assert!(std::fs::read_to_string(&files).unwrap().contains("media/"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 389 (Р.49 А11): рендер с недоступным медиа — ОТКАЗ, а не молчаливая
/// тишина в выходном файле; `--allow-missing` переключает это осознанно.
#[test]
fn t389_render_refuses_missing_media_unless_allowed() {
    let dir = std::env::temp_dir().join(format!("kt-cli-t389-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let proj = dir.join("p.kt");
    let wav = dir.join("p.wav");
    let st = cli().args(["demo", proj.to_str().unwrap(), "--files"]).output().unwrap();
    assert!(st.status.success());

    let st =
        cli().args(["render", proj.to_str().unwrap(), wav.to_str().unwrap()]).output().unwrap();
    assert!(!st.status.success(), "рендер с отсутствующими медиа обязан отказать");
    assert!(!wav.exists(), "написан файл, в котором вместо материала тишина");

    let st = cli()
        .args(["render", proj.to_str().unwrap(), wav.to_str().unwrap(), "--allow-missing"])
        .output()
        .unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    assert!(wav.exists(), "с --allow-missing рендер обязан пройти");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 727 (Р.62): probe называет правило приведения, вид источника,
/// размер декода и рамку каждого видео-события в окне — первое, что
/// спросят по жалобе «картинка растянута/обрезана».
#[test]
fn t727_probe_reports_conform_geometry() {
    let dir = std::env::temp_dir().join(format!("kt-cli-t727-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("probe.kt");
    let st = cli().args(["demo", path.to_str().unwrap()]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));
    let out = cli().args(["probe", path.to_str().unwrap(), "0", "2"]).output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(text.contains("conform"), "probe обязан печатать приведение:\n{text}");
    assert!(text.contains("rule="), "…с правилом события:\n{text}");
    assert!(text.contains("decode="), "…и размером декода:\n{text}");
}

/// Тест 740b (Р.62, change B): kt-cli говорит, ЧТО НЕ ПОПАДЁТ В ФАЙЛ, —
/// заглушённые дорожки и активное соло, той же формулировкой, что окно
/// рендера, и ДО рендера. Отказа нет: mute и solo — содержимое проекта
/// (решение владельца), но молчать о них нельзя.
#[test]
fn t740b_cli_reports_muted_and_solo_before_render() {
    let dir = std::env::temp_dir().join(format!("kt-cli-t740b-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("demo.kt");
    let st = cli().args(["demo", path.to_str().unwrap(), "--force"]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));

    // без заглушённых дорожек — ни строки лишней
    let wav = dir.join("out.wav");
    let out = cli()
        .args(["render", path.to_str().unwrap(), wav.to_str().unwrap(), "--allow-missing"])
        .output()
        .unwrap();
    let clean = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(!clean.contains("muted tracks"), "лишняя строка на проекте без mute: {clean}");

    // глушим первую аудиодорожку и включаем соло на видео — обе строки
    let text = std::fs::read_to_string(&path).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    {
        // дорожки-объекты модели: порядок строк сверху вниз (видео, затем аудио)
        v["project"]["tracks"] = serde_json::json!([
            {"kind": "video", "id": 1, "solo": true},
            {"kind": "audio", "id": 2, "mute": true},
        ]);
    }
    let muted_path = dir.join("muted.kt");
    std::fs::write(&muted_path, serde_json::to_string_pretty(&v).unwrap()).unwrap();

    let out = cli()
        .args(["render", muted_path.to_str().unwrap(), wav.to_str().unwrap(), "--allow-missing"])
        .output()
        .unwrap();
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "рендер обязан идти, а не отказывать: {said}");
    assert!(said.contains("muted tracks"), "kt-cli молчит о заглушённой дорожке: {said}");
    assert!(said.contains("solo is on"), "kt-cli молчит об активном соло: {said}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 746 (Р.62, change B, 4.6): `probe` печатает ДОРОЖКИ. Жалобы
/// «дорожки не слышно» и «картинка светлее, чем в превью» разбираются
/// именно этими строками: выключатели, уровень, режим наложения и есть ли
/// у дорожки стадия (цепочка на сумме событий у аудио, неявная лента у
/// видео). Печатаются ВСЕ строки: «дорожка на месте и ничем не тронута» —
/// такой же ответ, как «заглушена».
#[test]
fn t746_probe_prints_tracks_with_switches_level_blend_and_stage() {
    let dir = std::env::temp_dir().join(format!("kt-cli-t746-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("demo.kt");
    let st = cli().args(["demo", path.to_str().unwrap(), "--force"]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));

    let plain = String::from_utf8_lossy(
        &cli().args(["probe", path.to_str().unwrap(), "0", "5"]).output().unwrap().stdout,
    )
    .to_string();
    assert!(plain.contains("-- tracks --"), "у probe нет раздела дорожек: {plain}");
    assert!(plain.contains("V1"), "дорожка без новшеств не показана — её тоже надо видеть");
    assert!(!plain.contains("MUTE"), "выключатель на нетронутой дорожке: {plain}");
    assert!(!plain.contains("stage:"), "стадия на дорожке без цепочки, уровня и режима");

    // дорожка с уровнем, режимом, глушением и соло
    let text = std::fs::read_to_string(&path).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    {
        v["project"]["tracks"] = serde_json::json!([
            {"kind": "video", "id": 1, "name": "Титры", "opacity": 0.5,
             "blend": "multiply", "solo": true},
            {"kind": "audio", "id": 2, "gain_db": -6.0, "mute": true},
        ]);
    }
    let p2 = dir.join("tracks.kt");
    std::fs::write(&p2, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    let said = String::from_utf8_lossy(
        &cli().args(["probe", p2.to_str().unwrap(), "0", "5"]).output().unwrap().stdout,
    )
    .to_string();
    let tracks = said
        .split("-- tracks --")
        .nth(1)
        .unwrap_or("")
        .split("-- ")
        .next()
        .unwrap_or("")
        .to_string();
    for want in [
        "Титры",       // имя дорожки, а не формула
        "opacity 0.50", // уровень видео
        "Multiply",    // режим наложения
        "SOLO",
        "-6.0 dB", // уровень аудио
        "MUTE",
        "stage:audio", // −6 дБ на аудио = стадия дорожки
        "stage:video", // непрозрачность и режим = неявная лента
    ] {
        assert!(tracks.contains(want), "в разделе дорожек нет «{want}»:\n{tracks}");
    }
    assert!(
        said.contains("solo active") && said.contains("muted:"),
        "probe не сказал, что из этого не попадёт в файл: {said}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// Тест 748b (Р.63 п.2): `probe` называет конец материала.
///
/// По жалобе «в конце тишина» первым делом спрашивают probe — и он обязан
/// сказать, что материал кончился раньше события, а не заставлять считать
/// это в уме по длине файла и скорости.
#[test]
fn t748b_probe_names_the_material_end() {
    let dir = std::env::temp_dir().join(format!("kt-cli-t748b-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("edge.kt");
    let st = cli().args(["demo", path.to_str().unwrap(), "--force"]).output().unwrap();
    assert!(st.status.success(), "{}", String::from_utf8_lossy(&st.stderr));

    // событие ДЛИННЕЕ материала: синтетический источник длится час
    // (`SYNTH_DURATION`), поэтому растягиваем событие на полтора часа
    let text = std::fs::read_to_string(&path).unwrap();
    let mut v: serde_json::Value = serde_json::from_str(&text).unwrap();
    let mut patched = 0usize;
    fn stretch(x: &mut serde_json::Value, n: &mut usize) {
        match x {
            serde_json::Value::Object(m) => {
                // событие модели: у него есть и `source_in`, и `duration`
                if m.contains_key("source_in")
                    && let Some(val) = m.get_mut("duration")
                {
                    *val = serde_json::json!(5400i64 * 90_000);
                    *n += 1;
                }
                for (_, val) in m.iter_mut() {
                    stretch(val, n);
                }
            }
            serde_json::Value::Array(a) => a.iter_mut().for_each(|val| stretch(val, n)),
            _ => {}
        }
    }
    stretch(&mut v, &mut patched);
    assert!(patched > 0, "сцена вырождена: событий в демо-проекте нет");
    let long = dir.join("long.kt");
    std::fs::write(&long, serde_json::to_string_pretty(&v).unwrap()).unwrap();

    let out = cli().args(["probe", long.to_str().unwrap(), "0", "2"]).output().unwrap();
    let said = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        said.contains("material ends at"),
        "probe молчит о конце материала — по жалобе «в конце тишина» спрашивать нечего:\n{said}"
    );
    assert!(
        said.contains("not starvation"),
        "probe не сказал, что тишина за краем — не голодание: {said}"
    );

    // событие ВНУТРИ материала — ни строки лишней (правило 9)
    let out = cli().args(["probe", path.to_str().unwrap(), "0", "2"]).output().unwrap();
    let clean = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(
        !clean.contains("material ends at"),
        "у события внутри материала строки о конце быть не должно: {clean}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
