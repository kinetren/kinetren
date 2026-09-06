//! Тесты 237-238 (docs/porting-notes.md): резка цели морфер-слоем в РЕНДЕРЕ.
//!
//! Морфер-слой режет цель по границам своего окна (Р.35). Две вещи,
//! которые при этом обязаны остаться нетронутыми:
//!
//! * **переход не исчезает** — он адресован `ClipId`, а у разрезанного
//!   клипа этот id носит только кусок №0; конец перехода перевешивается на
//!   тот кусок, чья граница совпала со стыком (`same_clip` + сверка
//!   границы в `VideoRenderer::new`/`AudioRenderer::new`);
//! * **резка не слышна и не видна** — патч, который этому виду клипа
//!   ничего не делает, обязан давать БАЙТ-В-БАЙТ прежний результат.

use kt_engine::{AudioRenderer, DirectFetcher, VideoRenderer};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_CHANNELS, Tick};

const CH: usize = AUDIO_CHANNELS;
const LEN: Tick = Tick(90_000);

fn direct() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(
        MediaId(1),
        Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Sine { hz: 440.0 })),
    );
    f.add_source(MediaId(2), Box::new(SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Ramp)));
    f
}

fn clip(id: u64, kind: ClipKind, start: Tick, dur: Tick) -> ClipEvent {
    ClipEvent {
        id: ClipId(id),
        link_group: None,
        kind,
        lane: 0,
        start,
        duration: dur,
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: Vec::new(),
        fx_bypass: Vec::new(),
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    }
}

fn base(kind: ClipKind) -> Project {
    let mut p =
        Project::new(ProjectSettings { fps: 30, width: 64, height: 36, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "synth:sine:440".into() });
    p.media.push(MediaRef { id: MediaId(2), path: "synth:ramp".into() });
    p.clips.push(clip(10, kind, Tick(0), LEN));
    p.clips.push(clip(11, kind, LEN, LEN));
    p.morph_lanes = 1;
    p
}

/// Слой на середину ПЕРВОГО клипа. `patch` задаётся вызывающим.
fn layer(patch: MorphPatch) -> MorpherEvent {
    MorpherEvent {
        id: MorphId(1),
        lane: 0,
        start: Tick(30_000),
        duration: Tick(30_000),
        targets: MorphTargets::Clips { ids: vec![ClipId(10)] },
        patch,
        timed: true,
        enabled: true,
    }
}

fn crossfade(p: &mut Project) {
    p.transitions.push(TransitionEvent {
        id: TransitionId(1),
        a: Some(ClipId(10)),
        b: Some(ClipId(11)),
        in_offset: Tick(9_000),
        out_offset: Tick(9_000),
        kind: Default::default(),
        audio: Default::default(),
        auto: vec![],
    });
}

fn render_audio(p: &Project, len: Tick) -> Vec<f32> {
    let r = AudioRenderer::new(p);
    let frames = len.to_audio_frames() as usize;
    let mut out = vec![0f32; frames * CH];
    let s = r.render_frames(&mut direct(), 0, &mut out);
    assert_eq!(s.underruns, 0, "underrun при полностью доступном источнике");
    out
}

/// Кадр превью через тот же путь, что у t137: греем кэш ровно теми
/// слоями, которые назвал сам план, и композитим CPU-эталоном.
fn render_frame(p: &Project, t: Tick) -> Vec<u8> {
    use kt_media::MediaSource;
    let mut vr = VideoRenderer::new(p);
    let cache = kt_cache::BurstCache::new(kt_cache::CacheConfig::default());
    let q = kt_media::Quality { width: 64, height: 36 };
    let mut synth = SyntheticSource::new(Tick(90_000 * 60), AudioPattern::Silence);
    for (media, src_pts) in vr.layers_at(t) {
        let frame = synth.read_video_frame(src_pts, Some(q)).unwrap();
        cache.insert_video(
            kt_engine::source_id(media),
            Some(q),
            kt_engine::video_cache_idx(src_pts),
            std::sync::Arc::new(frame),
        );
    }
    let (out, complete) = vr.compose_from_cache(&cache, t, q);
    assert!(complete, "кадр {t:?}: все слои прогреты");
    out
}

