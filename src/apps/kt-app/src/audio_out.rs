//! cpal output. Invariant No. 6: the callback is the master clock. In the callback there are no allocations,
//! no locks: only reading the SPSC ring, an atomic counter of consumed
//! frames (the audio-render thread advances the playhead by it) and a flush flag
//! (after a seek we discard the tail of the old position).
//!
//! The OUTPUT EDGE (windows-build, transport): the engine gives f32 48 kHz/2ch (invariant 1).
//! A device that cannot run at 48 kHz stereo — WASAPI shared mode reports only its mix
//! format: 44.1 kHz, or 5.1/7.1 — is served HERE and only here: a band-limited fixed-ratio
//! resampler after the varispeed kernel, and a channel map (L→front left, R→front right,
//! silence elsewhere, mono = (L+R)/2). The playhead still counts ENGINE frames consumed.
//! Before, a 44.1 kHz device got the varispeed kernel's linear interpolation (audible
//! aliasing) and a 5.1 device the right channel in centre, LFE and every surround.

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

    // we prefer 48k stereo f32 — then the callback needs no conversions at all; then 48k with
    // any channel count (only a channel map); then the device's own format (resampled)
    let mut chosen: Option<cpal::SupportedStreamConfig> = None;
    if let Ok(configs) = device.supported_output_configs() {
        let configs: Vec<_> = configs
            .filter(|c| {
                c.sample_format() == cpal::SampleFormat::F32
                    && c.min_sample_rate().0 <= AUDIO_SAMPLE_RATE
                    && c.max_sample_rate().0 >= AUDIO_SAMPLE_RATE
            })
            .collect();
        let best = configs
            .iter()
            .find(|c| c.channels() == AUDIO_CHANNELS as u16)
            .or_else(|| configs.first());
        chosen = best.map(|c| c.with_sample_rate(cpal::SampleRate(AUDIO_SAMPLE_RATE)));
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
    // The edge is built BEFORE the stream: every buffer is allocated here, none in the callback
    // (invariant 6). The varispeed kernel now always runs at the engine rate (step = |rate|);
    // the device rate is the edge's business.
    let mut edge = OutputEdge::new(device_rate, device_channels, MAX_CALLBACK_FRAMES)?;
    let edge_latency = edge.latency_frames();
    let stream = device
        .build_output_stream(
            &config.config(),
            move |data: &mut [f32], info: &cpal::OutputCallbackInfo| {
                // output latency: when what we take now will sound
                let ts = info.timestamp();
                if let Some(d) = ts.playback.duration_since(&ts.callback) {
                    let frames = (d.as_secs_f64() * 48_000.0) as i64 + edge_latency;
                    shared.output_latency_frames.store(frames, Ordering::Relaxed);
                }
                if shared.flush.swap(false, Ordering::AcqRel) {
                    while ring.pop().is_ok() {}
                    edge.reset();
                }
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
                let Some(eff_step) = kt_engine::callback_step(1.0, rate) else {
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
                let consumed = edge.fill(data, eff_step, &mut next);
                shared.consumed_frames.fetch_add(consumed, Ordering::AcqRel);
            },
            |e| eprintln!("cpal error: {e}"),
            None,
        )
        .map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(AudioOutput { _stream: stream, device_rate })
}

/// The largest callback the edge expects in one piece; a larger one is served in pieces.
const MAX_CALLBACK_FRAMES: usize = 8192;
/// Engine frames per resampler chunk (10 ms at 48 kHz).
const EDGE_CHUNK_IN: usize = 480;

/// The varispeed kernel's state between callbacks.
#[derive(Default)]
struct Varispeed {
    prev: (f32, f32),
    cur: (f32, f32),
    primed: bool,
    phase: f64,
}

/// The conversion between the engine (48 kHz stereo) and the device (see the module doc).
pub struct OutputEdge {
    channels: usize,
    vs: Varispeed,
    rs: Option<Resample>,
}

struct Resample {
    fft: rubato::FftFixedInOut<f32>,
    input: [Vec<f32>; 2],
    output: [Vec<f32>; 2],
    /// Device-rate stereo frames resampled but not yet given to the device.
    fifo_l: Vec<f32>,
    fifo_r: Vec<f32>,
    head: usize,
    len: usize,
    delay_engine: i64,
}

impl OutputEdge {
    pub fn new(device_rate: u32, channels: usize, _max_callback: usize) -> Result<Self, String> {
        if channels == 0 {
            return Err("the output device has no channels".into());
        }
        let rs = if device_rate == AUDIO_SAMPLE_RATE {
            None
        } else {
            use rubato::Resampler;
            let fft = rubato::FftFixedInOut::<f32>::new(
                AUDIO_SAMPLE_RATE as usize,
                device_rate as usize,
                EDGE_CHUNK_IN,
                2,
            )
            .map_err(|e| format!("output resampler {AUDIO_SAMPLE_RATE} to {device_rate} Hz: {e}"))?;
            let mut fft = fft;
            let n_in = fft.input_frames_max();
            let n_out = fft.output_frames_max();
            let delay_engine =
                (fft.output_delay() as i64 * AUDIO_SAMPLE_RATE as i64) / device_rate as i64;
            let input = [vec![0.0; n_in], vec![0.0; n_in]];
            let mut output = [vec![0.0; n_out], vec![0.0; n_out]];
            // One pass on silence HERE, then a reset: the FFT allocates its scratch lazily on
            // the first process, and on some CPUs (another SIMD path of rustfft — the CI runner,
            // not this workshop) that happened in the first callbacks: 4 allocations, invariant 6.
            let _ = fft.process_into_buffer(&input, &mut output, None);
            fft.reset();
            Some(Resample {
                fft,
                input,
                output,
                fifo_l: vec![0.0; n_out],
                fifo_r: vec![0.0; n_out],
                head: 0,
                len: 0,
                delay_engine,
            })
        };
        Ok(OutputEdge { channels, vs: Varispeed::default(), rs })
    }

    /// The extra delay of the edge, in engine frames (latency compensation adds it).
    pub fn latency_frames(&self) -> i64 {
        self.rs.as_ref().map(|r| r.delay_engine).unwrap_or(0)
    }

    /// After a seek: the old position's tail must not sound.
    pub fn reset(&mut self) {
        self.vs = Varispeed::default();
        if let Some(r) = self.rs.as_mut() {
            use rubato::Resampler;
            r.fft.reset();
            r.head = 0;
            r.len = 0;
        }
    }

    /// Fill one callback buffer (interleaved, `channels` per frame). Returns the ENGINE
    /// frames consumed from the ring — what advances the playhead (invariant 6). No
    /// allocations, no locks.
    pub fn fill(&mut self, data: &mut [f32], step: f64, next: &mut dyn FnMut() -> Option<(f32, f32)>) -> i64 {
        let ch = self.channels;
        let out_frames = data.len() / ch;
        let vs = &mut self.vs;
        let Some(r) = self.rs.as_mut() else {
            return kt_engine::varispeed_fill(
                next,
                step,
                out_frames,
                &mut vs.prev,
                &mut vs.cur,
                &mut vs.primed,
                &mut vs.phase,
                |f, lr| put(data, ch, f, lr),
            );
        };
        use rubato::Resampler;
        let mut consumed = 0i64;
        let mut written = 0usize;
        while written < out_frames {
            if r.len == 0 {
                // one chunk of engine frames through the kernel, then through the resampler
                let n_in = r.fft.input_frames_next();
                let [il, ir] = &mut r.input;
                consumed += kt_engine::varispeed_fill(
                    next,
                    step,
                    n_in,
                    &mut vs.prev,
                    &mut vs.cur,
                    &mut vs.primed,
                    &mut vs.phase,
                    |f, (l, rr)| {
                        il[f] = l;
                        ir[f] = rr;
                    },
                );
                let (_, n_out) = r
                    .fft
                    .process_into_buffer(&r.input, &mut r.output, None)
                    .unwrap_or((0, 0));
                r.fifo_l[..n_out].copy_from_slice(&r.output[0][..n_out]);
                r.fifo_r[..n_out].copy_from_slice(&r.output[1][..n_out]);
                r.head = 0;
                r.len = n_out;
                if n_out == 0 {
                    break;
                }
            }
            let take = r.len.min(out_frames - written);
            for i in 0..take {
                put(data, ch, written + i, (r.fifo_l[r.head + i], r.fifo_r[r.head + i]));
            }
            r.head += take;
            r.len -= take;
            written += take;
        }
        for x in &mut data[written * ch..] {
            *x = 0.0;
        }
        consumed
    }
}

/// One engine frame into a device frame: L→front left, R→front right, silence in every other
/// channel; a mono device gets (L+R)/2.
fn put(data: &mut [f32], ch: usize, f: usize, (l, r): (f32, f32)) {
    let frame = &mut data[f * ch..(f + 1) * ch];
    match ch {
        1 => frame[0] = 0.5 * (l + r),
        _ => {
            frame[0] = l;
            frame[1] = r;
            for x in &mut frame[2..] {
                *x = 0.0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU64;

    /// Allocations made on the thread that runs the fills — only while it is measuring
    /// (another thread of the harness allocating at the same time must not count: on the CI
    /// runner the process-wide count saw 4 that the fills did not make). The flag is a
    /// const-initialised thread-local: reading it does not allocate.
    struct Counting;
    static ALLOCS: AtomicU64 = AtomicU64::new(0);
    thread_local! {
        static MEASURING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    }
    unsafe impl std::alloc::GlobalAlloc for Counting {
        unsafe fn alloc(&self, l: std::alloc::Layout) -> *mut u8 {
            if MEASURING.with(|m| m.get()) {
                ALLOCS.fetch_add(1, Ordering::Relaxed);
            }
            unsafe { std::alloc::System.alloc(l) }
        }
        unsafe fn dealloc(&self, p: *mut u8, l: std::alloc::Layout) {
            unsafe { std::alloc::System.dealloc(p, l) }
        }
    }
    #[global_allocator]
    static A: Counting = Counting;

    /// Run `edge` for `frames` device frames in callbacks cycling through `sizes`; the source
    /// is a 1 kHz sine on L and silence on R at 48 kHz. Returns (output, consumed, allocs).
    fn run(edge: &mut OutputEdge, ch: usize, frames: usize, sizes: &[usize]) -> (Vec<f32>, i64, u64) {
        let mut n = 0u64;
        let mut next = move || -> Option<(f32, f32)> {
            let t = n as f64 / 48_000.0;
            n += 1;
            Some(((2.0 * std::f64::consts::PI * 1000.0 * t).sin() as f32 * 0.5, 0.0))
        };
        let mut out = vec![0.0f32; frames * ch];
        let mut buf = vec![0.0f32; 1024 * ch];
        let mut consumed = 0;
        let mut done = 0;
        let mut i = 0;
        let before = ALLOCS.load(Ordering::Relaxed);
        let mut allocs = 0;
        while done < frames {
            let k = sizes[i % sizes.len()].min(frames - done);
            i += 1;
            // only the fill is measured: the copy into `out` is the test's own bookkeeping
            MEASURING.with(|m| m.set(true));
            consumed += edge.fill(&mut buf[..k * ch], 1.0, &mut next);
            MEASURING.with(|m| m.set(false));
            out[done * ch..(done + k) * ch].copy_from_slice(&buf[..k * ch]);
            done += k;
        }
        allocs += ALLOCS.load(Ordering::Relaxed) - before;
        (out, consumed, allocs)
    }

    /// t857 (windows-build, transport): 44.1 kHz and 6-channel devices are converted at the
    /// edge; a 48 kHz stereo device gets the engine's frames byte for byte; no allocation in
    /// the fills.
    #[test]
    fn t857_output_edge_converts_rate_and_channels() {
        let sizes = [441, 512, 1024];
        // per-thread counting: valid under cargo test's shared process as well
        let strict_allocs = true;

        // 44.1 kHz stereo: one second of device time
        let mut e = OutputEdge::new(44_100, 2, MAX_CALLBACK_FRAMES).unwrap();
        let (out, consumed, allocs) = run(&mut e, 2, 44_100, &sizes);
        let chunk = EDGE_CHUNK_IN as i64;
        assert!(
            (48_000..=48_000 + chunk + 2).contains(&consumed),
            "44 100 device frames take 48 000 engine frames (plus one chunk held in the FIFO and the \
             kernel's two priming frames): {consumed}"
        );
        if strict_allocs {
            assert_eq!(allocs, 0, "the 44.1 kHz fills allocated");
        }
        // spectrum of the last 16384 samples of L: the 1 kHz line, and nothing else within 60 dB
        let n = 16_384;
        let l: Vec<f32> = out.chunks(2).map(|f| f[0]).collect();
        let tail = &l[l.len() - n..];
        let mut planner = realfft::RealFftPlanner::<f64>::new();
        let fft = planner.plan_fft_forward(n);
        let mut input: Vec<f64> = tail
            .iter()
            .enumerate()
            .map(|(i, x)| {
                // Blackman-Harris: sidelobes far below 60 dB
                let a = 2.0 * std::f64::consts::PI * i as f64 / (n - 1) as f64;
                let w = 0.35875 - 0.48829 * a.cos() + 0.14128 * (2.0 * a).cos() - 0.01168 * (3.0 * a).cos();
                *x as f64 * w
            })
            .collect();
        let mut spec = fft.make_output_vec();
        fft.process(&mut input, &mut spec).unwrap();
        let mag: Vec<f64> = spec.iter().map(|c| c.norm()).collect();
        let hz = |bin: usize| bin as f64 * 44_100.0 / n as f64;
        let (peak_bin, peak) = mag.iter().enumerate().fold((0, 0.0), |a, (i, m)| if *m > a.1 { (i, *m) } else { a });
        assert!((hz(peak_bin) - 1000.0).abs() < 10.0, "the peak is at 1 kHz: {} Hz", hz(peak_bin));
        let worst = mag
            .iter()
            .enumerate()
            .filter(|(i, _)| (hz(*i) - 1000.0).abs() > 100.0)
            .map(|(_, m)| *m)
            .fold(0.0, f64::max);
        let db = 20.0 * (worst / peak).log10();
        assert!(db < -60.0, "aliasing products at {db:.1} dB (must be below -60 dB)");

        // 48 kHz, 6 channels: the sine in channel 0, exact zeros in 1..5
        let mut e = OutputEdge::new(48_000, 6, MAX_CALLBACK_FRAMES).unwrap();
        let (out, consumed, allocs) = run(&mut e, 6, 48_000, &sizes);
        // the kernel primes its interpolation with two frames (transport design: at rate 1,
        // 100 output frames consume 102)
        assert_eq!(consumed, 48_002);
        assert!(out.chunks(6).any(|f| f[0] != 0.0));
        assert!(out.chunks(6).all(|f| f[1..].iter().all(|x| *x == 0.0)), "R and the surrounds are silent");
        if strict_allocs {
            assert_eq!(allocs, 0, "the 6-channel fills allocated");
        }

        // 48 kHz stereo: byte for byte the engine's frames, through the same kernel call as
        // before the edge existed (after priming it emits from the engine's second frame)
        let mut e = OutputEdge::new(48_000, 2, MAX_CALLBACK_FRAMES).unwrap();
        let (out, consumed, _) = run(&mut e, 2, 4_800, &sizes);
        assert_eq!(consumed, 4_802);
        for (i, f) in out.chunks(2).enumerate() {
            let t = (i + 1) as f64 / 48_000.0;
            let want = ((2.0 * std::f64::consts::PI * 1000.0 * t).sin() as f32) * 0.5;
            assert_eq!(f[0].to_bits(), want.to_bits(), "frame {i}");
            assert_eq!(f[1], 0.0);
        }
    }
}
