//! cpal output. Invariant No. 6: the callback is the master clock. In the callback there are no allocations,
//! no locks: only reading the SPSC ring, an atomic counter of consumed
//! frames (the audio-render thread advances the playhead by it) and a flush flag
//! (after a seek we discard the tail of the old position).

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

    // we prefer 48k stereo f32 — then the callback needs no conversions at all
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
    // A naive adjustment to someone else's rate (positional repeat/skip of frames).
    // On 48k devices (the norm for PipeWire) the path is identity.
    let step = AUDIO_SAMPLE_RATE as f64 / device_rate as f64;

    // state of the varispeed interpolator between callbacks
    let mut vs_prev = (0.0f32, 0.0f32);
    let mut vs_cur = (0.0f32, 0.0f32);
    let mut vs_primed = false;
    let mut vs_phase = 0.0f64;
    let stream = device
        .build_output_stream(
            &config.config(),
            move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                // output latency: when what we take now will sound
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
                // A single varispeed path (item 4): step = device_step × |rate|,
                // linear interpolation (the kt_engine::varispeed_fill kernel,
                // test 32). The callback does NOT know the direction (J4): on reverse
                // the material in the ring is already reversed by the render thread, the sign
                // is applied when the playhead advances. rate≈0 → silence,
                // zero consumption (the playhead stands still). Test 45.
                let rate = f32::from_bits(shared.playback_rate.load(Ordering::Acquire));
                let Some(eff_step) = kt_engine::callback_step(step, rate) else {
                    data.fill(0.0);
                    return;
                };
                // the L/R pair atomically: we do not take half a frame from the ring — otherwise
                // a permanent channel swap (a latent defect of the former code,
                // test 45); the underrun is counted honestly
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