/// Тест 237: переход на РАЗРЕЗАННОМ клипе жив. Слой накрывает ХВОСТ
/// первого клипа вместе с окном кроссфейда и подменяет там точку входа —
/// значит стык достаётся куску, у которого id уже синтетический. Эталон
/// — тот же монтаж, разрезанный РУКАМИ (Split + переход на правой
/// половине). На старом коде (`c.id == id`) конец перехода доставался
/// куску №0, тот вылезал ручкой на всё окно, и кадр расходился.
#[test]
fn t237_transition_survives_slicing() {
    let mut sliced = base(ClipKind::Video);
    crossfade(&mut sliced);
    let mut m = layer(MorphPatch { absolute_source_in: Some(Tick(15_000)), ..Default::default() });
    m.start = Tick(60_000);
    m.duration = Tick(30_000); // до конца клипа: окно перехода накрыто целиком
    sliced.morphers.push(m);
    sliced.validate().unwrap();
    let eff = sliced.effective_clips();
    assert_eq!(eff.iter().filter(|c| same_clip(c.id, ClipId(10))).count(), 2, "клип разрезан");
    assert_ne!(eff[0].id, eff[1].id);

    // эталон: то же самое, сделанное командами модели
    let mut manual = base(ClipKind::Video);
    crossfade(&mut manual);
    let mut mm = TimelineModel::new(manual);
    mm.apply(Command::Split {
        id: ClipId(10),
        at: Tick(60_000),
        new_id: ClipId(12),
        new_group: None,
    })
    .unwrap();
    let mut manual = mm.project().clone();
    manual.clips.iter_mut().find(|c| c.id == ClipId(12)).unwrap().source_in = Tick(15_000);
    manual.validate().unwrap();
    assert_eq!(
        manual.transitions[0].a,
        Some(ClipId(12)),
        "Split перевесил переход на правую половину"
    );

    for t in [Tick(81_000), Tick(84_000), Tick(90_000), Tick(96_000)] {
        assert_eq!(render_frame(&sliced, t), render_frame(&manual, t), "кадр {t:?}");
    }
    // страховка: без перехода кадр в середине ДРУГОЙ — сравнение выше
    // действительно проверяет переход, а не совпадение источников
    let mut no_trans = manual.clone();
    no_trans.transitions.clear();
    assert_ne!(
        render_frame(&no_trans, Tick(90_000)),
        render_frame(&manual, Tick(90_000)),
        "переход виден в кадре"
    );
}

/// Тест 237b: то же в аудио — окно кроссфейда считается в кадрах, и стык
/// ищется той же сверкой.
#[test]
fn t237_audio_transition_survives_slicing() {
    let mut plain = base(ClipKind::Audio);
    crossfade(&mut plain);
    plain.validate().unwrap();
    let mut sliced = plain.clone();
    sliced.morphers.push(layer(MorphPatch { media: Some(MediaId(1)), ..Default::default() }));
    sliced.validate().unwrap();

    let a = render_audio(&plain, Tick(180_000));
    let b = render_audio(&sliced, Tick(180_000));
    assert_eq!(a, b, "кроссфейд на разрезанном клипе звучит байт-в-байт так же");

    let mut no_trans = plain.clone();
    no_trans.transitions.clear();
    assert_ne!(render_audio(&no_trans, Tick(180_000)), a, "кроссфейд слышен");
}

/// Тест 238: резка не слышна. Слой с одними ВИДЕО-эффектами режет видео,
/// но аудио под собой не трогает вовсе; слой, который аудио-клип всё же
/// режет, даёт тот же звук, если сам патч звук не меняет.
#[test]
fn t238_slicing_is_inaudible() {
    // видео-патч не режет аудио вовсе
    let mut p = base(ClipKind::Audio);
    let mut m = layer(MorphPatch {
        video_fx: vec![FxInstance::Mirror { horizontal: true, vertical: false }],
        ..Default::default()
    });
    m.targets = MorphTargets::Clips { ids: vec![ClipId(10), ClipId(11)] };
    p.morphers.push(m);
    p.validate().unwrap();
    assert_eq!(p.effective_clips(), p.clips, "видео-слой аудио под собой не трогает");

    // патч, который режет: подмена источника на ТОТ ЖЕ — звук прежний
    let plain = base(ClipKind::Audio);
    let mut sliced = plain.clone();
    sliced.morphers.push(layer(MorphPatch { media: Some(MediaId(1)), ..Default::default() }));
    sliced.validate().unwrap();
    assert_eq!(sliced.effective_clips().len(), 4, "клип разрезан");
    assert_eq!(
        render_audio(&sliced, Tick(180_000)),
        render_audio(&plain, Tick(180_000)),
        "шов резки не слышен — байт-в-байт"
    );
}

/// Тест 238b: клип со СВОИМ варпом времени (slowed&reverb) не режется
/// вовсе — линейно посчитанная точка входа куска указала бы не туда
/// (находка 2 Р.35). Патч действует на него целиком.
#[test]
fn t238_warped_clip_is_never_sliced() {
    let mut p = base(ClipKind::Audio);
    p.clips[0].fx = vec![FxInstance::slowed_reverb(0.8, 0.3, 60.0)];
    p.morphers.push(layer(MorphPatch { media: Some(MediaId(2)), ..Default::default() }));
    p.validate().unwrap();
    let eff = p.effective_clips();
    assert_eq!(eff.iter().filter(|c| same_clip(c.id, ClipId(10))).count(), 1, "варп не режется");
    assert_eq!(eff[0].media, MediaId(2), "патч действует на весь клип");

    // варп, дописанный САМИМ слоем, — тот же случай
    let mut q = base(ClipKind::Audio);
    q.morphers.push(layer(MorphPatch {
        audio_fx: vec![FxInstance::slowed_reverb(0.8, 0.3, 60.0)],
        ..Default::default()
    }));
    q.validate().unwrap();
    assert_eq!(
        q.effective_clips().iter().filter(|c| same_clip(c.id, ClipId(10))).count(),
        1,
        "слой, добавляющий варп, тоже не режет"
    );
}
