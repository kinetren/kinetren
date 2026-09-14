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
    rate: f64,
    /// Следующий кадр источника, который будет запрошен у pull.
    src_cursor: i64,
    /// Готовый выход (interleaved), ещё не отданный.
    ready: std::collections::VecDeque<f32>,
    underruns: u64,
}

const WARP_CHUNK_OUT: usize = 1024;

impl Warp {
    pub fn new(rate: f64, src_start: i64) -> Warp {
        let rate = rate.clamp(0.25, 32.0);
        let params = SincInterpolationParameters {
            sinc_len: 128,
            f_cutoff: 0.95,
            interpolation: SincInterpolationType::Linear,
            oversampling_factor: 128,
            window: WindowFunction::BlackmanHarris2,
        };
        // ratio = f_out / f_in = 1 / rate (rate 0.5 → выхода вдвое больше)
        let rs = SincFixedOut::<f32>::new(1.0 / rate, 32.0, params, WARP_CHUNK_OUT, CH)
            .expect("rubato init");
        Warp {
            rs,
            rate,
            src_cursor: src_start,
            ready: std::collections::VecDeque::new(),
            underruns: 0,
        }
    }

    pub fn set_rate(&mut self, rate: f64) {
        let rate = rate.clamp(0.25, 32.0);
        if (rate - self.rate).abs() > 1e-9 {
            self.rate = rate;
            // ramp=true: rubato плавно ведёт ratio к цели внутри чанка
            let _ = self.rs.set_resample_ratio(1.0 / rate, true);
        }
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
