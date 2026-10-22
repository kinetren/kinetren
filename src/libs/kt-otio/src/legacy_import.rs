//! Import of projects of OLD versions of the envelope (R.67, owner’s decision).
//!
//! “Make an item in the File menu for importing old versions, and fix in the import itself
//! so that the reader can read it normally. And to the other agents, fix it so that
//! they themselves automatically make edits into the import of old versions; gradually
//! these edits will be removed as old versions are dropped.”
//!
//! Therefore the reader (`timeline_to_project`) does NOT learn the old forms. A file
//! that it rejects goes here: JSON preprocessing by the `FIXUPS` table,
//! then a normal read. Each fix carries the VERSION it was made for
//! (it applies to files of that version and older), and a notice — the person sees
//! exactly what was rewritten. Dropping support for a version deletes its fixes
//! together with the tests — the table is itself the list of “versions with fixes”.

use crate::Error;
use crate::convert::Notice;
use serde_json::Value;

/// One fix: the envelope version it was made for, its name (in the notice) and
/// a body — returns the number of corrected places.
pub struct Fixup {
    pub version: u32,
    pub name: &'static str,
    pub apply: fn(&mut Value) -> usize,
}

/// The table of fixes. Order — by version, newest first: a v5 file gets the
/// fixes of v5 and everything filed for newer forms, which it too
/// could not have known.
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

/// The document’s morpher lists: `morphers` and the `layers` before their merge —
/// in turn, with one closure each.
fn for_each_morph_list(doc: &mut Value, mut f: impl FnMut(&mut Vec<Value>)) {
    let Some(kt) = doc.get_mut("metadata").and_then(|m| m.get_mut("kt")) else { return };
    let Some(obj) = kt.as_object_mut() else { return };
    for key in ["morphers", "layers"] {
        if let Some(arr) = obj.get_mut(key).and_then(Value::as_array_mut) {
            f(arr);
        }
    }
}

/// v5 (R.38): morpher targets were `Option`, and “everything below the row” was written
/// as `null`. Since R.50 the targets are an enum without a “nothing” variant, and `null`
/// breaks the untagged reader on `MorphTargets`. The enum’s default is exactly
/// “below”, so the key is simply removed.
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

/// The abolished effect `kt:pan` (R.62) inside a morpher PATCH. For a clip the reader
/// converts it (`migrate_pan_effect`, the effect is recognized by its name
/// `kt:pan`), but in a patch the effect lies bare as an `FxInstance` with `kind: pan` —
/// there is no such variant any more, and the untagged reader of the patch failed completely.
/// The conversion is the same as for a clip: a “Geometry” node on the same slot, the curves
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

/// The envelope version of the document (None — not our file).
fn doc_version(doc: &Value) -> Option<u32> {
    doc.get("metadata")?.get("kt")?.get("v")?.as_u64().map(|v| v as u32)
}

/// Import of a file of an old version: fixes → normal read. Returns the
/// project and the notices (first — what was fixed, then the usual read
/// notices). A file of the current version passes too (there are no fixes for it) —
/// the door does not refuse a person who chose the wrong item.
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
