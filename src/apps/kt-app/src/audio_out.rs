//! cpal-вывод. Инвариант №6: колбэк — мастер-часы. В колбэке ни аллокаций,
//! ни локов: только чтение SPSC-кольца, атомарный счётчик потреблённых
//! кадров (по нему аудио-render-поток продвигает playhead) и flush-флаг
//! (после seek выбрасываем хвост старой позиции).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use kt_engine::{AudioShared, Transport};
use kt_time::{AUDIO_CHANNELS, AUDIO_SAMPLE_RATE};
use std::sync::Arc;
use std::sync::atomic::Ordering;

pub struct AudioOutput {
    _stream: cpal::Stream,
    #[allow(dead_code)]
    pub device_rate: u32,
}

pub fn start(
    mut ring: rtrb::Consumer<f32>,
    shared: Arc<AudioShared>,
    transport: Arc<Transport>,
) -> Result<AudioOutput, String> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("no audio output device")?;

    // предпочитаем 48k stereo f32 — тогда колбэк вообще без преобразований
    let mut chosen: Option<cpal::SupportedStreamConfig> = None;
    if let Ok(configs) = device.supported_output_configs() {
        for c in configs {
            if c.channels() == AUDIO_CHANNELS as u16
                && c.sample_format() == cpal::SampleFormat::F32
                && c.min_sample_rate().0 <= AUDIO_SAMPLE_RATE
                && c.max_sample_rate().0 >= AUDIO_SAMPLE_RATE
            {
                chosen = Some(c.with_sample_rate(cpal::SampleRate(AUDIO_SAMPLE_RATE)));
                break;
            }
        }
    }
    let config = match chosen {
        Some(c) => c,
        None => device.default_output_config().map_err(|e| e.to_string())?,
    };
    let device_rate = config.sample_rate().0;
    let device_channels = config.channels() as usize;
    if config.sample_format() != cpal::SampleFormat::F32 {
        return Err(format!(
            "sample format {:?} is not supported (need f32)",
            config.sample_format()
        ));
    }
    // Наивная подгонка под чужой rate (позиционный повтор/пропуск кадров).
    // На 48k-устройствах (норма для PipeWire) путь тождественный.
    let step = AUDIO_SAMPLE_RATE as f64 / device_rate as f64;

    // состояние varispeed-интерполятора между колбэками
    let mut vs_prev = (0.0f32, 0.0f32);
    let mut vs_cur = (0.0f32, 0.0f32);
    let mut vs_primed = false;
    let mut vs_phase = 0.0f64;
    let stream = device
        .build_output_stream(
            &config.config(),
            move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                // задержка вывода: когда прозвучит то, что забираем сейчас
                let ts = info.timestamp();
                if let Some(d) = ts.playback.duration_since(&ts.callback) {
                    let frames = (d.as_secs_f64() * 48_000.0) as i64;
                    shared.output_latency_frames.store(frames, Ordering::Relaxed);
                }
                if shared.flush.swap(false, Ordering::AcqRel) {
                    while ring.pop().is_ok() {}
                    vs_primed = false;
                    vs_phase = 0.0;
                }
                let out_frames = data.len() / device_channels;
                if !transport.is_playing() {
                    data.fill(0.0);
                    return;
                }
                // Единый varispeed-путь (п.4): шаг = device_step × |rate|,
                // линейная интерполяция (ядро kt_engine::varispeed_fill,
                // тест 32). Направление колбэк НЕ знает (J4): при реверсе
                // материал в кольце уже развёрнут render-потоком, знак
                // применяется при продвижении playhead. rate≈0 → тишина,
                // ноль потребления (playhead стоит). Тест 45.
                let rate = f32::from_bits(shared.playback_rate.load(Ordering::Acquire));
                let Some(eff_step) = kt_engine::callback_step(step, rate) else {
                    data.fill(0.0);
                    return;
                };
                // пара L/R атомарно: полкадра из кольца не забираем — иначе
                // перманентный swap каналов (латентный дефект прежнего кода,
                // тест 45); underrun честно считается
                let mut next = || -> Option<(f32, f32)> {
                    if ring.slots() < 2 {
                        shared.callback_underruns.fetch_add(1, Ordering::Relaxed);
                        return None;
                    }
                    match (ring.pop(), ring.pop()) {
                        (Ok(l), Ok(r)) => Some((l, r)),
                        _ => None,
                    }
                };
                let consumed = kt_engine::varispeed_fill(
                    &mut next,
                    eff_step,
                    out_frames,
                    &mut vs_prev,
                    &mut vs_cur,
                    &mut vs_primed,
                    &mut vs_phase,
                    |f, (l, r)| {
                        for ch in 0..device_channels {
                            data[f * device_channels + ch] = if ch == 0 { l } else { r };
                        }
                    },
                );
                shared.consumed_frames.fetch_add(consumed, Ordering::AcqRel);
            },
            |e| eprintln!("cpal error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(AudioOutput { _stream: stream, device_rate })
}
