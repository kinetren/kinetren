//! Bridge to the in-process MP4 muxer (kt-ffmpeg).
//! Codecs come from the system FFmpeg, so its hardware encoders are available.

pub use kt_ffmpeg::{MuxConfig, Muxer};

/// Video encoders available in the system (for selection in the UI).
pub fn available_encoders() -> Vec<String> {
    kt_ffmpeg::available_encoders()
}
