//! Варispeed-ресемплер (дословный переезд из lib.rs). Варп — НЕ узел
//! цепочки: у него монополия на потребление источника (chain_source_*).

use crate::{CH, PullKind};
use rubato::{
    Resampler, SincFixedOut, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};

// ---------------------------------------------------------------------------
// Warp (варispeed через rubato)
// ---------------------------------------------------------------------------

/// Читает источник ПОСЛЕДОВАТЕЛЬНО и выдаёт клип-локальный поток с
/// изменённой скоростью (rate < 1 — slowed, питч падает: это желаемая
/// эстетика slowed&reverb). Ни пропусков, ни дублей: source-позиция —
/// внутренний счётчик rubato, не пересчитывается извне.
pub struct Warp {
    rs: SincFixedOut<f32>,
    /// МОДУЛЬ скорости: направление к ресемплеру отношения не имеет
    /// (Р.57). Знак живёт в адресе чтения (`fill_source`), а рубато ест
    /// поток со скоростью |rate| и о направлении не знает.
    rate: f64,
    /// Следующий кадр источника, который будет запрошен у pull.
    src_cursor: i64,
    /// Готовый выход (interleaved), ещё не отданный.
    ready: std::collections::VecDeque<f32>,
    underruns: u64,
    /// Сколько раз rubato ОТКАЗАЛ в смене отношения (окно ±rel вокруг
    /// якоря). Раньше отказ глотался `let _ =`, и кривая 0.25→32 молча
    /// застревала на последнем принятом значении (Р.57, латентный дефект).
    rejected: u64,
}

const WARP_CHUNK_OUT: usize = 1024;

/// Окно отношений вокруг якоря у ЛЕГАСИ-режима (кривая целиком в
/// 0.25…32): ровно как было до Р.57 — на этом стоит байтовое тождество.
const WARP_RATIO_REL_LEGACY: f64 = 32.0;
/// Потолок ЯКОРЯ в расширенном режиме. Якорь задаёт и срез анти-алиасинга
/// (rubato масштабирует `f_cutoff` конструкционным отношением), и размер
/// буфера `(rel+1)·needed_input`. Два — компромисс: выше растёт память,
/// ниже становится слышен алиасинг на быстрых участках.
const WARP_EXT_ANCHOR_MAX_RATE: f64 = 2.0;

impl Warp {
    /// `rate` — МОДУЛЬ скорости. `reach` — (мин, макс) модуля по ВСЕМУ
    /// профилю: ресемплеру нужно знать разброс отношений ДО первого чанка,
    /// иначе смена отношения будет отвергнута (см. `rejected`).
    pub fn new(rate: f64, src_start: i64, reach: (f64, f64)) -> Warp {
        let rate = rate.abs().clamp(kt_model::RATE_AUDIO_FLOOR, 32.0);
        // ЛЕГАСИ-РЕЖИМ (профиль целиком в 0.25…32) — тело дословно
        // прежнее: тот же якорь, то же окно. Байты не двигаются.
        let extended = reach.0 < 0.25;
        let (anchor, rel) = if extended {
            let anchor = rate.clamp(kt_model::RATE_AUDIO_FLOOR, WARP_EXT_ANCHOR_MAX_RATE);
            let rel = WARP_RATIO_REL_LEGACY
                .max(reach.1 / anchor)
                .max(anchor / reach.0.max(kt_model::RATE_AUDIO_FLOOR))
                * 1.01;
            (anchor, rel)
        } else {
            (rate.clamp(0.25, 32.0), WARP_RATIO_REL_LEGACY)
        };
        let params = SincInterpolationParameters {
            sinc_len: 128,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };
        // ratio = f_out / f_in = 1 / rate (rate 0.5 → выхода вдвое больше)
        let mut rs = SincFixedOut::<f32>::new(1.0 / anchor, rel, params, WARP_CHUNK_OUT, CH)
            .expect("rubato init");
        let mut rejected = 0;
        if anchor != rate && rs.set_resample_ratio(1.0 / rate, false).is_err() {
            rejected += 1;
            debug_assert!(false, "rubato rejected the initial ratio");
        }
        Warp {
            rs,
            rate,
            src_cursor: src_start,
            ready: std::collections::VecDeque::new(),
            underruns: 0,
            rejected,
        }
    }

    /// `rate` — МОДУЛЬ скорости.
    pub fn set_rate(&mut self, rate: f64) {
        let rate = rate.abs().clamp(kt_model::RATE_AUDIO_FLOOR, 32.0);
        if (rate - self.rate).abs() > 1e-9 {
            self.rate = rate;
            // ramp=true: rubato плавно ведёт ratio к цели внутри чанка
            if self.rs.set_resample_ratio(1.0 / rate, true).is_err() {
                // ОТКАЗ НАЗЫВАЕТСЯ (Р.57): молчаливое залипание на прежнем
                // отношении слышно как «замедление не применилось»
                self.rejected += 1;
                debug_assert!(false, "rubato rejected a ratio change: {rate}");
            }
        }
    }

    /// Сколько смен отношения rubato отверг (диагностика, тесты).
    pub fn rejected(&self) -> u64 {
        self.rejected
    }

    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// Выдать `out.len()/CH` кадров, дотягивая источник через `pull`.
    /// `pull(frame, buf)` заполняет interleaved-кадры источника, начиная с
    /// `frame`; false = не удалось (underrun, будет тишина).
    pub fn produce(
        &mut self,
        kind: PullKind,
        out: &mut [f32],
        pull: &mut dyn FnMut(PullKind, i64, &mut [f32]) -> bool,
    ) {
        let need_frames = out.len() / CH;
        while self.ready.len() < need_frames * CH {
            let in_frames = self.rs.input_frames_next();
            let mut input = vec![0f32; in_frames * CH];
            if !pull(kind, self.src_cursor, &mut input) {
                self.underruns += 1;
                input.fill(0.0);
            }
            self.src_cursor += in_frames as i64;
            // interleaved → planar
            let planar: Vec<Vec<f32>> =
                (0..CH).map(|ch| (0..in_frames).map(|f| input[f * CH + ch]).collect()).collect();
            let out_planar = self.rs.process(&planar, None).expect("rubato process");
            let produced = out_planar[0].len();
            for f in 0..produced {
                for chd in out_planar.iter().take(CH) {
                    self.ready.push_back(chd[f]);
                }
            }
        }
        for s in out.iter_mut() {
            *s = self.ready.pop_front().unwrap();
        }
    }

    /// Переставить источник БЕЗ пересоздания ресемплера — шов заворота
    /// (см. `ClipProcessor::seam`): внутреннее состояние sinc'а живёт
    /// дальше, выбрасывается только произведённый, но не отданный выход
    /// (он посчитан от прежней позиции источника).
    pub fn seek_source(&mut self, src: i64) {
        self.src_cursor = src;
        self.ready.clear();
    }

    pub fn underruns(&self) -> u64 {
        self.underruns
    }

    /// Текущий курсор источника (диагностика/тесты).
    pub fn src_cursor(&self) -> i64 {
        self.src_cursor
    }
}
