//! Источники медиа: трейт `MediaSource`, синтетические источники для тестов,
//! rsmpeg/FFmpeg-бэкенд (feature `ffmpeg`, M3) и sidecar индекс-БД (M3,
//! см. docs/media-index.md).

use kt_time::{AUDIO_BURST_FRAMES, AUDIO_CHANNELS, Tick};

#[derive(Debug, thiserror::Error)]
pub enum MediaError {
    #[error("out of range")]
    OutOfRange,
    #[error("decode failed: {0}")]
    Decode(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct MediaInfo {
    pub duration: Tick,
    pub has_video: bool,
    pub has_audio: bool,
    /// Частота кадров источника (числитель/знаменатель), если есть видео.
    pub fps: Option<(u32, u32)>,
    /// Исходный размер кадра (0×0, если видео нет) — для «Max (по
    /// источникам)» в выборе разрешения (п.11, порт v1).
    pub width: u32,
    pub height: u32,
    /// Пиксельное соотношение сторон (SAR, num/den; Р.62). Вид кадра =
    /// `width × sar`: всё приведение, тумбы и «Max (по источникам)»
    /// считаются от ВИДА, иначе анаморф (DV 4:3, SAR 16:15) — ещё один
    /// молчаливый растяг. Умолчание 1:1.
    pub sar: (u32, u32),
    /// ОДИНОЧНАЯ КАРТИНКА (Р.67): один кадр на любой тик, своей сетки
    /// кадров и конца материала нет. Событие любой длительности показывает
    /// её целиком, а «Loading…» над ним не висит.
    pub still: bool,
}

impl MediaInfo {
    /// Вид кадра в квадратных пикселях: (ширина × SAR, высота).
    pub fn view_size(&self) -> (f64, f64) {
        let (n, d) = self.sar;
        let k = if n > 0 && d > 0 { n as f64 / d as f64 } else { 1.0 };
        (self.width as f64 * k, self.height as f64)
    }
}

/// Запрошенное качество видео (размер, в который декодер отдаёт кадр).
/// `None` = полное разрешение источника. Входит в ключ кэша.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Quality {
    pub width: u32,
    pub height: u32,
}

/// Аудио-burst: ровно `AUDIO_BURST_FRAMES` кадров, 48 kHz stereo f32 interleaved.
/// Инвариант движка №1: после декода другого формата аудио не существует.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioBurst {
    /// Номер burst'а (покрывает сэмплы [idx*1024, (idx+1)*1024)).
    pub idx: i64,
    pub data: Box<[f32; AUDIO_BURST_FRAMES * AUDIO_CHANNELS]>,
}

impl AudioBurst {
    pub fn silence(idx: i64) -> AudioBurst {
        AudioBurst { idx, data: Box::new([0.0; AUDIO_BURST_FRAMES * AUDIO_CHANNELS]) }
    }
}

/// Видеокадр: RGBA8, premultiplied alpha не требуется на этом уровне.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoFrame {
    pub pts: Tick,
    pub width: u32,
    pub height: u32,
    pub rgba: Box<[u8]>,
    pub has_alpha: bool,
}

/// Источник медиа. Реализации: `FfmpegSource` (feature `ffmpeg`, M3),
/// синтетические источники (тесты). Экземпляром владеет ровно один
/// MediaWorker-поток (инвариант №5) — поэтому `&mut self` и только `Send`.
pub trait MediaSource: Send {
    fn info(&self) -> &MediaInfo;
    /// Sample-accurate: burst `idx` покрывает сэмплы источника [idx*1024, (idx+1)*1024).
    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError>;
    fn read_video_frame(
        &mut self,
        pts: Tick,
        quality: Option<Quality>,
    ) -> Result<VideoFrame, MediaError>;
    fn keyframe_before(&self, pts: Tick) -> Option<Tick>;

    /// Принять НОВЫЕ параметры БЕЗ пересоздания источника (Р.53, п.410).
    ///
    /// Контракт жёсткий: `true` — параметры применены И `info()` НЕ
    /// изменился; `false` — источник этого не умеет, и вызывающий обязан
    /// честно пересоздать его (`Player::replace_media`). Промежуточного
    /// состояния не бывает: `false` не меняет ничего.
    ///
    /// Зачем: у генератора параметры лежат в пути медиа, и до этого
    /// пункта их правка выглядела для движка ПОДМЕНОЙ ФАЙЛА — гасился
    /// поток декодера, чистился кэш, дважды пересобирались рендереры. У
    /// файловых источников метод так и остаётся `false`: смена пути там
    /// и есть смена файла.
    fn retune(&mut self, _path: &str) -> bool {
        false
    }

    /// Принять НОВЫЕ ЗНАЧЕНИЯ ГЕНЕРАТОРА без пересоздания (Р.59, стадия 2).
    ///
    /// Близнец `retune` с тем же жёстким контрактом (`true` ⇒ применено И
    /// `info()` не изменился), только вход — состояние события, а не
    /// строка. Отдельный метод, а не «`retune` с сериализацией в путь»:
    /// кривые в путь не укладываются, а сериализовать значения в строку
    /// ради того, чтобы тут же их разобрать, — это второй парсер на том
    /// же горячем пути.
    fn retune_gen(&mut self, _state: &kt_model::GenState) -> bool {
        false
    }

    /// Спан: кадр для КАЖДОГО ключа (ключи выровнены по сетке, строго
    /// возрастают). Семантика кадра для ключа k — как у read_video_frame
    /// (наибольший pts <= k). Планировщик кладёт ВСЕ кадры в кэш — GOP
    /// декодируется один раз на спан (п.14/15). Дефолт — цикл одиночных.
    fn read_video_span(
        &mut self,
        keys: &[Tick],
        quality: Option<Quality>,
        emit: &mut dyn FnMut(Tick, VideoFrame),
    ) -> Result<(), MediaError> {
        for &k in keys {
            match self.read_video_frame(k, quality) {
                Ok(f) => emit(k, f),
                Err(MediaError::OutOfRange) => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

/// Кадров в блоке уровня 0 peak-пирамиды (docs/media-index.md).
pub const PEAK_BLOCK_FRAMES: usize = 256;

pub mod factory;
pub mod ffmpeg;
pub mod generator;
pub mod index;
#[cfg(feature = "ffmpeg")]
pub mod muxer;
pub(crate) mod pixfont;
pub mod placeholder;
#[cfg(feature = "ffmpeg")]
pub mod sync;
pub mod synthetic;
