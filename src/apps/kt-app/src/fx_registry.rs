//! Единый реестр FX + popup-пикер с поиском (п.3 v1.4): одно место выбора
//! эффектов для всех четырёх точек добавления (FX-редактор клипа ×2 вида,
//! morpher FX-батч ×2 цепочки), масштабируемое на сотни эффектов —
//! строка поиска (имя + keywords, без регистра) и группировка по
//! категориям. Шаблоны — те же, что были в кнопках/комбо (тест 48).

use crate::fx_editor::video_fx_templates;
use crate::i18n::{Key, k, tr, tr_en};
use eframe::egui;
use kt_model::FxInstance;

pub struct FxEntry {
    /// Имя эффекта НЕ переводится: это отраслевой термин, по нему эффект ищут
    /// в любой локали (и он же сверяется с `fx_display_name`, тест 48).
    pub name: &'static str,
    pub category: Key,
    /// В каком пикере предлагать. Обычно ровно одно из двух; двусторонним
    /// бывает ТОЛЬКО варп времени (Р.37): он не пиксельный фильтр, а
    /// правило выбора исходного кадра, и осмыслен на обоих видах.
    pub audio: bool,
    pub video: bool,
    /// Ключевые слова поиска. Ключ каталога, а не текст: реестр от языка не
    /// зависит и `OnceLock` не пересобирается при переключении.
    pub keywords: Option<Key>,
    pub template: FxInstance,
}

/// Категория и keywords видео-эффекта по имени из video_fx_templates.
fn video_meta(name: &str) -> (Key, Option<Key>) {
    // Полными путями намеренно: тест 125 ищет ключ каталога по пути `k::…`
    // и сокращённый `use` спрятал бы использование.
    match name {
        "Pan" => (k::fxreg::cat::MOTION, Some(k::fxreg::kw::PAN)),
        "Mirror" => (k::fxreg::cat::GEOMETRY, Some(k::fxreg::kw::MIRROR)),
        "Blur" => (k::fxreg::cat::BLUR, Some(k::fxreg::kw::BLUR)),
        "Greenscreen" => (k::fxreg::cat::KEY, Some(k::fxreg::kw::GREENSCREEN)),
        "Edges" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::EDGES)),
        "Noise" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::NOISE)),
        "VHS" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::VHS)),
        "Glow" => (k::fxreg::cat::GLOW, Some(k::fxreg::kw::GLOW)),
        "EventGrid" => (k::fxreg::cat::STYLE, Some(k::fxreg::kw::EVENTGRID)),
        "Pan 3D" => (k::fxreg::cat::THREE_D, Some(k::fxreg::kw::PAN3D)),
        _ => (k::fxreg::cat::VIDEO, None),
    }
}

pub fn registry() -> &'static [FxEntry] {
    static REG: std::sync::OnceLock<Vec<FxEntry>> = std::sync::OnceLock::new();
    REG.get_or_init(|| {
        let mut v = vec![
            FxEntry {
                name: "Gain",
                category: k::fxreg::cat::VOLUME,
                audio: true,
                video: false,
                keywords: Some(k::fxreg::kw::GAIN),
                template: FxInstance::gain(0.0),
            },
            FxEntry {
                name: "Slowed+Reverb",
                category: k::fxreg::cat::TIMESTRETCH,
                audio: true,
                // ДВУСТОРОННИЙ (Р.37 п.2): на видео-клипе он растягивает
                // время, то есть меняет выбор исходного кадра
                video: true,
                keywords: Some(k::fxreg::kw::SLOWEDREVERB),
                template: FxInstance::slowed_reverb(0.82, 0.1, 85.0),
            },
        ];
        for (name, tmpl) in video_fx_templates() {
            let (category, keywords) = video_meta(name);
            v.push(FxEntry { name, category, audio: false, video: true, keywords, template: tmpl });
        }
        v
    })
}

/// Фильтр реестра: вид (аудио/видео) + подстрока запроса в имени или
/// keywords, без регистра. Чистая функция — тест 48.
///
/// Ищет по ключевым словам ТЕКУЩЕГО языка И по английским: иначе выбравший
/// китайский не смог бы найти «blur», хотя имя эффекта именно такое.
/// Значения keywords лежат в каталоге уже в нижнем регистре.
pub fn filter_entries<'a>(
    entries: &'a [FxEntry],
    audio: bool,
    query: &str,
) -> Vec<&'a FxEntry> {
    let q = query.trim().to_lowercase();
    entries
        .iter()
        .filter(|e| if audio { e.audio } else { e.video })
        .filter(|e| {
            q.is_empty()
                || e.name.to_lowercase().contains(&q)
                || e.keywords.is_some_and(|kw| {
                    tr(kw).contains(&q) || tr_en(kw).contains(&q)
                })
        })
        .collect()
}

/// Кнопка «+ эффект…» с popup: поиск + список по категориям.
/// Что выбрали в пикере.
pub enum Picked {
    /// Приколотая голова полосы (у морфера — подмена источника).
    Head,
    Fx(FxInstance),
}

