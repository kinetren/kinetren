//! Единый реестр FX + popup-пикер с поиском (п.3 v1.4): одно место выбора
//! эффектов для всех четырёх точек добавления (FX-редактор клипа ×2 вида,
//! morpher FX-батч ×2 цепочки), масштабируемое на сотни эффектов —
//! строка поиска (имя + keywords, без регистра) и группировка по
//! категориям. Шаблоны — те же, что были в кнопках/комбо (тест 48).

use crate::fx_editor::video_fx_templates;
use eframe::egui;
use kt_model::FxInstance;

pub struct FxEntry {
    pub name: &'static str,
    pub category: &'static str,
    pub audio: bool,
    /// Ключевые слова поиска (рус/англ, через пробел, в нижнем регистре).
    pub keywords: &'static str,
    pub template: FxInstance,
}

/// Категория и keywords видео-эффекта по имени из video_fx_templates.
fn video_meta(name: &str) -> (&'static str, &'static str) {
    match name {
        "Pan" => ("Движение", "пан зум pan zoom кадрирование crop"),
        "CamMotion" => ("Движение", "камера тряска shake handheld движение"),
        "Mirror" => ("Геометрия", "зеркало отражение flip mirror"),
        "Blur" => ("Размытие", "блюр размытие мягкость blur soft"),
        "Greenscreen" => ("Ключ", "хромакей ключ green chroma key прозрачность"),
        "Edges" => ("Стилизация", "контуры края edges обводка"),
        "Noise" => ("Стилизация", "шум зерно noise grain плёнка"),
        "VHS" => ("Стилизация", "вхс кассета ретро vhs glitch"),
        "Glow" => ("Свечение", "свечение блум glow bloom"),
        "EventGrid" => ("Стилизация", "сетка мозаика плитка grid tiles"),
        _ => ("Видео", ""),
    }
}

pub fn registry() -> &'static [FxEntry] {
    static REG: std::sync::OnceLock<Vec<FxEntry>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        let mut v = vec![
            FxEntry {
                name: "Gain",
                category: "Громкость",
                audio: true,
                keywords: "громкость усиление volume гейн db",
                template: FxInstance::Gain { db: 0.0 },
            },
            FxEntry {
                name: "Slowed+Reverb",
                category: "Тайм-стретч",
                audio: true,
                keywords: "замедление реверб слоуед slowed reverb warp скорость эхо",
                template: FxInstance::slowed_reverb(0.82, 0.1, 85.0),
            },
        ];
        for (name, tmpl) in video_fx_templates() {
            let (category, keywords) = video_meta(name);
            v.push(FxEntry { name, category, audio: false, keywords, template: tmpl });
        }
        v
    })
}

/// Фильтр реестра: вид (аудио/видео) + подстрока запроса в имени или
/// keywords, без регистра. Чистая функция — тест 48.
pub fn filter_entries<'a>(
    entries: &'a [FxEntry],
    audio: bool,
    query: &str,
) -> Vec<&'a FxEntry> {
    let q = query.trim().to_lowercase();
    entries
        .iter()
        .filter(|e| e.audio == audio)
        .filter(|e| {
            q.is_empty() || e.name.to_lowercase().contains(&q) || e.keywords.contains(&q)
        })
        .collect()
}

/// Кнопка «+ эффект…» с popup: поиск + список по категориям.
/// Возвращает шаблон выбранного эффекта.
pub fn fx_picker(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    audio: bool,
    query: &mut String,
) -> Option<FxInstance> {
    let mut picked = None;
    ui.menu_button("+ эффект…", |ui| {
        ui.set_min_width(230.0);
        let edit = egui::TextEdit::singleline(query).hint_text("поиск…").desired_width(210.0);
        let resp = ui.add(edit);
        // фокус в строку поиска при открытии меню
        if ui.input(|i| i.events.is_empty()) && !resp.has_focus() {
            resp.request_focus();
        }
        ui.separator();
        let list = filter_entries(registry(), audio, query);
        if list.is_empty() {
            ui.weak("ничего не найдено");
        }
        let mut last_cat = "";
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for e in list {
                if e.category != last_cat {
                    last_cat = e.category;
                    ui.label(egui::RichText::new(e.category).small().weak());
                }
                if ui.button(e.name).clicked() {
                    picked = Some(e.template.clone());
                    query.clear();
                    ui.close();
                }
            }
        });
        let _ = id_salt;
    });
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Тест 48 (п.3 v1.4): полнота реестра и идентичность шаблонов прежним
    /// кнопкам/комбо; фильтр находит по подстроке имени и по keyword и
    /// отделяет аудио от видео.
    #[test]
    fn t48_registry_complete_and_filter_works() {
        let reg = registry();
        // все 10 видео-шаблонов на месте и байт-в-байт равны прежним
        let old = video_fx_templates();
        assert_eq!(reg.iter().filter(|e| !e.audio).count(), old.len());
        for (name, tmpl) in &old {
            let e = reg.iter().find(|e| e.name == *name).expect("видео-эффект в реестре");
            assert!(!e.audio);
            assert_eq!(
                serde_json::to_string(&e.template).unwrap(),
                serde_json::to_string(tmpl).unwrap(),
                "шаблон {name} идентичен прежнему"
            );
        }
        // аудио-шаблоны прежних кнопок
        let gain = reg.iter().find(|e| e.name == "Gain").unwrap();
        assert!(gain.audio);
        assert_eq!(
            serde_json::to_string(&gain.template).unwrap(),
            serde_json::to_string(&FxInstance::Gain { db: 0.0 }).unwrap()
        );
        let sr = reg.iter().find(|e| e.name == "Slowed+Reverb").unwrap();
        assert!(sr.audio);
        assert_eq!(
            serde_json::to_string(&sr.template).unwrap(),
            serde_json::to_string(&FxInstance::slowed_reverb(0.82, 0.1, 85.0)).unwrap()
        );
        // имена согласованы с fx_display_name
        for e in reg {
            assert_eq!(crate::fx_editor::fx_display_name(&e.template), e.name);
        }
        // фильтр: по имени, по keyword, по виду, регистронезависимо
        assert_eq!(filter_entries(reg, true, "").len(), 2);
        assert_eq!(filter_entries(reg, false, "").len(), old.len());
        assert_eq!(filter_entries(reg, true, "РевЕрб").len(), 1, "кириллический keyword");
        assert_eq!(filter_entries(reg, false, "виш").len(), 0);
        assert_eq!(filter_entries(reg, false, "vhs").len(), 1);
        assert_eq!(filter_entries(reg, false, "blur").len(), 1);
        assert_eq!(filter_entries(reg, true, "gain").len(), 1);
        assert!(filter_entries(reg, true, "vhs").is_empty(), "видео-эффект не в аудио-списке");
    }
}
