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
        opts: kt_fx::ChainOpts,
    ) {
        if fx.is_empty() {
            self.map.remove(&id);
            return;
        }
        let fp = chain_fingerprint(fx);
        match self.map.get_mut(&id) {
            Some(p) if p.fingerprint() == fp => {
                p.set_clip_len(clip_len_frames);
                // ГЕОМЕТРИЯ доносится тоже (Р.43, п.293): `chain_fingerprint`
                // хэширует только ВИДЫ слотов, поэтому трим/разрез/punch-out/
                // переанкоринг морф-куска меняли `source_in` при том же id и
                // той же структуре цепочки — и процессор читал от старого
                // якоря, то есть «звук от другого события».
                p.set_src_in(src_in_frames);
                // кривые доносятся ДО параметров: apply_params смотрит,
                // у каких слотов есть кривая (громкость, скорость)
                p.set_auto(auto);
                p.apply_params(fx, false);
                // Упреждение — часть УСТРОЙСТВА цепочки, а не значение
                // параметра (Р.48, п.337): длина колец меняет выравнивание
                // потока, и донести её сглаживанием нельзя. Расхождение
                // (правка окна лимитера, нейтраль питча ↔ актив) —
                // пересборка процессора: следующий read сделает hard_seek
                // с прогревом, как настоящий seek.
                if p.lookahead_frames() != kt_fx::chain_lookahead(fx, auto) || p.opts() != opts {
                    self.map.insert(
                        id,
                        ClipProcessor::new_with_opts(
                            fx,
                            auto,
                            src_in_frames,
                            clip_len_frames,
                            opts,
                        ),
                    );
                }
            }
            _ => {
                self.map.insert(
                    id,
                    ClipProcessor::new_with_opts(fx, auto, src_in_frames, clip_len_frames, opts),
                );
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