/// Пикер, у которого ПЕРВОЙ строкой стоит голова полосы. В `registry()`
/// её нет намеренно: t48 сверяет реестр с шаблонами эффектов, а у
/// псевдо-слота шаблона не бывает — он не эффект, а часть морфера.
pub fn fx_picker_with_head(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    audio: bool,
    query: &mut String,
    head: Option<&str>,
) -> Option<Picked> {
    let mut picked = None;
    ui.menu_button(tr(k::fxreg::ADD), |ui| {
        ui.set_min_width(230.0);
        let edit = egui::TextEdit::singleline(query).hint_text(tr(k::fxreg::SEARCH)).desired_width(210.0);
        let resp = ui.add(edit);
        // фокус в строку поиска при открытии меню
        if ui.input(|i| i.events.is_empty()) && !resp.has_focus() {
            resp.request_focus();
        }
        ui.separator();
        // голова — своим цветом и ПЕРВОЙ («поднять в списке fx»);
        // ищется той же строкой поиска, что и эффекты
        if let Some(h) = head
            && (query.is_empty() || h.to_lowercase().contains(&query.to_lowercase()))
        {
            let text = egui::RichText::new(h).color(crate::theme::MORPH_SLOT);
            if ui.button(text).clicked() {
                picked = Some(Picked::Head);
                query.clear();
                ui.close();
            }
            ui.separator();
        }
        let list = filter_entries(registry(), audio, query);
        if list.is_empty() && head.is_none() {
            ui.weak(tr(k::fxreg::NOTHING));
        }
        let mut last_cat: Option<Key> = None;
        egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
            for e in list {
                if last_cat != Some(e.category) {
                    last_cat = Some(e.category);
                    ui.label(egui::RichText::new(tr(e.category)).small().weak());
                }
                if ui.button(e.name).clicked() {
                    picked = Some(Picked::Fx(e.template.clone()));
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
        // ДВУСТОРОННЯЯ запись ровно одна — варп времени (Р.37). Он не
        // пиксельный фильтр, а правило выбора исходного кадра, поэтому
        // осмыслен на обоих видах; всё остальное обязано остаться в своей
        // корзине, иначе деление вырождается
        assert_eq!(
            reg.iter().filter(|e| e.audio && e.video).map(|e| e.name).collect::<Vec<_>>(),
            ["Slowed+Reverb"],
            "двусторонним бывает только растяжение времени"
        );
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
            serde_json::to_string(&FxInstance::gain(0.0)).unwrap()
        );
        let sr = reg.iter().find(|e| e.name == "Slowed+Reverb").unwrap();
        assert!(sr.audio);
        assert_eq!(
            serde_json::to_string(&sr.template).unwrap(),
            serde_json::to_string(&FxInstance::slowed_reverb(0.82, 0.1, 85.0)).unwrap()
        );
        // морф-слот в реестре НЕ живёт и жить не может (Р.35): у
        // псевдо-слота нет шаблона, а этот тест сверяет реестр именно с
        // шаблонами — «просто дописать его в реестр» обязано краснеть
        assert!(
            !reg.iter().any(|e| e.name.to_lowercase().contains("source")),
            "подмена источника — не эффект: её место в голове полосы, не в реестре"
        );
        // имена согласованы с fx_display_name
        for e in reg {
            assert_eq!(crate::fx_editor::fx_display_name(&e.template), e.name);
        }
        // Р.47 (п.326): счёт — СВОЙСТВОМ реестра FxInstance::all(), а не
        // константой. Каждый аудио-вид из all() (кроме LayerMix — слот
        // силы, у него нет места в пикере) обязан быть в аудио-списке;
        // каждый видео-вид, у которого есть шаблон, — в видео-списке.
        crate::i18n::set_lang(crate::i18n::Lang::Ru);
        let audio_kinds: std::collections::BTreeSet<&str> = FxInstance::all()
            .iter()
            .filter(|f| f.is_audio())
            .map(|f| f.tag())
            .collect();
        let audio_in_picker: std::collections::BTreeSet<&str> = filter_entries(reg, true, "")
            .iter()
            .map(|e| e.template.tag())
            .collect();
        assert_eq!(audio_in_picker, audio_kinds, "каждый аудио-вид реестра — в пикере");
        // видео-список = все видео-шаблоны + двусторонний Slowed+Reverb
        assert_eq!(filter_entries(reg, false, "").len(), old.len() + 1);
        assert_eq!(filter_entries(reg, false, "reverb").len(), 1, "варп ищется и в видео-пикере");
        assert_eq!(filter_entries(reg, true, "РевЕрб").len(), 1, "кириллический keyword");
        assert_eq!(filter_entries(reg, false, "виш").len(), 0);
        assert_eq!(filter_entries(reg, false, "vhs").len(), 1);
        assert_eq!(filter_entries(reg, false, "blur").len(), 1);
        assert_eq!(filter_entries(reg, true, "gain").len(), 1);
        assert!(filter_entries(reg, true, "vhs").is_empty(), "видео-эффект не в аудио-списке");
        // Р.19: английские ключевые слова обязаны находить эффект В ЛЮБОМ
        // языке — имя эффекта не переводится, и искать его будут по нему
        for lang in crate::i18n::Lang::ALL {
            crate::i18n::set_lang(lang);
            assert_eq!(filter_entries(reg, false, "blur").len(), 1, "blur в {}", lang.code());
            assert_eq!(filter_entries(reg, false, "vhs").len(), 1, "vhs в {}", lang.code());
            assert_eq!(filter_entries(reg, true, "reverb").len(), 1, "reverb в {}", lang.code());
        }
        crate::i18n::set_lang(crate::i18n::Lang::Ru);
    }
}
