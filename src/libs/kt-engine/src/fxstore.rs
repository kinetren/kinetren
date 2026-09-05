//! FxStateStore: состояние FX-цепочек по clip id, ВНЕ render graph
//! (инварианты №2–3). Владелец — аудио-render-поток; сюда не заходит ни
//! prefetch, ни UI. Пересборка AudioRenderer (любая правка модели) НЕ
//! уничтожает состояние: та же структура цепочки → тот же процессор,
//! параметры доезжают сглаженно (дефекты b, f старого движка).

use kt_fx::{ClipProcessor, chain_fingerprint};
use kt_model::{ClipId, FxInstance};
use std::collections::HashMap;

#[derive(Default)]
pub struct FxStateStore {
    map: HashMap<ClipId, ClipProcessor>,
}

impl FxStateStore {
    pub fn new() -> FxStateStore {
        FxStateStore::default()
    }

    /// Синхронизировать процессор клипа с моделью:
    /// - структура цепочки та же → переиспользуем state, доносим параметры;
    /// - структура сменилась → пересоздаём ТОЛЬКО этот клип;
    /// - цепочка пуста → процессор не нужен.
    pub fn sync_clip(
        &mut self,
        id: ClipId,
        fx: &[FxInstance],
        auto: &kt_fx::AutoProfiles,
        src_in_frames: i64,
        clip_len_frames: i64,
    ) {
        if fx.is_empty() {
            self.map.remove(&id);
            return;
        }
        let fp = chain_fingerprint(fx);
        match self.map.get_mut(&id) {
            Some(p) if p.fingerprint() == fp => {
                p.set_clip_len(clip_len_frames);
                // кривые доносятся ДО параметров: apply_params смотрит,
                // у каких слотов есть кривая (громкость, скорость)
                p.set_auto(auto);
                p.apply_params(fx, false);
            }
            _ => {
                self.map
                    .insert(id, ClipProcessor::new(fx, auto, src_in_frames, clip_len_frames));
            }
        }
    }

    /// Убрать состояние клипов, которых больше нет в модели.
    pub fn retain_clips(&mut self, alive: &dyn Fn(ClipId) -> bool) {
        self.map.retain(|id, _| alive(*id));
    }

    pub fn get_mut(&mut self, id: ClipId) -> Option<&mut ClipProcessor> {
        self.map.get_mut(&id)
    }

    /// Полный сброс (seek транспорта): каждый процессор сделает pre-roll
    /// при следующем чтении.
    pub fn reset_all(&mut self) {
        for p in self.map.values_mut() {
            p.reset();
        }
    }
}
