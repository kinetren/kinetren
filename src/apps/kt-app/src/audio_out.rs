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
    let device = host.default_output_device().ok_or("нет аудио-устройства")?;

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
            "формат сэмплов {:?} не поддержан (нужен f32)",
            config.sample_format()
        ));
    }
    // Наивная подгонка под чужой rate (позиционный повтор/пропуск кадров).
    // На 48k-устройствах (норма для PipeWire) путь тождественный.
    let step = AUDIO_SAMPLE_RATE as f64 / device_rate as f64;

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
                }
                let out_frames = data.len() / device_channels;
                if !transport.is_playing() {
                    data.fill(0.0);
                    return;
                }
                if (step - 1.0).abs() < 1e-9 {
                    // прямой путь: device 48k
                    let mut consumed = 0i64;
                    for f in 0..out_frames {
                        let (l, r) = match (ring.pop(), ring.pop()) {
                            (Ok(l), Ok(r)) => {
                                consumed += 1;
                                (l, r)
                            }
                            _ => {
                                shared.callback_underruns.fetch_add(1, Ordering::Relaxed);
                                (0.0, 0.0)
                            }
                        };
                        for ch in 0..device_channels {
                            data[f * device_channels + ch] = if ch == 0 { l } else { r };
                        }
                    }
                    shared.consumed_frames.fetch_add(consumed, Ordering::AcqRel);
                } else {
                    // редкий путь: устройство не 48k — тянем с дробным шагом
                    let mut acc = 0f64;
                    let mut consumed = 0i64;
                    let mut cur = (0.0f32, 0.0f32);
                    let mut have = false;
                    for f in 0..out_frames {
                        acc += step;
                        while acc >= 1.0 || !have {
                            match (ring.pop(), ring.pop()) {
                                (Ok(l), Ok(r)) => {
                                    cur = (l, r);
                                    consumed += 1;
                                    have = true;
                                }
                                _ => {
                                    shared.callback_underruns.fetch_add(1, Ordering::Relaxed);
                                    cur = (0.0, 0.0);
                                    have = true;
                                }
                            }
                            if acc >= 1.0 {
                                acc -= 1.0;
                            } else {
                                break;
                            }
                        }
                        for ch in 0..device_channels {
                            data[f * device_channels + ch] = if ch == 0 { cur.0 } else { cur.1 };
                        }
                    }
                    shared.consumed_frames.fetch_add(consumed, Ordering::AcqRel);
                }
            },
            |e| eprintln!("cpal error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(AudioOutput { _stream: stream, device_rate })
}
