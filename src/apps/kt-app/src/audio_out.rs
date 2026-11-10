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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// How long the device stays open after the transport stopped (kept-mix-settle D5): the device's
/// own buffer has played out long before, and a Space released and pressed again within it does not
/// pay a reopen. Past it the stream is dropped and nothing of the sound wakes on a pause.
pub const CLOSE_AFTER_REST: std::time::Duration = std::time::Duration::from_secs(1);

/// The sound output: a thread of its own (`audio-output`) owns the device's stream, opens it when
/// the transport plays and drops it once the transport has stood for [`CLOSE_AFTER_REST`]
/// (kept-mix-settle, found-defects 274: open, cpal's ALSA worker and PipeWire's data loop woke
/// 24 and 48 times a second on a pause handing out silence). `cpal::Stream` is not `Send` and its
/// open blocks: the UI thread never opens it.
pub struct AudioOutput {
    ctl: Arc<OutputCtl>,
    wake: Arc<kt_engine::Wake>,
    thread: Option<std::thread::JoinHandle<()>>,
}

/// What the output thread tells the rest of the program.
#[derive(Default)]
pub struct OutputCtl {
    stop: AtomicBool,
    /// A stream is open now.
    pub open: AtomicBool,
    /// Streams opened so far.
    pub opens: AtomicU64,
    /// The thread's passes: a pause parks it, so on a settled pause this stands.
    pub passes: AtomicU64,
    /// The last open: µs it took, µs from its start to the first callback (0 — none came within a
    /// second after the open returned).
    pub open_us: AtomicU64,
    pub first_callback_us: AtomicU64,
    /// An error the UI has not shown yet (an open at Play, or the stream's own while open).
    has_error: AtomicBool,
    error: Mutex<Option<String>>,
}

impl OutputCtl {
    fn report(&self, e: String) {
        eprintln!("audio output: {e}");
        kt_time::trace::ev(kt_time::trace::Ev::Play).aux(kt_time::trace::play::DEVICE_ERROR).emit();
        *self.error.lock().unwrap() = Some(e);
        self.has_error.store(true, Ordering::Release);
    }
}

impl AudioOutput {
    /// A device error not yet shown (the UI asks once a frame: an atomic until there is one).
    pub fn take_error(&self) -> Option<String> {
        if !self.ctl.has_error.swap(false, Ordering::AcqRel) {
            return None;
        }
        self.ctl.error.lock().unwrap().take()
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn ctl(&self) -> &OutputCtl {
        &self.ctl
    }
}

impl Drop for AudioOutput {
    fn drop(&mut self) {
        self.ctl.stop.store(true, Ordering::Release);
        self.wake.notify();
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// What the callback owns while a stream is open: the ring's consumer. It comes back to the output
/// thread when the stream drops the callback ([`Sink`]'s drop sends it), so a closed device keeps the
/// ring for the next open; nothing in the callback locks.
pub struct Feed {
    ring: rtrb::Consumer<f32>,
}

/// The callback's state for one open stream (kept-mix-settle D5).
pub struct Sink {
    feed: Option<Feed>,
    back: std::sync::mpsc::Sender<Feed>,
    shared: Arc<AudioShared>,
    transport: Arc<Transport>,
    /// When the first callback came, ns after `opened` (0 — not yet).
    first: Arc<AtomicU64>,
    opened: std::time::Instant,
}

impl Drop for Sink {
    fn drop(&mut self) {
        if let Some(f) = self.feed.take() {
            let _ = self.back.send(f);
        }
    }
}

impl Sink {
    /// One device callback through `edge` (`device_channels` interleaved). Invariant 6: no
    /// allocations, no locks; the playhead advances by the engine frames handed out.
    pub fn fill(&mut self, edge: &mut OutputEdge, data: &mut [f32], device_channels: usize) {
        if self.first.load(Ordering::Relaxed) == 0 {
            self.first.store((self.opened.elapsed().as_nanos() as u64).max(1), Ordering::Relaxed);
        }
        let shared = &self.shared;
        let Some(feed) = self.feed.as_mut() else {
            data.fill(0.0);
            return;
        };
        let ring = &mut feed.ring;
        shared.callback_frames.store((data.len() / device_channels.max(1)) as i64, Ordering::Relaxed);
        if shared.flush.swap(false, Ordering::AcqRel) {
            while ring.pop().is_ok() {}
            edge.reset();
        }
        if !self.transport.is_playing() {
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
        // Nothing has played yet and the render is waiting for the start's audio: the
        // clock stands by design (invariant 6), and an empty pass here is no underrun.
        if !edge.started() && ring.slots() < 2 {
            data.fill(0.0);
            return;
        }
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
    }
}

/// A device the output thread opens: cpal's default output, or a test's double.
pub trait Device: Send + 'static {
    /// Open and start a stream whose callback calls [`Sink::fill`]; the guard returned keeps it
    /// open and closes it when dropped (it may not be `Send`: it lives on the output thread). On an
    /// error the sink must have been dropped, so its feed came back.
    fn open(&mut self, sink: Sink, ctl: &Arc<OutputCtl>) -> Result<Box<dyn std::any::Any>, String>;
}

/// The system's default output through cpal.
pub struct CpalDevice;

impl Device for CpalDevice {
    fn open(&mut self, mut sink: Sink, ctl: &Arc<OutputCtl>) -> Result<Box<dyn std::any::Any>, String> {
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
            return Err(format!("sample format {:?} is not supported (need f32)", config.sample_format()));
        }
        // The edge is built BEFORE the stream, for the device this open found: every buffer is
        // allocated here, none in the callback (invariant 6). The varispeed kernel always runs at
        // the engine rate (step = |rate|); the device rate is the edge's business.
        let mut edge = OutputEdge::new(device_rate, device_channels, MAX_CALLBACK_FRAMES)?;
        let edge_latency = edge.latency_frames();
        let shared = sink.shared.clone();
        let errs = ctl.clone();
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
                    sink.fill(&mut edge, data, device_channels);
                },
                move |e| errs.report(format!("the sound device: {e}")),
                None,
            )
            .map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok(Box::new(stream))
    }
}

