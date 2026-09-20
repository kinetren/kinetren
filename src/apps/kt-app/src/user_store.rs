//! Пользовательское хранилище вне проекта (Р.48, п.343).
//!
//! До этого раунда такого места не было вовсе: eframe-persistence держал
//! язык и путь последнего проекта, всё остальное «удобство» жило внутри
//! файла проекта. На отсутствие общего места упирались четыре фичи
//! разведки (библиотека FX-пресетов, шаблоны титров, экспорт-пресеты,
//! раскладки клавиш) — поэтому модуль сделан ОБЩИМ: каталог, атомарная
//! запись и политика битого файла здесь, а конкретные библиотеки кладут
//! рядом свои файлы.
//!
//! Принципы, общие для всех будущих файлов:
//! - **проект самодостаточен**: библиотека — шаблоны вставки, а не
//!   ссылки. Потерялся файл библиотеки — проекты целы;
//! - **записи хранятся СЫРЫМИ** (`serde_json::Value`) рядом с разобранной
//!   копией: незнакомая запись из будущей версии пропускается при
//!   показе, но переживает сохранение — старая сборка не съедает работу
//!   новой;
//! - **файл новее поддерживаемой версии — только чтение**: перезаписывать
//!   то, чего мы не понимаем, нельзя;
//! - **битый файл не роняет программу**: он отодвигается в `.broken` при
//!   первой же записи, а библиотека начинает с пустой.

use std::path::PathBuf;

/// Версия формата файла пресетов, которую эта сборка умеет писать.
const PRESETS_VERSION: u64 = 1;

/// Каталог пользовательских данных — тот же, где лежит автосейв: одно
/// место для бэкапа. `KT_USER_STORE_DIR` подменяет его в тестах и живых
/// сценариях (иначе они писали бы в домашний каталог разработчика).
pub fn store_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("KT_USER_STORE_DIR") {
        return PathBuf::from(dir);
    }
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("kinotrain")
}

/// Разобранный пресет: имя и готовая к вставке цепочка.
#[derive(Debug, Clone, PartialEq)]
pub struct PresetRow {
    pub name: String,
    /// Ключи кривых ребейзнуты к нулю: пресет не помнит, из какого
    /// события его сняли.
    pub block: kt_model::ops::FxChainBlock,
    /// Куда его законно вставлять (считается по реестру эффектов).
    pub audio_ok: bool,
    pub video_ok: bool,
    /// Заводской — не переименовывается и не удаляется.
    pub factory: bool,
}

/// Заводские пресеты живут в бинаре и разбираются ТЕМ ЖЕ кодом, что и
/// пользовательский файл: значит загрузчик проверен на каждом запуске.
const FACTORY: &str = include_str!("../assets/fx-presets-factory.json");

/// Библиотека FX-пресетов.
pub struct FxPresetStore {
    /// Сырые записи пользовательского файла (форвард-совместимость).
    raw: Vec<serde_json::Value>,
    /// Разобранные: пользовательские, затем заводские.
    rows: Vec<PresetRow>,
    /// Файл не разобрался — при первой записи уедет в `.broken`.
    broken: bool,
    /// Файл новее, чем мы умеем: показываем, но не перезаписываем.
    readonly: bool,
    loaded: bool,
    /// Сколько записей не разобралось (незнакомый вид эффекта из будущей
    /// версии): они остаются в файле нетронутыми, но не показываются.
    skipped: usize,
    /// `None` — хранилище в памяти (тесты): домашний каталог не трогаем.
    dir: Option<PathBuf>,
}

impl Default for FxPresetStore {
    fn default() -> FxPresetStore {
        FxPresetStore {
            raw: Vec::new(),
            rows: Vec::new(),
            broken: false,
            readonly: false,
            loaded: false,
            skipped: 0,
            // под тестами — только память, если каталог не задан явно
            dir: (!cfg!(test) || std::env::var_os("KT_USER_STORE_DIR").is_some()).then(store_dir),
        }
    }
}

impl FxPresetStore {
    /// Хранилище в заданном каталоге (тесты с настоящей записью и
    /// живые сценарии через `KT_USER_STORE_DIR`).
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn at(dir: PathBuf) -> FxPresetStore {
        FxPresetStore { dir: Some(dir), ..Default::default() }
    }

