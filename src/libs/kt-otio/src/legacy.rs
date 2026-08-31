//! Импорт легаси .ktproj (Р.24 п.12: как ПРОЕКТ он больше не открывается —
//! только пункт «Импортировать…», результат живёт как .otio).
//!
//! Пока легаси-писатель kt-model жив (этап 13 его снесёт), чтение v1 идёт
//! через его же `Project::from_json` (back-compat формы маркеров и дефолты —
//! там); при сносе замороженные копии v1-структур переедут СЮДА. Перекрытия
//! конвертируются `upgrade_legacy` (п.143): аудио-пары → переходы
//! (байт-идентично), 3+/FX — развод по дорожкам, видео — демоция.

use crate::Error;
use crate::convert::Notice;
use kt_model::Project;

pub fn import_legacy(text: &str) -> Result<(Project, Vec<Notice>), Error> {
    // без validate: легаси-перекрытия законны ДО upgrade_legacy
    let mut p = Project::from_json_unvalidated(text)
        .map_err(|e| Error::UnsupportedSchema(format!("ktproj: {e}")))?;
    let rep = kt_model::ops::upgrade_legacy(&mut p);
    p.validate().map_err(|e| Error::UnsupportedSchema(format!("ktproj after upgrade: {e}")))?;
    let mut notices = Vec::new();
    if rep.transitions_created > 0 {
        notices.push(format!("crossfades converted to transitions: {}", rep.transitions_created));
    }
    if rep.audio_moved > 0 {
        notices.push(format!(
            "overlapping audio clips moved to their own tracks: {} (1/N mix is gone — see docs)",
            rep.audio_moved
        ));
    }
    if rep.video_moved > 0 {
        notices.push(format!("overlapping video clips moved below: {}", rep.video_moved));
    }
    Ok((p, notices))
}
