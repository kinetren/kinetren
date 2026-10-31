//! An effect slot backed by a plugin, and a slot whose plugin this build lacks (plugin-api-stage1,
//! D3). The id IS the tag the kind had as an enum variant, and a plugin slot writes the same text a
//! variant wrote: `kind`, then the passport rows in passport order, each in its field's type, with
//! the rows declared `omit_default` left out at their default — t973 compares it with a frozen copy
//! of the derive. A slot nobody registered keeps its raw content and writes it back unchanged.

use kt_plugin_api::{Family, Manifest, ParamSpec, Registry};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The installed registry; before an install, an empty one. Reading then keeps every plugin kind
/// as `Missing` (written back unchanged) instead of panicking: a deserializer runs in background
/// workers (presets, the sidecar, recovery), and a panic there stopped their work silently — the
/// first workspace run after 2.3 showed six such timeouts. kt-app installs the built-in set in
/// `run` and in every `KtApp` constructor, kt-cli in `main`, tests through the same function.
pub fn registry() -> &'static Registry {
    static EMPTY: std::sync::LazyLock<Registry> = std::sync::LazyLock::new(Registry::new);
    kt_plugin_api::installed().map(|r| &**r).unwrap_or(&EMPTY)
}

/// A plugin-backed effect: its id and its numeric values in passport order.
#[derive(Clone, PartialEq)]
pub struct PluginSlot {
    /// The family the id is looked up in (an id is unique within its family only, D13).
    pub family: Family,
    pub id: String,
    pub values: Vec<f64>,
    /// The legacy fields the plugin declares (`Decl.legacy_fields`) as the file had them: a field a
    /// door still migrates (Slowed+Reverb's `automation`, `migrate_legacy_slowed`) reaches the
    /// migration instead of being dropped by the read; written back after the rows only while
    /// nothing consumed it, as the variant wrote it. Nothing else is kept — the envelope keys a
    /// door puts next to the rows (kt-otio's `auto`) are the door's.
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl std::fmt::Debug for PluginSlot {
    /// The derive's form while `extra` is empty: the Debug text is part of `content_hash`, so a slot
    /// without foreign fields must print as it did before the field existed.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut d = f.debug_struct("PluginSlot");
        d.field("id", &self.id).field("values", &self.values);
        if !self.extra.is_empty() {
            d.field("extra", &self.extra);
        }
        d.finish()
    }
}

impl PluginSlot {
    /// A slot of `id` of `family` at its passport's defaults; `None` — no such plugin is registered.
    pub fn new(family: Family, id: &str) -> Option<PluginSlot> {
        let m = registry().get(family, id)?.manifest();
        let values = m.passport.iter().enumerate().map(|(i, s)| m.row(i).store.cast(s.default)).collect();
        Some(PluginSlot { family, id: id.to_string(), values, extra: Default::default() })
    }

    /// The installed plugin of this slot.
    pub fn plugin(&self) -> Option<&'static std::sync::Arc<dyn kt_plugin_api::Plugin>> {
        registry().get(self.family, &self.id)
    }

    pub fn manifest(&self) -> &'static Manifest {
        self.plugin()
            .unwrap_or_else(|| panic!("plugin `{}` is not registered", self.id))
            .manifest()
    }

    pub fn specs(&self) -> &'static [ParamSpec] {
        &self.manifest().passport
    }

    /// The id as `&'static str` (it lives in the installed registry).
    pub fn tag(&self) -> &'static str {
        &self.manifest().id
    }

    pub fn get_idx(&self, idx: usize) -> Option<f64> {
        self.values.get(idx).copied()
    }

    /// Write by row index: clamped to the passport and cast to the row's field type (`fx_cast!`).
    pub fn set_idx(&mut self, idx: usize, v: f64) -> bool {
        let m = self.manifest();
        let Some(spec) = m.passport.get(idx) else { return false };
        let Some(slot) = self.values.get_mut(idx) else { return false };
        *slot = m.row(idx).store.cast(v.clamp(spec.min, spec.max));
        true
    }

    pub fn get(&self, name: &str) -> Option<f64> {
        let idx = self.specs().iter().position(|s| s.name == name)?;
        self.get_idx(idx)
    }

    /// The non-empty host-reference rows of `role` (D11.4): (row index, the referenced id).
    pub fn refs(&self, role: kt_plugin_api::RowRole) -> impl Iterator<Item = (usize, u64)> + '_ {
        let m = self.manifest();
        (0..m.passport.len()).filter_map(move |i| match m.row(i).store {
            kt_plugin_api::Store::Ref(r) if r == role => {
                let v = self.values.get(i).copied().unwrap_or(0.0);
                (v > 0.0).then_some((i, v as u64))
            }
            _ => None,
        })
    }

    /// Set the FIRST row of `role` (`None` — clear it); false when the plugin has no such row.
    pub fn set_ref(&mut self, role: kt_plugin_api::RowRole, id: Option<u64>) -> bool {
        let m = self.manifest();
        let Some(i) = (0..m.passport.len())
            .find(|&i| matches!(m.row(i).store, kt_plugin_api::Store::Ref(r) if r == role))
        else {
            return false;
        };
        self.set_idx(i, id.map_or(0.0, |n| n as f64))
    }

    pub fn set(&mut self, name: &str, v: f64) -> bool {
        match self.specs().iter().position(|s| s.name == name) {
            Some(idx) => self.set_idx(idx, v),
            None => false,
        }
    }
}

