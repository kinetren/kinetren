//! The render settings as text (change render-settings D9, capability export): ordered
//! `key=value` pairs, the one form kt-cli's flags and kt-app's presets share. An unknown key or a
//! bad value is an error naming it; a reader that wants to keep a record it does not understand
//! (the preset store) asks `known_key` first.

use crate::export::ExportOptions;
use kt_media::muxer::format::{AudioCodec, BitDepth, Container, Matrix, Range, RateMode, Speed, VideoCodec};

/// Every key, in the order `pairs` writes them.
pub const KEYS: [&str; 19] = [
    "container", "video", "encoder", "rate", "quality", "bitrate", "maxrate", "keyint", "bframes", "speed", "depth",
    "matrix", "range", "audio", "abitrate", "size", "fps", "title", "comment",
];

/// The bitrate a rate mode starts from when none was given.
pub const DEFAULT_BITRATE: u32 = 8_000_000;

pub fn known_key(k: &str) -> bool {
    KEYS.contains(&k)
}

/// The words of each choice: (value, text).
pub fn container_word(c: Container) -> &'static str {
    c.extension()
}
pub fn video_word(v: VideoCodec) -> &'static str {
    match v {
        VideoCodec::H264 => "h264",
        VideoCodec::H265 => "h265",
        VideoCodec::Av1 => "av1",
        VideoCodec::Vp9 => "vp9",
        VideoCodec::ProRes => "prores",
    }
}
pub fn rate_word(r: RateMode) -> &'static str {
    match r {
        RateMode::Quality => "quality",
        RateMode::Vbr => "vbr",
        RateMode::Cbr => "cbr",
    }
}
pub fn speed_word(s: Speed) -> &'static str {
    match s {
        Speed::Fastest => "fastest",
        Speed::Fast => "fast",
        Speed::Balanced => "balanced",
        Speed::Slow => "slow",
        Speed::Slowest => "slowest",
    }
}
pub fn audio_word(a: AudioCodec) -> &'static str {
    match a {
        AudioCodec::Aac => "aac",
        AudioCodec::Pcm16 => "pcm16",
        AudioCodec::Pcm24 => "pcm24",
        AudioCodec::Flac => "flac",
        AudioCodec::Opus => "opus",
        AudioCodec::None => "none",
    }
}

pub const SPEEDS: [Speed; 5] = [Speed::Fastest, Speed::Fast, Speed::Balanced, Speed::Slow, Speed::Slowest];
pub const RATES: [RateMode; 3] = [RateMode::Quality, RateMode::Vbr, RateMode::Cbr];

/// "8M", "8000k", "8000000", "8.5M" → bit/s.
pub fn parse_bits(v: &str) -> Option<u32> {
    let v = v.trim();
    let (num, mul) = match v.chars().last()? {
        'k' | 'K' => (&v[..v.len() - 1], 1_000.0),
        'm' | 'M' => (&v[..v.len() - 1], 1_000_000.0),
        _ => (v, 1.0),
    };
    let x: f64 = num.parse().ok()?;
    let b = x * mul;
    (b >= 1.0 && b <= u32::MAX as f64).then_some(b as u32)
}

fn bits_text(b: u32) -> String {
    if b.is_multiple_of(1_000_000) {
        format!("{}M", b / 1_000_000)
    } else if b.is_multiple_of(1_000) {
        format!("{}k", b / 1_000)
    } else {
        b.to_string()
    }
}

