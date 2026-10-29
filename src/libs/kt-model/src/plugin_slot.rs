//! An effect slot backed by a plugin, and a slot whose plugin this build lacks (plugin-api-stage1,
//! D3). The id IS the tag the kind had as an enum variant, and a plugin slot writes the same text a
//! variant wrote: `kind`, then the passport rows in passport order, each in its field's type, with
//! the rows declared `omit_default` left out at their default — t973 compares it with a frozen copy
//! of the derive. A slot nobody registered keeps its raw content and writes it back unchanged.

use kt_plugin_api::{Manifest, ParamSpec, Registry};
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
#[derive(Debug, Clone, PartialEq)]
pub struct PluginSlot {
    pub id: String,
    pub values: Vec<f64>,
}

impl PluginSlot {
    /// A slot of `id` at its passport's defaults; `None` — no such plugin is registered.
    pub fn new(id: &str) -> Option<PluginSlot> {
        let m = registry().get(id)?.manifest();
        let values = m.passport.iter().enumerate().map(|(i, s)| m.row(i).store.cast(s.default)).collect();
        Some(PluginSlot { id: id.to_string(), values })
    }

    pub fn manifest(&self) -> &'static Manifest {
        registry()
            .get(&self.id)
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
            }
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for PluginSlot {
    fn deserialize<D: Deserializer<'de>>(de: D) -> Result<PluginSlot, D::Error> {
        let raw = serde_json::Value::deserialize(de)?;
        let kind = raw.get("kind").and_then(|k| k.as_str()).ok_or_else(|| serde::de::Error::custom("no kind"))?;
        // an id this build does not register is not an error of the file: the slot is kept as
        // `Missing` (the next untagged variant)
        let Some(p) = registry().get(kind) else {
            return Err(serde::de::Error::custom(format!("plugin `{kind}` is not registered")));
        };
        let m = p.manifest();
        let values = m
            .passport
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let v = raw.get(&*s.name).and_then(|v| v.as_f64()).unwrap_or(s.default);
                m.row(i).store.cast(v)
            })
            .collect();
        Ok(PluginSlot { id: kind.to_string(), values })
    }
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