impl Serialize for PluginSlot {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        let m = self.manifest();
        let mut map = ser.serialize_map(None)?;
        map.serialize_entry("kind", &*m.id)?;
        for (i, spec) in m.passport.iter().enumerate() {
            let row = m.row(i);
            let v = self.values.get(i).copied().unwrap_or(spec.default);
            if row.omit_default && v == row.store.cast(spec.default) {
                continue;
            }
            let key: &str = &spec.name;
            match row.store {
                kt_plugin_api::Store::F64 => map.serialize_entry(key, &v)?,
                kt_plugin_api::Store::F32 => map.serialize_entry(key, &(v as f32))?,
                kt_plugin_api::Store::U32 => map.serialize_entry(key, &(v as u32))?,
                kt_plugin_api::Store::U8 => map.serialize_entry(key, &(v as u8))?,
                kt_plugin_api::Store::I32 => map.serialize_entry(key, &(v as i32))?,
                kt_plugin_api::Store::Bool => map.serialize_entry(key, &(v != 0.0))?,
                kt_plugin_api::Store::Choice(keys) => map.serialize_entry(key, keys[v as usize])?,
                kt_plugin_api::Store::Ref(_) => map.serialize_entry(key, &(v as u64))?,
            }
        }
        for (k, v) in &self.extra {
            map.serialize_entry(k, v)?;
        }
        map.end()
    }
}

impl PluginSlot {
    /// Read a slot of `family` from its raw content; an id this family does not register is not an
    /// error of the file: the slot is kept as `Missing` (the next untagged variant).
    pub fn from_value(raw: &serde_json::Value, family: Family) -> Result<PluginSlot, String> {
        let kind = raw.get("kind").and_then(|k| k.as_str()).ok_or("no kind")?;
        let Some(p) = registry().get(family, kind) else {
            return Err(format!("plugin `{kind}` is not registered"));
        };
        let m = p.manifest();
        let values = m
            .passport
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let store = m.row(i).store;
                let field = raw.get(&*s.name);
                let v = match store {
                    kt_plugin_api::Store::Bool => field.and_then(|v| v.as_bool()).map(|b| if b { 1.0 } else { 0.0 }),
                    kt_plugin_api::Store::Choice(keys) => {
                        field.and_then(|v| v.as_str()).and_then(|k| keys.iter().position(|x| *x == k)).map(|i| i as f64)
                    }
                    kt_plugin_api::Store::Ref(_) => field.and_then(|v| v.as_u64()).map(|n| n as f64),
                    _ => field.and_then(|v| v.as_f64()),
                };
                store.cast(v.unwrap_or(s.default))
            })
            .collect();
        let extra = m
            .decl
            .legacy_fields
            .iter()
            .filter_map(|k| raw.get(*k).map(|v| (k.to_string(), v.clone())))
            .collect();
        Ok(PluginSlot { family, id: kind.to_string(), values, extra })
    }
}

fn de_family<'de, D: Deserializer<'de>>(de: D, family: Family) -> Result<PluginSlot, D::Error> {
    let raw = serde_json::Value::deserialize(de)?;
    PluginSlot::from_value(&raw, family).map_err(serde::de::Error::custom)
}

/// The reader of an effect slot (`FxInstance::Plugin`).
pub fn de_fx<'de, D: Deserializer<'de>>(de: D) -> Result<PluginSlot, D::Error> {
    de_family(de, Family::Fx)
}

/// The reader of a transition kind (`TransitionKind::Plugin`).
pub fn de_transition<'de, D: Deserializer<'de>>(de: D) -> Result<PluginSlot, D::Error> {
    de_family(de, Family::Transition)
}

/// The reader of a generator kind (`GeneratorKind::Plugin`).
pub fn de_generator<'de, D: Deserializer<'de>>(de: D) -> Result<PluginSlot, D::Error> {
    de_family(de, Family::Generator)
}

/// An effect whose plugin this build lacks: kept, passed through, written back unchanged.
#[derive(Debug, Clone, PartialEq)]
pub struct MissingSlot {
    pub raw: serde_json::Value,
}

impl MissingSlot {
    /// The id the file names.
    pub fn id(&self) -> &str {
        self.raw.get("kind").and_then(|k| k.as_str()).unwrap_or("")
    }
}

impl Serialize for MissingSlot {
    fn serialize<S: Serializer>(&self, ser: S) -> Result<S::Ok, S::Error> {
        self.raw.serialize(ser)
    }
}

impl<'de> Deserialize<'de> for MissingSlot {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<MissingSlot, D::Error> {
        let raw = serde_json::Value::deserialize(de)?;
        if raw.get("kind").and_then(|k| k.as_str()).is_none() {
            return Err(serde::de::Error::custom("an effect without a kind"));
        }
        Ok(MissingSlot { raw })
    }
}
