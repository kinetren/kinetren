//! Import of a legacy .ktproj (R.24 item 12: as a PROJECT it no longer opens —
//! only the “Import…” item; the result lives as .otio).
//!
//! While the legacy writer of kt-model is alive (stage 13 will remove it), reading v1 goes
//! through its own `Project::from_json` (back-compat of the marker forms and defaults —
//! there); when it is removed, the frozen copies of the v1 structures will move HERE. Overlaps are
//! converted by `upgrade_legacy` (item 143): audio pairs → transitions
//! (byte-identical), 3+/FX — spread across tracks, video — demoted.

use crate::Error;
use crate::convert::Notice;
use kt_model::Project;

pub fn import_legacy(text: &str) -> Result<(Project, Vec<Notice>), Error> {
    // without validate: legacy overlaps are legal BEFORE upgrade_legacy
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
