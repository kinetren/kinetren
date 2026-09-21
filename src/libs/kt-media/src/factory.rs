//! ЕДИНСТВЕННЫЙ дом разбора псевдо-путей и открытия источников (Р.52).
//!
//! До этого разбор `synth:` был написан ДВАЖДЫ (kt-app и kt-cli) и уже
//! расходился: kt-app понимал `synth:Ramp`, kt-cli — только `synth:ramp`,
//! и настоящий источник молча считался отсутствующим файлом (сноска
//! п.211 в docs/porting-notes.md). Третий вид псевдо-пути (`gen:`) без
//! единой фабрики воспроизвёл бы дефект тише: вместо цветного кадра —
//! заглушка. Поэтому:
//! - разбор регистронезависимый и живёт только здесь;
//! - битый псевдо-путь — честная ошибка, а НЕ попытка открыть его
//!   FFmpeg'ом («no such file» прятал бы настоящую причину).

use crate::MediaSource;
use crate::generator::GeneratorSource;
use crate::synthetic::{AudioPattern, SyntheticSource};
use kt_time::Tick;

/// Длительность synth-источников: час, как было в kt-app с первого дня.
const SYNTH_DURATION: Tick = Tick(90_000 * 3600);

/// Образец `synth:<pattern>[:<arg>]`. Имя образца регистронезависимо
/// (живые сценарии пишут «synth:Ramp», тесты — «synth:ramp»).
pub fn synth_pattern(path: &str) -> Option<AudioPattern> {
    let rest = strip_ci(path, "synth:")?;
    let mut it = rest.split(':');
    match it.next()?.to_ascii_lowercase().as_str() {
        "silence" => Some(AudioPattern::Silence),
        "ramp" => Some(AudioPattern::Ramp),
        "impulse" => Some(AudioPattern::Impulse),
        "sine" => Some(AudioPattern::Sine { hz: it.next()?.parse().ok()? }),
        "dc" => Some(AudioPattern::Dc { value: it.next()?.parse().ok()? }),
        _ => None,
    }
}

/// Источник по псевдо-пути. `None` — путь не псевдо (обычный файл,
/// открывается FFmpeg'ом). `Some(Err)` — путь ПСЕВДО, но битый: выше это
/// ошибка «источник недоступен» (плейсхолдер там, где он уместен), в
/// FFmpeg такой путь не ходит.
pub fn open_pseudo(path: &str) -> Option<Result<Box<dyn MediaSource>, String>> {
    if starts_ci(path, "synth:") {
        return Some(match synth_pattern(path) {
            Some(p) => Ok(Box::new(SyntheticSource::new(SYNTH_DURATION, p)) as _),
            None => Err(format!("unknown synth pattern: {path}")),
        });
    }
    if starts_ci(path, "gen:") {
        return Some(match kt_model::parse_gen_path(path) {
            Some(k) => Ok(Box::new(GeneratorSource::new(k)) as _),
            None => Err(format!("unknown generator: {path}")),
        });
    }
    None
}

/// Можно ли перевести ЖИВОЙ источник со старого пути на новый БЕЗ
/// пересоздания (Р.53, п.410). Предикат живёт здесь, в единственном доме
/// разбора псевдо-путей, и обязан совпадать с `MediaSource::retune`
/// точь-в-точь (сторож — t499): разойдутся — вызывающий либо зря
/// пересоздаёт поток, либо считает перенастроенным то, что ею не было.
///
/// Файлы не перенастраиваются никогда: смена пути там и есть смена файла.
pub fn can_retune(old: &str, new: &str) -> bool {
    let (Some(a), Some(b)) = (kt_model::parse_gen_path(old), kt_model::parse_gen_path(new)) else {
        return false;
    };
    // паспорт обязан остаться прежним: у SMPTE включение тона делает
    // медиа звучащим, а это уже другой набор клипов на таймлайне
    GeneratorSource::info_for(&a) == GeneratorSource::info_for(&b)
}

/// Источник генератора по СОСТОЯНИЮ СОБЫТИЯ (Р.59, стадия 2).
///
/// Отдельная дверь рядом с `open_pseudo`: у генератора на событии пути
/// со значениями больше нет — значения приходят из модели, а `gen:`-путь
/// медиа несёт только вид. Все, кто поднимает источники (плеер, экспорт,
/// kt-cli), зовут именно её, поэтому «что нарисует генератор» считается
/// в одном месте.
pub fn open_gen(state: &kt_model::GenState) -> Box<dyn MediaSource> {
    Box::new(GeneratorSource::from_state(state.clone()))
}

/// Можно ли перевести ЖИВОЙ источник на новые ЗНАЧЕНИЯ без пересоздания.
/// Близнец `can_retune` и обязан совпадать с `MediaSource::retune_gen`
/// точь-в-точь (сторож — t660): расходись они, вызывающий либо зря гасит
/// поток, либо считает перенастроенным то, что ею не было.
pub fn can_retune_gen(old: &kt_model::GenState, new: &kt_model::GenState) -> bool {
    // паспорт обязан остаться прежним: у SMPTE включение тона делает
    // медиа звучащим, а это уже другой набор клипов на таймлайне
    GeneratorSource::info_for(&old.kind) == GeneratorSource::info_for(&new.kind)
}

