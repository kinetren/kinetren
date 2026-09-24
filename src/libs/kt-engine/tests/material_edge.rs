//! Тест 748 (Р.63 п.2): КОНЕЦ МАТЕРИАЛА — это не голодание.
//!
//! У владельца событие Rolling Stones длилось 316.967 с при файле 226.268 с:
//! при rate 0.75 оно потребляет 237.7 с материала, и последние 11.5 с
//! читают за концом файла. Звучала там тишина — правильно, — но каждый
//! такой кадр считался underrun'ом (180 480 кадров на пять секунд), и
//! регион-луп на хвосте выглядел как «луп ломает звук».
//!
//! Утверждается СЛЕДСТВИЕ: за краем материала тишина, underrun'ов ноль, а
//! внутри материала промах кэша по-прежнему считается — иначе тест
//! «починил» бы счётчик, а не движок.

use kt_engine::{AudioFetcher, AudioRenderer, DirectFetcher};
use kt_media::synthetic::{AudioPattern, SyntheticSource};
use kt_model::*;
use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};

/// Материал — 2 секунды, событие — 4: половина за краем.
const MEDIA_SECS: i64 = 2;
const CLIP_SECS: i64 = 4;

fn project_kind(rate: Option<f64>, kind: ClipKind) -> Project {
    let mut p = Project::new(ProjectSettings {
        fps: 30,
        width: 640,
        height: 360,
        sample_rate: 48000,
        pitch_engine: Default::default(),
        default_shape: Default::default(),
        conform: Default::default(),
        settings_explicit: false,
    });
    p.media.push(MediaRef::with_duration(
        MediaId(1),
        "synth:1",
        Tick(MEDIA_SECS * 90_000),
    ));
    let mut c = ClipEvent {
        r#gen: None,
        gen_auto: Vec::new(),
        overrides: Vec::new(),
        geometry: Default::default(),
        geometry_pos: 0,
        geometry_auto: Vec::new(),
        id: ClipId(1),
        link_group: None,
        kind,
        lane: 0,
        start: Tick(0),
        duration: Tick(CLIP_SECS * 90_000),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx: vec![],
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    };
    if let Some(r) = rate {
        c.fx = vec![FxInstance::slowed_reverb(r, 0.0, 120.0)];
        c.fx_bypass = vec![false];
    }
    p.clips.push(c);
    p.normalize();
    p.validate().unwrap();
    p
}

fn project(rate: Option<f64>) -> Project {
    project_kind(rate, ClipKind::Audio)
}

/// Источник с НАСТОЯЩИМ концом, как у файла.
fn fetcher_with_eof() -> DirectFetcher {
    let mut f = DirectFetcher::new(None);
    f.add_source(
        MediaId(1),
        Box::new(
            SyntheticSource::new(Tick(MEDIA_SECS * 90_000), AudioPattern::Ramp).with_eof(),
        ),
    );
    f
}

fn render(p: &Project, f: &mut dyn AudioFetcher, from_s: f64, len_s: f64) -> (Vec<f32>, u64, u64) {
    let r = AudioRenderer::new(p);
    let frames = (len_s * 48_000.0) as usize;
    let mut out = vec![0f32; frames * AUDIO_CHANNELS];
    let st = r.render_frames(f, (from_s * 48_000.0) as i64, &mut out);
    (out, st.underruns, st.warmup_misses)
}

