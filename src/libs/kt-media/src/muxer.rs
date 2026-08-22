//! Мост к внутрипроцессному муксеру MP4 (kt-ffmpeg).
//! Кодеки — из системного FFmpeg, поэтому доступны его аппаратные энкодеры.

pub use kt_ffmpeg::{MuxConfig, Muxer};

/// Доступные в системе видео-энкодеры (для выбора в UI).
pub fn available_encoders() -> Vec<String> {
    kt_ffmpeg::available_encoders()
}
