//! Тесты 292-295 (Р.43, пп.304-305): ЦЕНА МЕСТА.
//!
//! Претензия пользователя, доведённая до причины: «неужели эти два реверса
//! НАСТОЛЬКО тяжёлые?» и «версия до otio, та же отладочная сборка, всё как
//! по маслу если регион больше реверсов, а внутри начала события 21 растут
//! underruns».
//!
//! Причин оказалось две, и обе структурные:
//! - `overlap_weight` перебирал ВЕСЬ монтаж НА КАЖДЫЙ КАДР, поэтому цена
//!   места росла линейно с размером ПРОЕКТА (события за час оттуда стоили
//!   столько же, сколько соседние);
//! - в хвосте реверба варп считался и выбрасывался гейтом.

use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher, FxStateStore};
use kt_media::AudioBurst;
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};
use std::sync::Arc;

const CH: usize = AUDIO_CHANNELS;
const BURST: i64 = AUDIO_BURST_FRAMES as i64;
const BODY_MS: i64 = 300;

/// Фетчер, считающий обращения к источнику.
struct Counting {
    inner: DirectFetcher,
    reads: u64,
}

impl Counting {
    fn new() -> Counting {
        let mut inner = DirectFetcher::new(None);
        inner.add_source(
            MediaId(1),
            Box::new(SyntheticSource::new(Tick(90_000 * 600), AudioPattern::Ramp)),
        );
        Counting { inner, reads: 0 }
    }
}

impl AudioFetcher for Counting {
    fn audio_burst(&mut self, media: MediaId, idx: i64) -> Option<Arc<AudioBurst>> {
        self.reads += 1;
        self.inner.audio_burst(media, idx)
    }
}

fn slowed() -> FxInstance {
    FxInstance::SlowedReverb {
        rate: 0.75,
        mix: 0.2,
        reverb_delay_ms: 85.0,
        damp: 0.2,
        decay: 0.84,
        wet_gain: 1.0,
        predelay_ms: 0.0,
        width: 1.0,
        frames: Default::default(),
        automation: vec![],
    }
}

/// `n` событий по 300 мс подряд на РАЗНЫХ дорожках, со slowed+reverb.
fn base(n: u64) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 60,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
    });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:ramp".into() });
    p.audio_lanes = n.max(1) as u32;
    for i in 0..n {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            id: ClipId(i + 1),
            link_group: None,
            kind: ClipKind::Audio,
            lane: i as u32,
            start: Tick::from_audio_frames(48 * BODY_MS * i as i64),
            duration: Tick::from_audio_frames(48 * BODY_MS),
            source_in: Tick::from_audio_frames(480_000),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: i % 3 == 0,
            fx: vec![slowed()],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.validate().unwrap();
    p
}

/// Те же события плюс `extra` ДАЛЁКИХ — за минуту от них, на общей дорожке.
fn with_distant(mut p: Project, extra: u64) -> Project {
    for i in 0..extra {
        p.clips.push(ClipEvent {
            r#gen: None,
            gen_auto: Vec::new(),
            overrides: Vec::new(),
            id: ClipId(10_000 + i),
            link_group: None,
            kind: ClipKind::Audio,
            lane: 0,
            start: Tick::from_audio_frames(48_000 * 60 + 480 * i as i64),
            duration: Tick::from_audio_frames(480),
            source_in: Tick::from_audio_frames(480_000),
            media: MediaId(1),
            gain_db: 0.0,
            reversed: false,
            fx: vec![],
            fx_bypass: vec![],
            fx_auto: vec![],
            opacity: 1.0,
            fx_off: false,
        });
    }
    p.validate().unwrap();
    p
}

/// Отрендерить [a, b) кадров; вернуть звук и число обращений к источнику.
fn render(p: &Project, a: i64, b: i64) -> (Vec<f32>, u64) {
    let r = AudioRenderer::new(p);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut f = Counting::new();
    let mut out = Vec::new();
    let mut buf = vec![0f32; BURST as usize * CH];
    let mut pos = a;
    while pos < b {
        r.render_frames_with(&mut f, &mut store, pos, &mut buf);
        out.extend_from_slice(&buf);
        pos += BURST;
    }
    (out, f.reads)
}

fn rms(v: &[f32]) -> f64 {
    (v.iter().map(|s| (*s as f64).powi(2)).sum::<f64>() / v.len().max(1) as f64).sqrt()
}

/// Тест 292 (п.305): ДАЛЁКИЕ события не меняют звук.
///
/// Сторож правильности карты перекрытий: она заменила перебор всего
/// монтажа, и набор накрывающих событий обязан остаться ТЕМ ЖЕ — вместе с
/// порядком (сложение f32 не ассоциативно, на этом стоит GOLDEN_AUDIO).
#[test]
fn t292_distant_clips_do_not_change_the_sound() {
    let a = 48 * BODY_MS * 2;
    let b = a + 48_000;
    let (plain, _) = render(&base(7), a, b);
    let (crowded, _) = render(&with_distant(base(7), 2000), a, b);
    assert!(rms(&plain) > 1e-4, "эталон не должен быть тишиной");
    assert_eq!(plain, crowded, "события за минуту оттуда не имеют права влиять на звук");
}

