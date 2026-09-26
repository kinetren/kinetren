//! Импорт проектов СТАРЫХ ВЕРСИЙ конверта (Р.67, решение владельца).
//!
//! «Делай пункт в меню File для импорта старых версий, и именно в импорте
//! чини чтобы ридеру было нормально это читать. И другим агентам закрепи
//! чтобы сами автоматом правки в импорт старых версий делали, постепенно
//! эти правки будут удаляться с уходом старых версий».
//!
//! Поэтому ридер (`timeline_to_project`) старым формам НЕ учится. Файл,
//! который он отвергает, идёт сюда: JSON-предобработка по таблице `FIXUPS`,
//! затем обычное чтение. Каждая починка несёт ВЕРСИЮ, для которой заведена
//! (применяется к файлам этой версии и старше), и notice — человек видит,
//! что именно было переписано. Снятие поддержки версии удаляет её починки
//! вместе с тестами — таблица и есть список «версий с починками».

use crate::Error;
use crate::convert::Notice;
use serde_json::Value;

/// Одна починка: версия конверта, для которой заведена, имя (в notice) и
/// тело — возвращает число исправленных мест.
pub struct Fixup {
    pub version: u32,
    pub name: &'static str,
    pub apply: fn(&mut Value) -> usize,
}

/// Таблица починок. Порядок — по версии, старшие раньше: файл v5 получает
/// починки v5 и всё, что заведено для более новых форм, которые он тоже
/// мог не знать.
pub const FIXUPS: &[Fixup] = &[
    Fixup {
        version: crate::meta::KT_META_V5,
        name: "morpher targets: null → below (v5)",
        apply: v5_null_morph_targets,
    },
    Fixup {
        version: crate::meta::KT_META_V6,
        name: "morpher patch: pan → geometry node (v6)",
        apply: v6_pan_in_morph_patch,
    },
];

/// Списки морферов документа: `morphers` и слои `layers` до их слияния —
/// по очереди, одним замыканием на каждый.
fn for_each_morph_list(doc: &mut Value, mut f: impl FnMut(&mut Vec<Value>)) {
    let Some(kt) = doc.get_mut("metadata").and_then(|m| m.get_mut("kt")) else { return };
    let Some(obj) = kt.as_object_mut() else { return };
    for key in ["morphers", "layers"] {
        if let Some(arr) = obj.get_mut(key).and_then(Value::as_array_mut) {
            f(arr);
        }
    }
}

/// v5 (Р.38): цели морфера были `Option`, и «всё, что ниже строки» писалось
/// как `null`. С Р.50 цели — перечисление без варианта «ничего», и `null`
/// роняет untagged-ридер на `MorphTargets`. Умолчание перечисления и есть
/// «ниже», поэтому ключ просто снимается.
fn v5_null_morph_targets(doc: &mut Value) -> usize {
    let mut n = 0;
    for_each_morph_list(doc, |arr| {
        for m in arr.iter_mut() {
            if let Some(obj) = m.as_object_mut()
                && obj.get("targets").is_some_and(Value::is_null)
            {
                obj.remove("targets");
                n += 1;
            }
        }
    });
    n
}

/// Упразднённый эффект `kt:pan` (Р.62) внутри ПАТЧА морфера. У клипа его
/// переводит ридер (`migrate_pan_effect`, эффект отличим по имени
/// `kt:pan`), а в патче эффект лежит голым `FxInstance` с `kind: pan` —
/// варианта такого больше нет, и untagged-ридер патча падал целиком.
/// Перевод тот же, что у клипа: узел «Геометрия» на том же слоте, кривые
/// `pan_*` → `pos_*`.
fn v6_pan_in_morph_patch(doc: &mut Value) -> usize {
    let mut n = 0;
    for_each_morph_list(doc, |arr| {
        for m in arr.iter_mut() {
            let Some(patch) = m.get_mut("patch").and_then(Value::as_object_mut) else { continue };
            let Some(fx) = patch.get("video_fx").and_then(Value::as_array).cloned() else { continue };
            let mut auto: Vec<Value> = patch
                .get("video_auto")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            auto.resize(fx.len(), Value::Array(Vec::new()));
            let mut new_fx = fx.clone();
            let mut touched = false;
            for (i, f) in fx.iter().enumerate() {
                if f.get("kind").and_then(Value::as_str) != Some("pan") {
                    continue;
                }
                let mut kt = f.clone();
                kt["auto"] = auto[i].clone();
                let mut meta = serde_json::Map::new();
                meta.insert(crate::meta::KT_NS.to_string(), kt);
                if let Some((inst, curves)) = crate::convert::migrate_pan_effect(&meta)
                    && let Ok(v) = serde_json::to_value(&inst)
                    && let Ok(c) = serde_json::to_value(&curves)
                {
                    new_fx[i] = v;
                    auto[i] = c;
                    touched = true;
                    n += 1;
                }
            }
            if touched {
                patch.insert("video_fx".into(), Value::Array(new_fx));
                patch.insert("video_auto".into(), Value::Array(auto));
            }
        }
    });
    n
}

/// Версия конверта документа (None — не наш файл).
fn doc_version(doc: &Value) -> Option<u32> {
    doc.get("metadata")?.get("kt")?.get("v")?.as_u64().map(|v| v as u32)
}

/// Импорт файла старой версии: починки → обычное чтение. Возвращает
/// проект и notice'ы (сначала — что починено, затем обычные notice'ы
/// чтения). Файл текущей версии тоже проходит (починок для него нет) —
/// дверь не отказывает человеку, который выбрал не тот пункт.
pub fn import_older(
    text: &str,
    base_dir: Option<&std::path::Path>,
) -> Result<crate::convert::ProjectBundle, Error> {
    let mut doc: Value = serde_json::from_str(text)?;
    let file_v = doc_version(&doc).ok_or(Error::NotOtio)?;
    let mut fixed: Vec<Notice> = Vec::new();
    for f in FIXUPS {
        if file_v <= f.version {
            let n = (f.apply)(&mut doc);
            if n > 0 {
                fixed.push(format!("import fixup: {} ×{n}", f.name));
            }
        }
    }
    let patched = serde_json::to_string(&doc)?;
    let tl = crate::io::parse_document(&patched)?;
    let mut b = crate::convert::timeline_to_project(&tl, base_dir)?;
    fixed.append(&mut b.notices);
    b.notices = fixed;
    Ok(b)
}
