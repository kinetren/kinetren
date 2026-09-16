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

use crate::generator::GeneratorSource;
use crate::synthetic::{AudioPattern, SyntheticSource};
use crate::MediaSource;
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
