//! Компилированная автоматизация цепочки (Р.59, п.467).
//!
//! До этого раунда кривые видео-пути разрешались ПО ИМЕНИ на каждый кадр:
//! `layer_src` клонировал всю цепочку и для каждой кривой звал
//! `set_param(&str, …)`, а тот линейно искал спеку и сравнивал строки.
//! У звука такого никогда не было — там кривые компилируются ОДИН РАЗ при
//! сборке процессора (`AutoProfiles` в `warp.rs`), и на блок остаётся
//! только арифметика. Этот модуль — то же самое для видео.
//!
//! Что план даёт помимо скорости: он ЯВНО отвечает на вопрос «есть ли у
//! этого клипа живые кривые». Пустой план — это `None`, и вызывающий
//! остаётся на прежнем пути заимствования (ноль аллокаций, байты прежние).
//!
//! Чего план НЕ делает: он не трогает кривые ГЕНЕРАТОРА. Их разрешает сам
//! источник по своему тику — кадр генератора обязан оставаться чистой
//! функцией (состояние источника, тик, качество), иначе ключ кэша
//! перестаёт определять кадр (инвариант 4).

use crate::{FxInstance, HasParams, ParamCurve, curve_value};
use kt_time::Tick;

/// Одна скомпилированная кривая: куда писать (слот + ИНДЕКС ряда паспорта)
/// и чем (ключи + пределы клампа владельца).
#[derive(Debug, Clone, PartialEq)]
struct PlannedCurve {
    slot: u32,
    idx: u16,
    keys: Box<[crate::AutoKey]>,
}

/// План автоматизации цепочки. Строится при сборке рендерера, живёт
/// столько же, применяется на кадр.
#[derive(Debug, Clone, PartialEq)]
pub struct AutoPlan {
    curves: Vec<PlannedCurve>,
}

impl AutoPlan {
    /// Скомпилировать кривые цепочки. `None` — живых кривых нет вовсе:
    /// ни одной, все пустые или все сиротские. Это и есть гейт «ноль
    /// цены без использования».
    ///
    /// Сироты (имя не подходит варианту) отбрасываются ЗДЕСЬ — ровно как
    /// их молча пропускает именной путь: `set_param` вернул бы `false`.
    pub fn compile(fx: &[FxInstance], auto: &[Vec<ParamCurve>]) -> Option<AutoPlan> {
        let mut curves = Vec::new();
        for (slot, list) in auto.iter().enumerate() {
            let Some(f) = fx.get(slot) else { break };
            for c in list {
                if c.keys.is_empty() || !c.source.is_keys() {
                    continue;
                }
                let Some(idx) = f.param_idx(&c.param) else { continue };
                curves.push(PlannedCurve {
                    slot: slot as u32,
                    idx: idx as u16,
                    keys: c.keys.clone().into_boxed_slice(),
                });
            }
        }
        (!curves.is_empty()).then_some(AutoPlan { curves })
    }

    /// Разрешить кривые в материальной позиции `at`.
    ///
    /// Числа обязаны совпадать с именным путём (`apply_fx_auto`) до
    /// последнего бита: значение считает ТА ЖЕ `curve_value`, а предел
    /// накладывает САМ ВЛАДЕЛЕЦ — кламп живёт в одном месте, в макросе
    /// паспорта, и своего клампа у плана нет намеренно. Сторож —
    /// замороженный оракул в тесте.
    pub fn apply(&self, fx: &mut [FxInstance], at: Tick) {
        for c in &self.curves {
            let Some(f) = fx.get_mut(c.slot as usize) else { continue };
            f.set_param_idx(c.idx as usize, curve_value(&c.keys, at));
        }
    }

    /// Тики всех ключей плана — по ним зондируют кривую там, где важны
    /// именно изломы (например максимум зума по клипу).
    pub fn key_ticks(&self) -> impl Iterator<Item = Tick> + '_ {
        self.curves.iter().flat_map(|c| c.keys.iter().map(|k| k.t))
    }

    /// Сколько кривых скомпилировалось (для тестов и диагностики).
    pub fn len(&self) -> usize {
        self.curves.len()
    }

    pub fn is_empty(&self) -> bool {
        self.curves.is_empty()
    }
}
