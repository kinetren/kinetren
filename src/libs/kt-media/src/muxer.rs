//! Bridge to the in-process MP4 muxer (kt-ffmpeg).
//! Codecs come from the system FFmpeg, so its hardware encoders are available.

pub use kt_ffmpeg::{MuxConfig, Muxer, OpenReport};

/// The video encoders this OS tries, in order (the names of `kt_ffmpeg`'s cascade): the order
/// the open report's failed candidates must follow (render-window-v1-parity, t954).
pub fn encoder_cascade() -> Vec<String> {
    kt_ffmpeg::encoders::video_encoder_candidates(kt_ffmpeg::search::Os::NATIVE)
        .iter()
        .map(|s| s.name.to_string())
        .collect()
}

/// Video encoders available in the system (for selection in the UI).
pub fn available_encoders() -> Vec<String> {
    kt_ffmpeg::available_encoders()
}