/// Start the sound output on the system's default device: it opens on Play.
pub fn start(ring: rtrb::Consumer<f32>, player: &kt_engine::Player) -> AudioOutput {
    start_with(CpalDevice, ring, player.shared.clone(), player.transport.clone(), player.wake(), CLOSE_AFTER_REST)
}

/// [`start`] on any device, with the rest it waits before closing.
pub fn start_with<D: Device>(
    device: D,
    ring: rtrb::Consumer<f32>,
    shared: Arc<AudioShared>,
    transport: Arc<Transport>,
    wake: Arc<kt_engine::Wake>,
    close_after: std::time::Duration,
) -> AudioOutput {
    let ctl = Arc::new(OutputCtl::default());
    let thread = {
        let (ctl, wake) = (ctl.clone(), wake.clone());
        std::thread::Builder::new()
            .name("audio-output".into())
            .spawn(move || output_loop(device, Feed { ring }, shared, transport, wake, ctl, close_after))
            .expect("the audio output thread starts")
    };
    AudioOutput { ctl, wake, thread: Some(thread) }
}

/// The output thread: parks on the player's wake with no clock; opens on play, closes after rest.
fn output_loop<D: Device>(
    mut device: D,
    feed: Feed,
    shared: Arc<AudioShared>,
    transport: Arc<Transport>,
    wake: Arc<kt_engine::Wake>,
    ctl: Arc<OutputCtl>,
    close_after: std::time::Duration,
) {
    use kt_time::trace::{self, play};
    let (back, returned) = std::sync::mpsc::channel::<Feed>();
    let mut feed = Some(feed);
    let mut stream: Option<(Box<dyn std::any::Any>, std::time::Instant)> = None;
    let mut rest_since: Option<std::time::Instant> = None;
    // an open failed for the Play of this count: the next Play tries again
    let mut failed: Option<u64> = None;
    let first = Arc::new(AtomicU64::new(0));
    loop {
        let seen = wake.seen();
        ctl.passes.fetch_add(1, Ordering::Relaxed);
        if ctl.stop.load(Ordering::Acquire) {
            break;
        }
        // closed, the ring is ours: a seek's flush is done here, so the first callback of the next
        // open does not drop the new start's pass (D6)
        if let Some(f) = feed.as_mut()
            && shared.flush.swap(false, Ordering::AcqRel)
        {
            while f.ring.pop().is_ok() {}
        }
        if transport.is_playing() {
            rest_since = None;
            let plays = shared.plays.load(Ordering::Acquire);
            if stream.is_none() && failed != Some(plays) {
                let Some(f) = feed.take() else {
                    failed = Some(plays);
                    ctl.report("the sound device kept the ring after it closed".into());
                    continue;
                };
                first.store(0, Ordering::Relaxed);
                let opened = std::time::Instant::now();
                let sink = Sink {
                    feed: Some(f),
                    back: back.clone(),
                    shared: shared.clone(),
                    transport: transport.clone(),
                    first: first.clone(),
                    opened,
                };
                match device.open(sink, &ctl) {
                    Ok(s) => {
                        let open_us = opened.elapsed().as_micros() as u64;
                        // the first callback: when the first sound can leave (D7); bounded, once a Play
                        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
                        while first.load(Ordering::Relaxed) == 0 && std::time::Instant::now() < deadline {
                            std::thread::sleep(std::time::Duration::from_millis(1));
                        }
                        let first_us = first.load(Ordering::Relaxed) / 1000;
                        ctl.open_us.store(open_us, Ordering::Relaxed);
                        ctl.first_callback_us.store(first_us, Ordering::Relaxed);
                        ctl.opens.fetch_add(1, Ordering::Relaxed);
                        ctl.open.store(true, Ordering::Release);
                        trace::ev(trace::Ev::Play).b(open_us).c(first_us).aux(play::DEVICE_OPEN).emit();
                        stream = Some((s, opened));
                    }
                    Err(e) => {
                        failed = Some(plays);
                        feed = returned.recv_timeout(std::time::Duration::from_secs(1)).ok();
                        ctl.report(format!("the sound device did not open: {e}"));
                    }
                }
            }
        } else {
            if stream.is_some() {
                let since = *rest_since.get_or_insert_with(std::time::Instant::now);
                let left = close_after.saturating_sub(since.elapsed());
                if !left.is_zero() {
                    wake.wait_timeout(left);
                    continue;
                }
                let (s, opened) = stream.take().expect("open");
                drop(s);
                feed = returned.recv_timeout(std::time::Duration::from_secs(2)).ok();
                ctl.open.store(false, Ordering::Release);
                trace::ev(trace::Ev::Play).b(opened.elapsed().as_millis() as u64).aux(play::DEVICE_CLOSE).emit();
                rest_since = None;
                continue;
            }
        }
        wake.wait_since(seen);
    }
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

    /// Has a frame of this start reached the output?
    pub fn started(&self) -> bool {
        self.vs.primed
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

    // ---- the device opens on Play (kept-mix-settle D5–D7, found-defects 274) ----

    use kt_media::synthetic::{AudioPattern, SyntheticSource};
    use kt_model::*;
    use kt_time::Tick;
    use std::time::Duration;

    const SEC: i64 = 48_000;

    /// A device double: its callback is a thread calling [`Sink::fill`] on 1024-frame buffers every
    /// 5 ms through a 48 kHz stereo edge; what it hands out is kept, the fills' allocations counted.
    /// `fail` opens fail first; dropping the guard stops the thread, which drops the sink.
    #[derive(Clone, Default)]
    struct Double {
        fail: Arc<AtomicU64>,
        heard: Arc<Mutex<Vec<f32>>>,
        allocs: Arc<AtomicU64>,
    }

    struct Guard {
        stop: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            self.stop.store(true, Ordering::Release);
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
        }
    }

    impl Device for Double {
        fn open(&mut self, sink: Sink, _ctl: &Arc<OutputCtl>) -> Result<Box<dyn std::any::Any>, String> {
            if self.fail.load(Ordering::Acquire) > 0 {
                self.fail.fetch_sub(1, Ordering::AcqRel);
                return Err("no device here (test)".into());
            }
            let stop = Arc::new(AtomicBool::new(false));
            let (heard, allocs, halt) = (self.heard.clone(), self.allocs.clone(), stop.clone());
            let thread = std::thread::spawn(move || {
                let mut sink = sink;
                let mut edge = OutputEdge::new(AUDIO_SAMPLE_RATE, 2, MAX_CALLBACK_FRAMES).unwrap();
                let mut buf = vec![0f32; 1024 * 2];
                while !halt.load(Ordering::Acquire) {
                    let before = ALLOCS.load(Ordering::Relaxed);
                    MEASURING.with(|m| m.set(true));
                    sink.fill(&mut edge, &mut buf, 2);
                    MEASURING.with(|m| m.set(false));
                    allocs.fetch_add(ALLOCS.load(Ordering::Relaxed) - before, Ordering::Relaxed);
                    heard.lock().unwrap().extend_from_slice(&buf);
                    std::thread::sleep(Duration::from_millis(5));
                }
            });
            Ok(Box::new(Guard { stop, thread: Some(thread) }))
        }
    }

    fn project() -> Project {
        let mut p = Project::new(ProjectSettings {
            fps: 30,
            width: 320,
            height: 180,
            sample_rate: 48000,
            pitch_engine: Default::default(),
            default_shape: Default::default(),
            conform: Default::default(),
            settings_explicit: false,
        });
        p.media.push(MediaRef::new(MediaId(1), "synth:sine"));
        p.clips = vec![ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            geometry: Default::default(),
            geometry_pos: 0,
            geometry_auto: Vec::new(),
            masks: Default::default(),
            id: ClipId(1),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick::ZERO,
            duration: Tick::from_audio_frames(30 * SEC),
            source_in: Tick::ZERO,
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx_bypass: vec![],
            fx_auto: vec![],
            fx: vec![],
            opacity: 1.0,
            fx_off: false,
        }];
        p.ensure_tracks(LaneKind::Audio, 1);
        p
    }

    fn source() -> Box<dyn kt_media::MediaSource> {
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 330.0 }))
    }

    fn player_with(dev: Double, close_after: Duration) -> (kt_engine::Player, AudioOutput) {
        let (player, ring) = kt_engine::Player::new(project(), vec![(MediaId(1), source())]);
        let out = start_with(dev, ring, player.shared.clone(), player.transport.clone(), player.wake(), close_after);
        (player, out)
    }

    fn wait(what: &str, mut ok: impl FnMut() -> bool) {
        let deadline = kt_time::test_deadline();
        while !ok() {
            assert!(std::time::Instant::now() < deadline, "{what}");
            std::thread::sleep(Duration::from_millis(2));
        }
    }

    /// Does the output thread stand still over `d` (a pause parks it with no clock)?
    fn parked(out: &AudioOutput, d: Duration) -> bool {
        let p0 = out.ctl().passes.load(Ordering::Relaxed);
        std::thread::sleep(d);
        out.ctl().passes.load(Ordering::Relaxed) == p0
    }

    /// t2380: nothing is open before Play and the output thread stands; Play opens the device and
    /// the clock runs; a pause closes it after the rest, and the thread stands again.
    #[test]
    fn t2380_the_device_opens_on_play_and_closes_at_rest() {
        let rest = Duration::from_millis(150);
        let (player, out) = player_with(Double::default(), rest);
        let ctl = out.ctl();
        std::thread::sleep(Duration::from_millis(50));
        assert!(!ctl.open.load(Ordering::Acquire), "a device open before Play");
        assert!(parked(&out, Duration::from_millis(300)), "the output thread wakes on a pause");
        player.play();
        wait("Play opened the device", || ctl.open.load(Ordering::Acquire));
        let head = player.transport.playhead_frames();
        wait("the clock runs", || player.transport.playhead_frames() > head + SEC / 10);
        assert!(ctl.first_callback_us.load(Ordering::Relaxed) > 0, "the first callback was timed");
        player.pause();
        let paused = std::time::Instant::now();
        wait("the device closed at rest", || !ctl.open.load(Ordering::Acquire));
        assert!(paused.elapsed() >= rest, "closed before its rest: {:?}", paused.elapsed());
        assert!(parked(&out, Duration::from_millis(300)), "the output thread wakes on a pause after a play");
        assert_eq!(ctl.opens.load(Ordering::Relaxed), 1);
    }

    /// t2381: the ring comes back when the stream closes: the next Play opens again and sounds.
    #[test]
    fn t2381_the_ring_survives_a_close() {
        let dev = Double::default();
        let (player, out) = player_with(dev.clone(), Duration::from_millis(50));
        let ctl = out.ctl();
        for round in 1..=3u64 {
            player.play();
            wait("Play opened the device", || ctl.opens.load(Ordering::Relaxed) == round);
            let head = player.transport.playhead_frames();
            wait("the clock runs", || player.transport.playhead_frames() > head + SEC / 10);
            player.pause();
            wait("the device closed", || !ctl.open.load(Ordering::Acquire));
        }
        let heard = dev.heard.lock().unwrap();
        assert!(heard.iter().filter(|v| v.abs() > 1e-3).count() > 3 * SEC as usize / 10, "the reopened device did not sound");
        assert!(out.take_error().is_none());
    }

    /// t2382: a seek while the device is closed is flushed by the output thread: the ring holds no
    /// frame of the old place when the next Play opens it.
    #[test]
    fn t2382_a_seek_while_closed_is_flushed() {
        let (player, out) = player_with(Double::default(), Duration::from_millis(30));
        let ctl = out.ctl();
        player.play();
        wait("Play opened the device", || ctl.open.load(Ordering::Acquire));
        player.pause();
        wait("the device closed", || !ctl.open.load(Ordering::Acquire));
        player.seek(Tick::from_audio_frames(10 * SEC));
        wait("the flush was done while closed", || !player.shared.flush.load(Ordering::Acquire));
        assert!(!ctl.open.load(Ordering::Acquire), "a seek opened the device");
    }

    /// t2383: a device that does not open at Play says so (the bell takes it), the clock stands with
    /// no device, and the next Play tries again with the ring intact.
    #[test]
    fn t2383_an_open_error_at_play_is_reported_and_retried() {
        let dev = Double::default();
        dev.fail.store(1, Ordering::Release);
        let (player, out) = player_with(dev, Duration::from_millis(30));
        let ctl = out.ctl();
        let head = player.transport.playhead_frames();
        player.play();
        let mut err = None;
        wait("the open's error was reported", || {
            err = out.take_error();
            err.is_some()
        });
        assert!(err.unwrap().contains("no device here"));
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(player.transport.playhead_frames(), head, "the clock ran with no device");
        player.pause();
        player.play();
        wait("the next Play opened the device", || ctl.open.load(Ordering::Acquire));
        wait("the clock runs", || player.transport.playhead_frames() > head + SEC / 10);
    }

    /// t2384: the first sound after the open is the start's first frame: before it only silence
    /// (time stood), from it the engine's frames byte for byte; the callback allocates nothing.
    #[test]
    fn t2384_the_first_sound_after_the_open_is_the_starts_first_frame() {
        let dev = Double::default();
        let (player, out) = player_with(dev.clone(), Duration::from_millis(30));
        let at = 5 * SEC + 333;
        player.seek(Tick::from_audio_frames(at));
        wait("the seek was flushed", || !player.shared.flush.load(Ordering::Acquire));
        player.play();
        let head = player.transport.playhead_frames();
        wait("the clock runs", || player.transport.playhead_frames() > head + SEC / 2);
        player.pause();
        drop(out);
        let heard = dev.heard.lock().unwrap().clone();
        let r = kt_engine::AudioRenderer::new(&project());
        let mut f = kt_engine::DirectFetcher::new(None);
        f.add_source(MediaId(1), source());
        let n = 8192usize;
        let mut want = vec![0f32; n * 2];
        r.render_frames(&mut f, player.transport.last_seek_frames(), &mut want);
        let k = heard.iter().position(|v| *v != 0.0).expect("the device sounded");
        let j = want.iter().position(|v| *v != 0.0).unwrap();
        assert_eq!((k % 2, j % 2), (0, 0));
        // the varispeed kernel primes on the first frame and hands out from the next (t284: the
        // two frames of interpolation priming); nothing else stands between the ring and the device
        let from = (0..=1).find(|m| heard[k..k + 4096 * 2] == want[j + m * 2..j + m * 2 + 4096 * 2]);
        assert!(
            from.is_some(),
            "the first sound is not the start's frames: {:?} against {:?}",
            &heard[k..k + 6],
            &want[j..j + 6]
        );
        assert_eq!(dev.allocs.load(Ordering::Relaxed), 0, "the callback allocated");
    }
}
