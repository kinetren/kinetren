//! Bridge to the in-process MP4 muxer (kt-ffmpeg).
//! Codecs come from the system FFmpeg, so its hardware encoders are available.

pub use kt_ffmpeg::{AvailableEncoder, HwReport, MuxConfig, Muxer, OpenReport, hw_seam};

/// The video encoders this OS tries, in order (the names of `kt_ffmpeg`'s cascade): the order
/// the open report's failed candidates must follow (render-window-v1-parity, t954).
pub fn encoder_cascade() -> Vec<String> {
    kt_ffmpeg::encoders::video_encoder_candidates(kt_ffmpeg::search::Os::NATIVE)
        .iter()
        .map(|s| s.name.to_string())
        .collect()
}

/// Video encoders available in the system (for selection in the UI); a device encoder with the
/// device it will use (hw-encode-context D6).
pub fn available_encoders() -> Vec<AvailableEncoder> {
    kt_ffmpeg::available_encoders()
}

/// The hardware encoding section of the FFmpeg report (`None` — FFmpeg is not loaded).
pub fn hw_report() -> Option<HwReport> {
    kt_ffmpeg::hw_report()
}
