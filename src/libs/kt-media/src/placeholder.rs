//! Placeholder for a missing source (R.33 item 10, user requirement:
//! “when rendering and previewing, missing sources must not block anything: the user
//! must be notified and rendering continues, and for missing video a placeholder must
//! be substituted”).
//!
//! The source behaves like a normal one: it delivers a dark frame with the file's NAME
//! (it is immediately visible what is missing) and silence in the audio. Like all others,
//! it is owned by exactly one MediaWorker thread.
//!
//! Text is drawn with the built-in 3×5 pixel font (`pixfont`, shared with
//! generators): it depends neither on system fonts nor on the egui renderer, so the
//! frame is deterministic and identical in preview, export and tests. Characters outside
//! the table (Cyrillic, hieroglyphs) are shown as “?” — the file name stays recognizable
//! by its Latin letters, digits and extension.

use crate::pixfont::{Canvas, GLYPH_H, GLYPH_W, text_width};
use crate::{AudioBurst, MediaError, MediaInfo, MediaSource, Quality, VideoFrame};
use kt_time::Tick;

/// “Practically infinite” duration: 1000 hours. The placeholder does not
/// know how long the missing file lasted, and it must not trim the edit.
const PLACEHOLDER_DURATION: Tick = Tick(90_000 * 3600 * 1000);
const DEFAULT_W: u32 = 640;
const DEFAULT_H: u32 = 360;

const BG: [u8; 4] = [42, 44, 48, 255];
const INK: [u8; 4] = [188, 192, 200, 255];
const MARK: [u8; 4] = [214, 122, 96, 255];

/// Placeholder instead of an unavailable file: a dark frame with the name and a
/// film icon, silence in the audio.
pub struct PlaceholderSource {
    info: MediaInfo,
    label: String,
    /// The frame is built once per requested quality: it depends neither on time
    /// nor on anything else, and drawing it on every tick would be a waste.
    cache: Option<(Quality, Box<[u8]>)>,
}

impl PlaceholderSource {
    /// `label` — what is missing (usually the file name).
    pub fn new(label: impl Into<String>) -> PlaceholderSource {
        PlaceholderSource {
            info: MediaInfo {
                duration: PLACEHOLDER_DURATION,
                has_video: true,
                has_audio: true,
                // it has no frame grid of its own: the engine plans on the PROJECT's grid
                fps: None,
                width: DEFAULT_W,
                height: DEFAULT_H,
                sar: (1, 1),
                still: true,
            },
            label: label.into(),
            cache: None,
        }
    }

    fn frame_rgba(&mut self, q: Quality) -> &[u8] {
        if self.cache.as_ref().is_none_or(|(cq, _)| *cq != q) {
            self.cache = Some((q, render(&self.label, q.width, q.height)));
        }
        &self.cache.as_ref().expect("frame was just built").1
    }
}

fn render(label: &str, w: u32, h: u32) -> Box<[u8]> {
    let (w, h) = (w.max(1), h.max(1));
    let mut c = Canvas::new(w, h, BG);
    // the frame border — visible that it is a stub, not a black hole
    let b = (w.min(h) as i64 / 90).max(1);
    c.rect(0, 0, c.w, b, MARK);
    c.rect(0, c.h - b, c.w, b, MARK);
    c.rect(0, 0, b, c.h, MARK);
    c.rect(c.w - b, 0, b, c.h, MARK);

    // name: the scale is chosen to fit the frame width, too-long names are cut
    let mut text: Vec<char> = label.chars().collect();
    let s = ((c.w * 4 / 5) / text_width(text.len().max(1), 1).max(1)).clamp(1, 8);
    let max_chars = ((c.w * 4 / 5) / ((GLYPH_W as i64 + 1) * s)).max(1) as usize;
    if text.len() > max_chars {
        text.truncate(max_chars.saturating_sub(2));
        text.extend(['.', '.']);
    }
    let text: String = text.into_iter().collect();
    let tx = (c.w - text_width(text.chars().count(), s)) / 2;
    let ty = (c.h - GLYPH_H as i64 * s) / 2 + c.h / 12;
    c.text(tx, ty, s, &text, INK);
    // an icon above the caption
    c.film((c.w - 14 * s) / 2, ty - 14 * s, s, MARK);
    c.buf.into_boxed_slice()
}

impl MediaSource for PlaceholderSource {
    fn info(&self) -> &MediaInfo {
        &self.info
    }

    fn read_audio_burst(&mut self, idx: i64) -> Result<AudioBurst, MediaError> {
        Ok(AudioBurst::silence(idx))
    }

    fn read_video_frame(
        &mut self,
        pts: Tick,
        quality: Option<Quality>,
    ) -> Result<VideoFrame, MediaError> {
        let q = quality.unwrap_or(Quality { width: DEFAULT_W, height: DEFAULT_H });
        let rgba = self.frame_rgba(q).to_vec().into_boxed_slice();
        Ok(VideoFrame { pts, width: q.width, height: q.height, rgba, has_alpha: false })
    }

    fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        Some(Tick(pts.0.max(0)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(label: &str, q: Quality) -> VideoFrame {
        let mut s = PlaceholderSource::new(label);
        s.read_video_frame(Tick(12_345), Some(q)).unwrap()
    }

    /// Test 208 (R.33 item 10): the placeholder frame is deterministic, really
    /// contains the caption, and does not depend on system fonts.
    #[test]
    fn t208_placeholder_frame_is_deterministic_and_labeled() {
        let q = Quality { width: 320, height: 180 };
        let a = frame("clip-01.mp4", q);
        let b = frame("clip-01.mp4", q);
        assert_eq!(a.rgba, b.rgba, "the frame must be the same on every request");
        assert_eq!((a.width, a.height), (320, 180));

        // a second request for the SAME source is taken from the cache and matches
        let mut src = PlaceholderSource::new("clip-01.mp4");
        let f1 = src.read_video_frame(Tick(0), Some(q)).unwrap();
        let f2 = src.read_video_frame(Tick(90_000), Some(q)).unwrap();
        assert_eq!(f1.rgba, f2.rgba, "the frame must not depend on time");
        assert_eq!(f2.pts.0, 90_000, "pts must be the requested one");

        // the caption is visible: there are noticeably many text-colored pixels and the frame is not uniform
        let ink = a.rgba.chunks_exact(4).filter(|p| p[0] == INK[0] && p[1] == INK[1]).count();
        assert!(ink > 50, "there is no text in the frame: {ink} pixels");
        let other = frame("zzzz-99.mov", q);
        assert_ne!(a.rgba, other.rgba, "different names must give different frames");
    }

    /// Non-ASCII (Cyrillic, hieroglyphs) is drawn as “?” — the frame stays
    /// deterministic and does not depend on installed fonts.
    #[test]
    fn t208_non_ascii_label_falls_back_to_question_marks() {
        let q = Quality { width: 320, height: 180 };
        let cyr = frame("Ролик.mp4", q);
        let subs = frame("?????.mp4", q);
        assert_eq!(cyr.rgba, subs.rgba, "non-ASCII must be replaced by “?”");
    }

    #[test]
    fn t208_audio_is_silence_and_source_is_long() {
        let mut s = PlaceholderSource::new("gone.wav");
        let b = s.read_audio_burst(7).unwrap();
        assert_eq!(b.idx, 7);
        assert!(b.data.iter().all(|v| *v == 0.0), "placeholder must be silent");
        assert!(s.info().duration.0 > 90_000 * 3600, "duration must not cut the edit");
        assert!(s.info().has_video && s.info().has_audio);
    }
}