#[test]
fn t748_material_end_is_silence_not_starvation() {
    // ---- 1. прямое событие: вторая половина за материалом ----------------
    let p = project(None);
    let mut f = fetcher_with_eof();
    let (tail, under, warm) = render(&p, &mut f, 2.5, 1.0);
    assert!(
        tail.iter().all(|s| *s == 0.0),
        "за концом материала обязана быть тишина"
    );
    assert_eq!(
        (under, warm),
        (0, 0),
        "тишина за концом материала посчитана голоданием (на прежнем коде — 48000 кадров)"
    );

    // внутри материала звук есть и голодания тоже нет
    let (body, under, _) = render(&p, &mut f, 0.5, 0.5);
    assert!(body.iter().any(|s| *s != 0.0), "внутри материала обязан быть звук");
    assert_eq!(under, 0, "внутри материала промахов быть не должно");

    // ---- 2. ЗАМЕДЛЕНИЕ: край уезжает вправо по таймлайну -----------------
    // rate 0.5 — 2 с материала растянуты на 4 с: край материала ровно в
    // конце события, и тишины быть не должно вовсе
    let p = project(Some(0.5));
    let mut f = fetcher_with_eof();
    let (all, under, warm) = render(&p, &mut f, 0.0, 3.5);
    assert!(all.iter().any(|s| *s != 0.0), "замедленное событие обязано звучать");
    assert_eq!((under, warm), (0, 0), "у замедленного события голодания нет");

    // ---- 2б. КАК У ВЛАДЕЛЬЦА: замедление 0.75 и хвост за материалом ------
    // 2 с материала при rate 0.75 съедаются за 2.67 с, а событие длится 4 с:
    // последние 1.33 с читают за концом файла ЧЕРЕЗ ЦЕПОЧКУ FX (варп), а не
    // прямым путём — это ровно случай Rolling Stones.
    let p = project(Some(0.75));
    let mut f = fetcher_with_eof();
    let (tail, under, warm) = render(&p, &mut f, 3.0, 0.9);
    assert!(
        tail.iter().all(|s| s.abs() < 1e-6),
        "за концом материала на варп-пути обязана быть тишина"
    );
    assert_eq!(
        (under, warm),
        (0, 0),
        "хвост варп-события за материалом посчитан голоданием (случай владельца)"
    );

    // ---- 3. голодание ВНУТРИ материала по-прежнему считается -------------
    // пустой кэш-фетчер: бёрстов нет, но материал есть — это настоящий
    // underrun, и он обязан остаться видимым
    struct Empty {
        end: Option<i64>,
    }
    impl AudioFetcher for Empty {
        fn audio_burst(
            &mut self,
            _m: MediaId,
            _i: i64,
        ) -> Option<std::sync::Arc<kt_media::AudioBurst>> {
            None
        }
        fn material_end_frames(&mut self, _m: MediaId) -> Option<i64> {
            self.end
        }
    }
    let p = project(None);
    let (_, under, _) = render(&p, &mut Empty { end: Some(MEDIA_SECS * 48_000) }, 0.0, 1.0);
    assert!(
        under >= 48_000 - AUDIO_BURST_FRAMES as u64,
        "промах внутри материала обязан считаться underrun'ом, получено {under}"
    );
    // а за краем — ноль, тем же фетчером
    let (_, under_tail, _) = render(&p, &mut Empty { end: Some(MEDIA_SECS * 48_000) }, 2.5, 1.0);
    assert_eq!(under_tail, 0, "за краем материала промахов не бывает");

    // ---- 3б. ОБЁРТКА ФЕТЧЕРА обязана пропускать вопрос о крае ------------
    // Ловушка, стоившая половины задачи: `NestFetcher` оборачивает фетчер
    // на КАЖДОМ проекте (он же ведёт `begin_clip`), а умолчание трейта —
    // «края нет». Не переопредели его обёртка, механизм молчит целиком:
    // на проекте владельца хвост считался голоданием ровно поэтому.
    {
        struct Wrap<'a>(&'a mut dyn AudioFetcher);
        impl AudioFetcher for Wrap<'_> {
            fn audio_burst(
                &mut self,
                m: MediaId,
                i: i64,
            ) -> Option<std::sync::Arc<kt_media::AudioBurst>> {
                self.0.audio_burst(m, i)
            }
            // намеренно НЕ переопределяем material_end_frames
        }
        let p = project(None);
        let mut inner = fetcher_with_eof();
        let (_, under, _) = render(&p, &mut Wrap(&mut inner), 2.5, 1.0);
        assert!(
            under > 0,
            "сцена вырождена: обёртка без делегирования обязана терять край"
        );
        // а настоящая обёртка движка — делегирует
        let mut direct = fetcher_with_eof();
        assert_eq!(
            direct.material_end_frames(MediaId(1)),
            Some(MEDIA_SECS * 48_000),
            "DirectFetcher обязан знать конец материала"
        );
    }

    // ---- 4. без известной длины — поведение прежнее -----------------------
    let (_, under_unknown, _) = render(&p, &mut Empty { end: None }, 2.5, 1.0);
    assert!(
        under_unknown > 0,
        "без известной длины край не выдумывается: считаем как раньше"
    );
}

/// Та же политика у ВИДЕО: за концом материала держится последний кадр, и
/// заказ префетча не уходит за край (инвариант 4 — заказ равен чтению).
#[test]
fn t748e_video_holds_the_last_frame_past_the_material_end() {
    use kt_engine::{SourceFacts, VideoRenderer};
    use kt_media::Quality;

    let p = project_kind(None, ClipKind::Video);
    let mut facts = SourceFacts::default();
    {
        use kt_media::MediaSource;
        let src =
            SyntheticSource::new(Tick(MEDIA_SECS * 90_000), AudioPattern::Ramp).with_eof();
        facts.observe(MediaId(1), src.info());
    }
    let vr = VideoRenderer::new_with_facts(&p, &facts);
    let q = Quality { width: 640, height: 360 };
    let last_inside = Tick(MEDIA_SECS * 90_000 - 3_000);
    let inside: Vec<Tick> =
        vr.layers_at(last_inside, q).into_iter().map(|(_, pts, _)| pts).collect();
    let beyond: Vec<Tick> =
        vr.layers_at(Tick(3 * 90_000), q).into_iter().map(|(_, pts, _)| pts).collect();
    assert_eq!(inside.len(), 1, "сцена: один слой внутри материала");
    assert_eq!(beyond.len(), 1, "за краем слой обязан остаться (последний кадр)");
    assert!(
        beyond[0].0 < MEDIA_SECS * 90_000,
        "за концом материала просим НЕсуществующий кадр {:?}: заказ ушёл за край",
        beyond[0]
    );
    assert_eq!(beyond[0], inside[0], "держится последний кадр материала");

    // и заказ префетча на хвосте не выходит за материал
    let wanted =
        vr.wanted_frames(kt_time::TickRange::new(Tick(0), Tick(CLIP_SECS * 90_000)), q);
    assert!(
        wanted.iter().all(|(_, pts, _)| pts.0 < MEDIA_SECS * 90_000),
        "заказ просит кадры за концом материала"
    );
}