/// Открытие любого пути: псевдо — из фабрики, файл — FFmpeg с sidecar
/// индекс-БД (пакетный индекс из БД при неизменном файле, тест 34).
#[cfg(feature = "ffmpeg")]
pub fn open_indexed(
    path: &str,
    db: Option<&mut crate::index::MediaIndexDb>,
) -> Result<Box<dyn MediaSource>, String> {
    if let Some(r) = open_pseudo(path) {
        return r;
    }
    crate::ffmpeg::FfmpegSource::open_indexed(path, db)
        .map(|s| Box::new(s) as _)
        .map_err(|e| e.to_string())
}

fn starts_ci(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

fn strip_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    starts_ci(s, prefix).then(|| &s[prefix.len()..])
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t499 (Р.53, п.410): предикат фабрики и реализация источника не
    /// расходятся НИ НА ОДНОЙ паре видов. Расхождение означало бы либо
    /// зря погашенный поток, либо «перенастроили» то, что не менялось.
    #[test]
    fn t499_can_retune_equals_retune() {
        use kt_model::{GeneratorKind, format_gen_path};
        let mut kinds: Vec<GeneratorKind> = GeneratorKind::all();
        // вариации, меняющие паспорт и не меняющие его
        kinds.push(GeneratorKind::Solid { color_r: 1, color_g: 2, color_b: 3 });
        kinds.push(GeneratorKind::SmpteBars { tone: true, tone_db: -12.0 });
        kinds.push(GeneratorKind::SmpteBars { tone: false, tone_db: -6.0 });

        for a in &kinds {
            for b in &kinds {
                let (pa, pb) = (format_gen_path(a), format_gen_path(b));
                let predicate = can_retune(&pa, &pb);
                let actual = GeneratorSource::new(a.clone()).retune(&pb);
                assert_eq!(predicate, actual, "{pa} -> {pb}: предикат разошёлся с реализацией");
            }
        }
        // файлы и тестовая схема не перенастраиваются никогда
        assert!(!can_retune("a.mp4", "b.mp4"));
        assert!(!can_retune("synth:ramp", "synth:sine:440"));
        assert!(!can_retune("gen:solid", "a.mp4"));
    }

    /// t660 (Р.59, стадия 2; близнец t499): предикат фабрики и
    /// реализация источника не расходятся НИ НА ОДНОЙ паре СОСТОЯНИЙ.
    ///
    /// Разойдись они — вызывающий либо зря гасит поток декодера, либо
    /// считает перенастроенным то, что ею не было, и картинка навсегда
    /// остаётся старой.
    #[test]
    fn t660_can_retune_gen_equals_retune_gen() {
        use kt_model::{GenState, GeneratorKind, TextAnim, text_anim_curves};
        let mut states: Vec<GenState> =
            GeneratorKind::all().into_iter().map(GenState::new).collect();
        // вариации, меняющие паспорт и не меняющие его
        states.push(GenState::new(GeneratorKind::Solid { color_r: 1, color_g: 2, color_b: 3 }));
        states.push(GenState::new(GeneratorKind::SmpteBars { tone: true, tone_db: -12.0 }));
        states.push(GenState::new(GeneratorKind::SmpteBars { tone: false, tone_db: -6.0 }));
        states.push(GenState::new(GeneratorKind::Leader {
            seconds: 3,
            fps: 30,
            beep: true,
            beep_db: -20.0,
        }));
        // и состояние С КРИВЫМИ: паспорта они не трогают, значит
        // перенастройка обязана их принимать
        states.push(GenState {
            kind: GeneratorKind::default_text(),
            auto: text_anim_curves(TextAnim::Typewriter, 2.0, 2.0),
        });

        for a in &states {
            for b in &states {
                let predicate = can_retune_gen(a, b);
                let actual = GeneratorSource::from_state(a.clone()).retune_gen(b);
                assert_eq!(
                    predicate,
                    actual,
                    "{} -> {}: предикат разошёлся с реализацией",
                    a.kind.tag(),
                    b.kind.tag()
                );
            }
        }
        // файловые источники значений генератора не принимают никогда
        let mut synth = crate::synthetic::SyntheticSource::new(Tick(90_000), AudioPattern::Ramp);
        assert!(!MediaSource::retune_gen(&mut synth, &states[0]));
    }

    /// t471b: маршрутизация фабрики — обе псевдо-схемы в любом регистре,
    /// битое псевдо = Some(Err) (в FFmpeg не ходит), файл = None.
    #[test]
    fn t471b_factory_routes_pseudo_paths() {
        assert!(open_pseudo("synth:ramp").unwrap().is_ok());
        assert!(open_pseudo("Synth:Ramp").unwrap().is_ok(), "регресс сноски п.211");
        assert!(open_pseudo("synth:sine:440").unwrap().is_ok());
        assert!(open_pseudo("synth:nope").unwrap().is_err());

        let g = open_pseudo("gen:solid:c=ff8800").unwrap().unwrap();
        assert!(g.info().has_video && !g.info().has_audio);
        assert!(open_pseudo("GEN:SOLID").unwrap().is_ok());
        assert!(open_pseudo("gen:unknown").unwrap().is_err(), "битый gen: — ошибка, не ffmpeg");

        assert!(open_pseudo("media/clip.mp4").is_none());
        assert!(open_pseudo("generator.mp4").is_none());
    }
}