    fn path(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|d| d.join("fx-presets.json"))
    }

    /// Разобранные пресеты (ленивая загрузка при первом обращении).
    pub fn rows(&mut self) -> &[PresetRow] {
        self.load();
        &self.rows
    }

    pub fn is_broken(&self) -> bool {
        self.broken
    }

    pub fn is_readonly(&self) -> bool {
        self.readonly
    }

    /// Сколько записей файла не удалось разобрать (незнакомые эффекты из
    /// будущей версии). Они остаются в файле нетронутыми.
    pub fn skipped(&self) -> usize {
        self.skipped
    }

    fn load(&mut self) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let text = self.path().and_then(|p| std::fs::read_to_string(p).ok());
        if let Some(text) = text {
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(v) => {
                    let ver = v.get("version").and_then(|x| x.as_u64()).unwrap_or(0);
                    self.readonly = ver > PRESETS_VERSION;
                    self.raw =
                        v.get("presets").and_then(|x| x.as_array()).cloned().unwrap_or_default();
                }
                Err(_) => self.broken = true,
            }
        }
        self.reload_rows();
    }

    /// Добавить пресет (имя уже уникально — его выбирает вызывающий).
    /// Ключи кривых ребейзятся к нулю: пресет не помнит своего донора.
    pub fn add(&mut self, name: &str, block: &kt_model::ops::FxChainBlock) -> std::io::Result<()> {
        self.load();
        let mut b = block.clone();
        for slot in &mut b.slots {
            slot.curves = kt_model::ops::rebase_curves(&slot.curves, b.src_in, kt_time::Tick(0));
        }
        b.src_in = kt_time::Tick(0);
        let entry = serde_json::json!({ "name": name, "slots": b.slots });
        self.raw.push(entry);
        self.reload_rows();
        self.save()
    }

    /// Переименовать пользовательский пресет.
    pub fn rename(&mut self, old: &str, new: &str) -> std::io::Result<()> {
        self.load();
        for v in &mut self.raw {
            if v.get("name").and_then(|n| n.as_str()) == Some(old) {
                v["name"] = serde_json::Value::String(new.to_string());
            }
        }
        self.reload_rows();
        self.save()
    }

    /// Удалить пользовательский пресет. Это правка ФАЙЛА, а не модели —
    /// undo проекта её не откатывает (и подсказка об этом говорит).
    pub fn remove(&mut self, name: &str) -> std::io::Result<()> {
        self.load();
        self.raw.retain(|v| v.get("name").and_then(|n| n.as_str()) != Some(name));
        self.reload_rows();
        self.save()
    }

    /// Занятые имена (пользовательские + заводские) — по ним строится
    /// уникальное имя при сохранении.
    pub fn taken_names(&mut self) -> Vec<String> {
        self.rows().iter().map(|r| r.name.clone()).collect()
    }

    fn reload_rows(&mut self) {
        let mut rows: Vec<PresetRow> = self.raw.iter().filter_map(|v| decode(v, false)).collect();
        self.skipped = self.raw.len() - rows.len();
        if let Ok(f) = serde_json::from_str::<serde_json::Value>(FACTORY) {
            let list = f.get("presets").and_then(|x| x.as_array()).cloned().unwrap_or_default();
            let before = rows.len();
            rows.extend(list.iter().filter_map(|v| decode(v, true)));
            debug_assert_eq!(
                rows.len() - before,
                list.len(),
                "the factory preset file must decode completely: it ships in the binary"
            );
        }
        self.rows = rows;
    }

    fn save(&mut self) -> std::io::Result<()> {
        let Some(path) = self.path() else { return Ok(()) };
        if self.readonly {
            return Err(std::io::Error::other("presets file is newer than this build"));
        }
        if self.broken && path.exists() {
            // битый файл не выбрасываем молча — отодвигаем рядом
            let _ = std::fs::rename(&path, path.with_extension("json.broken"));
            self.broken = false;
        }
        let doc = serde_json::json!({ "version": PRESETS_VERSION, "presets": self.raw });
        let text = serde_json::to_string_pretty(&doc)?;
        crate::app::write_project_atomic(&path, &text)
    }
}

