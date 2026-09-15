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
    /// Состояние ВЛОЖЕННЫХ лент (Р.50, Б4): своё на ленту.
    ///
    /// Не «плоский ключ по ClipId на весь документ», хотя адреса уникальны:
    /// лента, поставленная в пяти местах, — это ПЯТЬ разных потоков через
    /// её цепочки, и делить между ними один реверб было бы слышно. Ключ —
    /// адрес ленты, а не экземпляра, потому что один экземпляр читает
    /// суб-микс последовательно, а разные — своими окнами; на них стоит
    /// `last` (см. ниже).
    nests: HashMap<kt_model::MediaId, NestState>,
}

/// Состояние одной вложенной ленты в аудио-пути.
#[derive(Default)]
pub struct NestState {
    pub store: FxStateStore,
    /// Последний отданный бёрст суб-микса. Повторный запрос того же —
    /// обычное дело (`render_plain_clip` держит курсор на одном бёрсте и
    /// берёт из него куски), а пересчитывать потоковую цепочку второй раз
    /// нельзя: она вернула бы ДРУГОЙ выход.
    pub last: Option<(i64, std::sync::Arc<kt_media::AudioBurst>)>,
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
    /// Состояние вложенной ленты (заводится по первому обращению).
    pub fn nest_mut(&mut self, seq: kt_model::MediaId) -> &mut NestState {
        self.nests.entry(seq).or_default()
    }

    /// Забрать состояния лент НАРУЖУ на время рендера.
    ///
    /// Нужно ровно для одного: суб-рендер зовётся ИЗ фетчера, а фетчер
    /// живёт рядом со стором — взять и то и другое мутабельно одновременно
    /// нечем. Вынутая карта развязывает заимствования, и `put_nests`
    /// возвращает её на место в том же вызове.
    pub fn take_nests(&mut self) -> HashMap<kt_model::MediaId, NestState> {
        std::mem::take(&mut self.nests)
    }

    pub fn put_nests(&mut self, v: HashMap<kt_model::MediaId, NestState>) {
        self.nests = v;
    }

    pub fn retain_nests(&mut self, alive: &dyn Fn(kt_model::MediaId) -> bool) {
        self.nests.retain(|id, _| alive(*id));
    }

    pub fn reset_all(&mut self) {
        // seek уходит ВГЛУБЬ: цепочки внутри ленты — такие же потоковые
        // автоматы, и хвост реверба из другого места звучал бы «из прошлого»
        for n in self.nests.values_mut() {
            n.store.reset_all();
            n.last = None;
        }
        for p in self.map.values_mut() {
            p.reset();
        }
    }
}
