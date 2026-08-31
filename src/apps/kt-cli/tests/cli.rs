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