/// The settings as ordered pairs; `bitrate` and `maxrate` only where a rate mode uses them.
pub fn pairs(o: &ExportOptions) -> Vec<(String, String)> {
    let f = &o.format;
    let mut out: Vec<(&str, String)> = vec![
        ("container", container_word(f.container).into()),
        ("video", video_word(f.video).into()),
        ("encoder", o.force_encoder.clone().unwrap_or_else(|| "auto".into())),
        ("rate", rate_word(effective_rate(o)).into()),
        ("quality", o.quality.to_string()),
    ];
    if let Some(b) = o.video_bitrate {
        out.push(("bitrate", bits_text(b)));
    }
    if let Some(m) = f.max_bitrate {
        out.push(("maxrate", bits_text(m)));
    }
    out.extend([
        ("keyint", format!("{}.{}", f.keyint_tenths / 10, f.keyint_tenths % 10)),
        ("bframes", f.b_frames.to_string()),
        ("speed", speed_word(f.speed).into()),
        ("depth", if f.depth == BitDepth::Ten { "10" } else { "8" }.into()),
        ("matrix", match f.matrix {
            None => "auto",
            Some(Matrix::Bt601) => "bt601",
            Some(Matrix::Bt709) => "bt709",
        }.into()),
        ("range", if f.range == Range::Full { "full" } else { "limited" }.into()),
        ("audio", audio_word(f.audio).into()),
        ("abitrate", bits_text(o.audio_bitrate)),
        ("size", o.resolution.map_or("project".into(), |(w, h)| format!("{w}x{h}"))),
        ("fps", o.fps.map_or("project".into(), |f| f.to_string())),
    ]);
    if !f.title.is_empty() {
        out.push(("title", f.title.clone()));
    }
    if !f.comment.is_empty() {
        out.push(("comment", f.comment.clone()));
    }
    out.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

/// The rate mode the export will use: a bitrate without a mode is the former VBR.
pub fn effective_rate(o: &ExportOptions) -> RateMode {
    match (o.format.rate, o.video_bitrate) {
        (_, None) => RateMode::Quality,
        (RateMode::Quality, Some(_)) => RateMode::Vbr,
        (r, Some(_)) => r,
    }
}

fn pick<T: Copy>(key: &str, v: &str, all: &[T], word: impl Fn(T) -> &'static str) -> Result<T, String> {
    all.iter().copied().find(|&x| word(x) == v.to_ascii_lowercase()).ok_or_else(|| {
        let words: Vec<&str> = all.iter().map(|&x| word(x)).collect();
        format!("{key}: “{v}” is not one of {}", words.join(", "))
    })
}

/// Set one key; the error names the key. `rate` and `bitrate` are reconciled by `finish`.
pub fn apply(o: &mut ExportOptions, key: &str, v: &str) -> Result<(), String> {
    let bad = |what: &str| format!("{key}: “{v}” is not {what}");
    let num = |lo: u32, hi: u32| -> Result<u32, String> {
        v.trim().parse::<u32>().ok().filter(|n| (lo..=hi).contains(n)).ok_or_else(|| bad(&format!("a number {lo}–{hi}")))
    };
    let f = &mut o.format;
    match key {
        "container" => f.container = pick(key, v, &Container::ALL, container_word)?,
        "video" => f.video = pick(key, v, &VideoCodec::ALL, video_word)?,
        "encoder" => o.force_encoder = (v != "auto" && !v.is_empty()).then(|| v.to_string()),
        "rate" => f.rate = pick(key, v, &RATES, rate_word)?,
        "quality" => o.quality = num(0, 63)?,
        "bitrate" => o.video_bitrate = Some(parse_bits(v).ok_or_else(|| bad("a bitrate like 8M"))?),
        "maxrate" => {
            f.max_bitrate = if v == "auto" { None } else { Some(parse_bits(v).ok_or_else(|| bad("a bitrate like 12M"))?) }
        }
        "keyint" => {
            let s: f64 = v.trim().parse().map_err(|_| bad("seconds"))?;
            if !(0.1..=60.0).contains(&s) {
                return Err(bad("0.1–60 seconds"));
            }
            f.keyint_tenths = (s * 10.0 + 0.5) as u32;
        }
        "bframes" => f.b_frames = num(0, 4)?,
        "speed" => f.speed = pick(key, v, &SPEEDS, speed_word)?,
        "depth" => {
            f.depth = match v {
                "8" => BitDepth::Eight,
                "10" => BitDepth::Ten,
                _ => return Err(bad("8 or 10")),
            }
        }
        "matrix" => {
            f.matrix = match v.to_ascii_lowercase().as_str() {
                "auto" => None,
                "bt601" => Some(Matrix::Bt601),
                "bt709" => Some(Matrix::Bt709),
                _ => return Err(bad("auto, bt601 or bt709")),
            }
        }
        "range" => {
            f.range = match v {
                "limited" => Range::Limited,
                "full" => Range::Full,
                _ => return Err(bad("limited or full")),
            }
        }
        "audio" => f.audio = pick(key, v, &AudioCodec::ALL, audio_word)?,
        "abitrate" => o.audio_bitrate = parse_bits(v).filter(|b| (32_000..=512_000).contains(b)).ok_or_else(|| bad("32k–512k"))?,
        "size" => {
            o.resolution = if v == "project" {
                None
            } else {
                let (w, h) = v.split_once(['x', '×']).ok_or_else(|| bad("WxH or project"))?;
                let d = |s: &str| s.trim().parse::<u32>().ok().filter(|n| (16..=8192).contains(n));
                Some((d(w).ok_or_else(|| bad("WxH, 16–8192"))?, d(h).ok_or_else(|| bad("WxH, 16–8192"))?))
            }
        }
        "fps" => {
            o.fps = if v == "project" {
                None
            } else {
                let n = num(1, 240)?;
                // the frame grid is whole ticks (time-model)
                if 90_000 % n != 0 {
                    return Err(bad("a rate that divides 90000 (24, 25, 30, 50, 60 …)"));
                }
                Some(n)
            }
        }
        "title" => f.title = v.to_string(),
        "comment" => f.comment = v.to_string(),
        _ => return Err(format!("{key}: not a render setting (known: {})", KEYS.join(", "))),
    }
    Ok(())
}

/// Reconcile the rate mode with the bitrate: Quality keeps no bitrate, a rate mode has one.
pub fn finish(o: &mut ExportOptions) {
    match o.format.rate {
        RateMode::Quality => o.video_bitrate = None,
        _ => {
            o.video_bitrate.get_or_insert(DEFAULT_BITRATE);
        }
    }
}

/// Settings from pairs over `base`.
pub fn from_pairs<'a>(base: ExportOptions, it: impl IntoIterator<Item = (&'a str, &'a str)>) -> Result<ExportOptions, String> {
    let mut o = base;
    for (k, v) in it {
        apply(&mut o, k, v)?;
    }
    finish(&mut o);
    Ok(o)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kt_media::muxer::OutputFormat;

    /// t2800 (render-settings D9): every option set off its default goes to pairs and back
    /// unchanged; an unknown key and a bad value are errors naming them.
    #[test]
    fn t2800_pairs_round_trip_and_refuse_what_they_do_not_know() {
        let o = ExportOptions {
            quality: 30,
            video_bitrate: Some(6_500_000),
            audio_bitrate: 256_000,
            force_encoder: Some("libx265".into()),
            resolution: Some((1280, 720)),
            fps: Some(25),
            format: OutputFormat {
                container: Container::Mkv,
                video: VideoCodec::H265,
                rate: RateMode::Cbr,
                max_bitrate: Some(9_000_000),
                keyint_tenths: 5,
                b_frames: 3,
                speed: Speed::Slowest,
                depth: BitDepth::Ten,
                matrix: Some(Matrix::Bt709),
                range: Range::Full,
                audio: AudioCodec::Flac,
                title: "A = b".into(),
                comment: "two words".into(),
            },
            ..Default::default()
        };
        let p = pairs(&o);
        let back = from_pairs(ExportOptions::default(), p.iter().map(|(k, v)| (k.as_str(), v.as_str()))).unwrap();
        assert_eq!(pairs(&back), p);
        assert_eq!(back.format, o.format);
        assert_eq!((back.quality, back.video_bitrate, back.audio_bitrate), (30, Some(6_500_000), 256_000));
        assert_eq!((back.force_encoder.as_deref(), back.resolution, back.fps), (Some("libx265"), Some((1280, 720)), Some(25)));
        for (k, _) in &p {
            assert!(known_key(k), "{k}");
        }
        // the default writes no bitrate and reads back to the default file
        let d = from_pairs(ExportOptions::default(), pairs(&ExportOptions::default()).iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .unwrap();
        assert_eq!(d.format, OutputFormat::default());
        assert_eq!(d.video_bitrate, None);
        // a rate mode without a bitrate starts from the default one
        let v = from_pairs(ExportOptions::default(), [("rate", "vbr")]).unwrap();
        assert_eq!(v.video_bitrate, Some(DEFAULT_BITRATE));

        let e = from_pairs(ExportOptions::default(), [("speed", "warp")]).unwrap_err();
        assert!(e.starts_with("speed:"), "{e}");
        let e = from_pairs(ExportOptions::default(), [("colour", "red")]).unwrap_err();
        assert!(e.starts_with("colour:"), "{e}");
        let e = from_pairs(ExportOptions::default(), [("fps", "29")]).unwrap_err();
        assert!(e.starts_with("fps:"), "{e}");
        assert_eq!(parse_bits("8.5M"), Some(8_500_000));
        assert_eq!(parse_bits("192k"), Some(192_000));
    }
}