/// Разобрать одну запись. Незнакомый вид эффекта, пустое имя или битые
/// кривые — запись пропускается (в файле она остаётся).
fn decode(v: &serde_json::Value, factory: bool) -> Option<PresetRow> {
    let name = v.get("name")?.as_str()?.trim().to_string();
    if name.is_empty() {
        return None;
    }
    let slots: Vec<kt_model::ops::FxSlotBlock> =
        serde_json::from_value(v.get("slots")?.clone()).ok()?;
    if slots.is_empty() {
        return None;
    }
    // Совместимость считается ПО РЕЕСТРУ: пресет виден там, где законны
    // ВСЕ его слоты (Slowed+Reverb двусторонний — попадает в оба списка).
    let entries = crate::fx_registry::registry();
    let kind_of = |f: &kt_model::FxInstance| -> Option<(bool, bool)> {
        let name = crate::fx_editor::fx_display_name(f);
        entries.iter().find(|e| e.name == name).map(|e| (e.audio, e.video))
    };
    let mut audio_ok = true;
    let mut video_ok = true;
    for s in &slots {
        let (a, vd) = kind_of(&s.fx)?;
        audio_ok &= a;
        video_ok &= vd;
    }
    Some(PresetRow {
        name,
        block: kt_model::ops::FxChainBlock { slots, src_in: kt_time::Tick(0), fx_off: false },
        audio_ok,
        video_ok,
        factory,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kt-presets-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn chain_with_curve() -> kt_model::ops::FxChainBlock {
        kt_model::ops::FxChainBlock {
            slots: vec![kt_model::ops::FxSlotBlock {
                fx: kt_model::FxInstance::blur(8),
                bypass: false,
                curves: vec![kt_model::ParamCurve {
                    param: "radius".into(),
                    ease: false,
                    keys: vec![
                        kt_model::AutoKey { t: kt_time::Tick(45_000), v: 1.0 },
                        kt_model::AutoKey { t: kt_time::Tick(135_000), v: 48.0 },
                    ],
                    source: Default::default(),
                }],
            }],
            src_in: kt_time::Tick(45_000),
            fx_off: false,
        }
    }

    /// Тест 344 (Р.48, п.343): пресет переживает круг через файл, и его
    /// ключи ребейзнуты к нулю — пресет не помнит своего донора.
    #[test]
    fn t344_preset_survives_the_file_roundtrip() {
        let dir = tmp_dir("roundtrip");
        let mut store = FxPresetStore::at(dir.clone());
        let factory = store.rows().iter().filter(|r| r.factory).count();
        assert!(factory > 0, "заводские пресеты обязаны разбираться");
        store.add("My chain", &chain_with_curve()).expect("save");

        // новый экземпляр читает файл с диска
        let mut back = FxPresetStore::at(dir.clone());
        let mine: Vec<_> = back.rows().iter().filter(|r| !r.factory).cloned().collect();
        assert_eq!(mine.len(), 1, "пресет обязан пережить запись и чтение");
        assert_eq!(mine[0].name, "My chain");
        assert_eq!(
            mine[0].block.slots[0].curves[0].keys.iter().map(|k| k.t.0).collect::<Vec<_>>(),
            vec![0, 90_000],
            "ключи обязаны храниться от нуля (пресет не помнит донора)"
        );
        assert!(mine[0].video_ok && !mine[0].audio_ok, "блюр — видео-пресет");

        // переименование и удаление доезжают до файла
        back.rename("My chain", "Renamed").expect("rename");
        let mut third = FxPresetStore::at(dir.clone());
        assert!(third.rows().iter().any(|r| r.name == "Renamed"));
        third.remove("Renamed").expect("remove");
        let mut fourth = FxPresetStore::at(dir.clone());
        assert!(fourth.rows().iter().all(|r| r.factory), "удалённого пресета быть не должно");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Тест 344b: чужие и битые файлы не роняют программу и не съедают
    /// чужую работу.
    #[test]
    fn t344b_broken_and_future_files_are_survivable() {
        // (1) битый JSON: пустая библиотека, флаг, и при первой записи
        //     файл отодвигается в .broken, а не пропадает
        let dir = tmp_dir("broken");
        std::fs::write(dir.join("fx-presets.json"), "{ not json").unwrap();
        let mut store = FxPresetStore::at(dir.clone());
        assert!(store.rows().iter().all(|r| r.factory), "из битого файла ничего не читается");
        assert!(store.is_broken());
        store.add("After crash", &chain_with_curve()).expect("save over broken");
        assert!(dir.join("fx-presets.json.broken").exists(), "битый файл обязан сохраниться рядом");
        let mut back = FxPresetStore::at(dir.clone());
        assert!(back.rows().iter().any(|r| r.name == "After crash"));

        // (2) незнакомый вид эффекта: запись пропускается при показе, но
        //     ПЕРЕЖИВАЕТ сохранение — старая сборка не съедает работу новой
        let dir = tmp_dir("future-kind");
        std::fs::write(
            dir.join("fx-presets.json"),
            r#"{"version":1,"presets":[
                {"name":"From the future","slots":[{"fx":{"kind":"time_machine","x":1}}]}
            ]}"#,
        )
        .unwrap();
        let mut store = FxPresetStore::at(dir.clone());
        assert!(store.rows().iter().all(|r| r.factory), "незнакомый вид не показывается");
        assert_eq!(store.skipped(), 1, "но он посчитан");
        store.add("Mine", &chain_with_curve()).expect("save");
        let text = std::fs::read_to_string(dir.join("fx-presets.json")).unwrap();
        assert!(text.contains("time_machine"), "чужая запись обязана пережить нашу запись");

        // (3) файл новее поддерживаемой версии — только чтение
        let dir = tmp_dir("future-version");
        std::fs::write(dir.join("fx-presets.json"), r#"{"version":99,"presets":[]}"#).unwrap();
        let mut store = FxPresetStore::at(dir.clone());
        let _ = store.rows();
        assert!(store.is_readonly(), "файл новее нас — читаем, но не пишем");
        assert!(store.add("Mine", &chain_with_curve()).is_err(), "запись обязана отказать");
    }

    /// Тест 347 (Р.48, п.346): куда пресет законно вставить, решает
    /// РЕЕСТР, а не догадка. Смешанная цепочка не видна ни в аудио-, ни
    /// в видео-окне — но морферу она законна, и он показывает всю
    /// библиотеку (в патче живут обе стороны).
    #[test]
    fn t347_preset_compatibility_comes_from_the_registry() {
        let mut store = FxPresetStore::default();
        let block = |fx: Vec<kt_model::FxInstance>| kt_model::ops::FxChainBlock {
            slots: fx
                .into_iter()
                .map(|f| kt_model::ops::FxSlotBlock { fx: f, bypass: false, curves: vec![] })
                .collect(),
            src_in: kt_time::Tick(0),
            fx_off: false,
        };
        store.add("audio only", &block(vec![kt_model::FxInstance::gain(-3.0)])).unwrap();
        store.add("video only", &block(vec![kt_model::FxInstance::blur(8)])).unwrap();
        store
            .add("both sides", &block(vec![kt_model::FxInstance::slowed_reverb(0.8, 0.3, 80.0)]))
            .unwrap();
        store
            .add(
                "mixed",
                &block(vec![kt_model::FxInstance::gain(-3.0), kt_model::FxInstance::blur(8)]),
            )
            .unwrap();
        let mut by = |name: &str| store.rows().iter().find(|r| r.name == name).cloned().unwrap();
        let g = by("audio only");
        assert!(g.audio_ok && !g.video_ok, "громкость — только звук");
        let b = by("video only");
        assert!(!b.audio_ok && b.video_ok, "блюр — только видео");
        let sr = by("both sides");
        assert!(sr.audio_ok && sr.video_ok, "Slowed+Reverb законен обеим сторонам");
        let m = by("mixed");
        assert!(
            !m.audio_ok && !m.video_ok,
            "смешанная цепочка не видна ни в аудио-, ни в видео-окне (морферу её покажут все)"
        );
    }

    /// Тест 344c: заводской файл — валидный, полный и разнообразный.
    /// Он в бинаре, значит разбирается тем же кодом на каждом запуске.
    #[test]
    fn t344c_factory_presets_are_valid() {
        let mut store = FxPresetStore::default();
        let rows: Vec<_> = store.rows().iter().filter(|r| r.factory).cloned().collect();
        assert!(rows.len() >= 4, "заводских пресетов должно быть несколько");
        let mut names = std::collections::BTreeSet::new();
        let mut any_audio = false;
        let mut any_video = false;
        for r in &rows {
            assert!(names.insert(r.name.clone()), "имена заводских уникальны: {}", r.name);
            assert!(
                r.block.slots.iter().any(|s| !s.curves.is_empty()),
                "{}: пресет обязан нести анимацию — ради неё библиотека и заводится",
                r.name
            );
            assert!(r.audio_ok || r.video_ok, "{}: пресет никуда не вставить", r.name);
            any_audio |= r.audio_ok;
            any_video |= r.video_ok;
        }
        assert!(any_audio && any_video, "заводские обязаны покрывать оба вида событий");
    }
}

// ---- рабочее место (Р.49, п.360) ------------------------------------------
//
// Ширина панели метров, зум, скролл и позиция курсора ехали В ФАЙЛЕ ПРОЕКТА.
// Следствие, заметное на любой совместной работе: открыл проект коллеги —
// потерял свою ширину метров; сохранил — навязал ему свою. Это РАБОЧЕЕ МЕСТО,
// а не монтаж.
//
// Имена и высоты дорожек, группы бина в проекте ОСТАЮТСЯ: они про ЭТОТ
// монтаж, и коллега должен получить их вместе с ним.
//
// В самом .otio прежние значения остаются passthrough — что прочитали, то и
// пишем. Байт-стабильность цела, GOLDEN не двигаются, а старая сборка,
// открыв файл, найдёт в нём ровно то, что там было.

/// Версия файла рабочего места.
const WORKSPACE_VERSION: u64 = 1;

/// Сколько проектов помним. Список — не история, а удобство; после сотни
/// монтажей самые старые записи никому не нужны.
const WORKSPACE_KEEP: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProjectView {
    pub view_start: f64,
    pub ticks_per_px: f64,
    pub cursor: i64,
}

/// Рабочее место пользователя: общее (ширина метров) и по проектам (вид).
#[derive(Debug, Clone, Default)]
pub struct Workspace {
    /// 0 — не задано, брать значение из файла проекта (первый запуск после
    /// переезда) или умолчание.
    pub meters_w: f32,
    /// Вид по проекту, свежие впереди.
    views: Vec<(String, ProjectView)>,
    /// Файл прочитан из будущей версии — только чтение.
    readonly: bool,
}

impl Workspace {
    pub fn view_of(&self, project: Option<&std::path::Path>) -> Option<ProjectView> {
        let key = project?.to_string_lossy().into_owned();
        self.views.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
    }

    pub fn set_view(&mut self, project: Option<&std::path::Path>, v: ProjectView) {
        let Some(key) = project.map(|p| p.to_string_lossy().into_owned()) else { return };
        self.views.retain(|(k, _)| *k != key);
        self.views.insert(0, (key, v));
        self.views.truncate(WORKSPACE_KEEP);
    }

    pub fn is_readonly(&self) -> bool {
        self.readonly
    }
}

fn workspace_path() -> PathBuf {
    store_dir().join("workspace.json")
}

pub fn load_workspace() -> Workspace {
    load_workspace_at(&workspace_path())
}

pub fn load_workspace_at(path: &std::path::Path) -> Workspace {
    let Ok(text) = std::fs::read_to_string(path) else { return Workspace::default() };
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else {
        // битый файл не роняет программу и не съедает работу: он будет
        // отодвинут в `.broken` при первой записи
        return Workspace::default();
    };
    let ver = v.get("version").and_then(|x| x.as_u64()).unwrap_or(0);
    let mut w = Workspace { readonly: ver > WORKSPACE_VERSION, ..Default::default() };
    w.meters_w = v.get("meters_w").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
    if let Some(arr) = v.get("views").and_then(|x| x.as_array()) {
        for e in arr.iter().take(WORKSPACE_KEEP) {
            let (Some(path), Some(vs), Some(tp)) = (
                e.get("project").and_then(|x| x.as_str()),
                e.get("view_start").and_then(|x| x.as_f64()),
                e.get("ticks_per_px").and_then(|x| x.as_f64()),
            ) else {
                continue;
            };
            w.views.push((
                path.to_string(),
                ProjectView {
                    view_start: vs,
                    ticks_per_px: tp,
                    cursor: e.get("cursor").and_then(|x| x.as_i64()).unwrap_or(0),
                },
            ));
        }
    }
    w
}

pub fn save_workspace(w: &Workspace) {
    save_workspace_at(&workspace_path(), w);
}

pub fn save_workspace_at(path: &std::path::Path, w: &Workspace) {
    if w.readonly {
        return; // файл новее нашего — не переписываем то, чего не понимаем
    }
    let views: Vec<serde_json::Value> = w
        .views
        .iter()
        .map(|(k, v)| {
            serde_json::json!({
                "project": k,
                "view_start": v.view_start,
                "ticks_per_px": v.ticks_per_px,
                "cursor": v.cursor,
            })
        })
        .collect();
    let doc = serde_json::json!({
        "version": WORKSPACE_VERSION,
        "meters_w": w.meters_w,
        "views": views,
    });
    if let Some(d) = path.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    // тем же кодом, что и проект: обрыв на середине не оставляет обрубка
    let _ = kt_otio::write_atomic(path, &format!("{doc:#}\n"));
}