/// Тест 293 (п.305): цена места НЕ растёт с размером проекта.
///
/// До Р.43 `overlap_weight` перебирал весь монтаж на каждый кадр: 7 событий
/// стоили 3.6 мс на секунду звука, те же 7 плюс 2000 далёких — 92 мс, то
/// есть в 25 раз дороже при том же звуке. Порог здесь щедрый (втрое):
/// тест ловит возврат ЛИНЕЙНОГО перебора, а не колебания машины.
#[test]
fn t293_cost_does_not_scale_with_project_size() {
    let a = 48 * BODY_MS * 2;
    let b = a + 48_000;
    let best = |p: &Project| -> f64 {
        (0..3)
            .map(|_| {
                let t = std::time::Instant::now();
                let _ = render(p, a, b);
                t.elapsed().as_secs_f64()
            })
            .fold(f64::INFINITY, f64::min)
    };
    let small = best(&base(7));
    let big = best(&with_distant(base(7), 2000));
    assert!(
        big < small * 3.0,
        "2000 далёких событий не имеют права дорожать место: {:.1} мс против {:.1} мс \
         (до Р.43 было в 25 раз)",
        big * 1e3,
        small * 1e3
    );
}

/// Тест 294 (п.304): в хвосте реверба источник НЕ ЧИТАЕТСЯ.
///
/// За концом материала сухой сигнал обнуляется гейтом, поэтому всё, что
/// выдаст варп, будет выброшено. Считать его незачем — а раньше считался,
/// с sinc-ресемплером и чтением источника.
#[test]
fn t294_reverb_tail_does_not_touch_the_source() {
    let p = base(7);
    let bodies_end = 48 * BODY_MS * 7;
    let r = AudioRenderer::new(&p);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut f = Counting::new();
    let mut buf = vec![0f32; BURST as usize * CH];

    // ПЕРВЫЙ вход в хвост — это `hard_seek` с пре-роллом: он честно
    // проигрывает тело в мусор, чтобы реверб был прогрет. Чтения там
    // законны, и оптимизация их не трогает.
    let mut pos = bodies_end + BURST;
    r.render_frames_with(&mut f, &mut store, pos, &mut buf);
    pos += BURST;
    let after_preroll = f.reads;
    assert!(after_preroll > 0, "пре-ролл обязан прочитать тело");

    // а дальше, НЕПРЕРЫВНО по хвосту, читать уже нечего
    let mut tail = Vec::new();
    for _ in 0..20 {
        r.render_frames_with(&mut f, &mut store, pos, &mut buf);
        tail.extend_from_slice(&buf);
        pos += BURST;
    }
    let reads = f.reads - after_preroll;
    assert_eq!(reads, 0, "в хвосте читать нечего, а источник трогали {reads} раз");
    assert!(rms(&tail) > 1e-6, "хвост обязан звучать: реверб дозванивает, rms={}", rms(&tail));
}

/// Тест 295 (пп.303-304): РЕГИОН ВНУТРИ ХВОСТОВ на повторе.
///
/// Ровно то место, о котором писал пользователь: «если выделить регион
/// внутри начала события 21 — растут underruns». Такой регион не содержит
/// НИ ОДНОГО тела, только хвосты; каждый круг — шов для каждой цепочки.
/// Утверждаем, что он играет: детерминированно, без чтения источника и
/// без underrun'ов.
#[test]
fn t295_looping_inside_the_tails_is_deterministic_and_silent_on_reads() {
    let p = base(7);
    let bodies_end = 48 * BODY_MS * 7;
    let region = bodies_end + BURST..bodies_end + BURST + 48_000 / 4;
    let lap = |store: &mut FxStateStore, r: &AudioRenderer, f: &mut Counting| -> (Vec<f32>, u64) {
        let mut out = Vec::new();
        let mut buf = vec![0f32; BURST as usize * CH];
        let mut done = 0i64;
        let mut pos = region.start;
        let stats_before = f.reads;
        while done < region.end - region.start {
            r.render_looped_with(f, store, region.clone(), pos, &mut buf);
            out.extend_from_slice(&buf);
            pos = region.start + (pos - region.start + BURST).rem_euclid(region.end - region.start);
            done += BURST;
        }
        (out, f.reads - stats_before)
    };
    let r = AudioRenderer::new(&p);
    let mut store = FxStateStore::new();
    r.sync_fx(&mut store);
    let mut f = Counting::new();
    let (_, _) = lap(&mut store, &r, &mut f); // круг прогрева
    let (a, reads_a) = lap(&mut store, &r, &mut f);
    let (b, reads_b) = lap(&mut store, &r, &mut f);
    assert_eq!(reads_a, 0, "круг по хвостам не читает источник, а прочитал {reads_a}");
    assert_eq!(reads_b, 0, "и следующий тоже");
    assert_eq!(a.len(), b.len());
    assert!(rms(&a) > 1e-7, "регион в хвостах обязан звучать, rms={}", rms(&a));
    // реверб затухает, поэтому круги НЕ обязаны совпадать байт-в-байт;
    // обязана совпадать детерминированность самого прогона
    let (c, _) = {
        let r2 = AudioRenderer::new(&p);
        let mut s2 = FxStateStore::new();
        r2.sync_fx(&mut s2);
        let mut f2 = Counting::new();
        let _ = lap(&mut s2, &r2, &mut f2);
        let _ = lap(&mut s2, &r2, &mut f2);
        lap(&mut s2, &r2, &mut f2)
    };
    assert_eq!(b, c, "тот же прогон обязан дать те же байты");
}
