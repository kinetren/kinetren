//! The colour of a decoded source (source-colour-matrix): which matrix and range turn its YUV
//! into RGB. Pure — the numbers are libavutil's `AVColorSpace`/`AVColorRange`/`AVColorPrimaries`,
//! fixed by ITU-T H.273 and the same on every major 6–9 (t2891 compares them with each module).

/// `AVCOL_SPC_UNSPECIFIED`, `AVCOL_RANGE_UNSPECIFIED`, `AVCOL_PRI_UNSPECIFIED`.
pub const MATRIX_UNSPECIFIED: u32 = 2;
pub const RANGE_UNSPECIFIED: u32 = 0;
pub const PRIMARIES_UNSPECIFIED: u32 = 2;

/// A stream's or a picture's colour tags as libavutil numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ColourTags {
    pub matrix: u32,
    pub range: u32,
    pub primaries: u32,
}

impl Default for ColourTags {
    fn default() -> ColourTags {
        ColourTags { matrix: MATRIX_UNSPECIFIED, range: RANGE_UNSPECIFIED, primaries: PRIMARIES_UNSPECIFIED }
    }
}

/// The YUV→RGB matrices swscale knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceMatrix {
    Bt601,
    Bt709,
    Fcc,
    Smpte240,
    Bt2020,
}

impl SourceMatrix {
    /// The `SWS_CS_*` number `sws_getCoefficients` takes.
    pub fn sws_cs(self) -> i32 {
        match self {
            SourceMatrix::Bt709 => 1,
            SourceMatrix::Fcc => 4,
            SourceMatrix::Bt601 => 5,
            SourceMatrix::Smpte240 => 7,
            SourceMatrix::Bt2020 => 9,
        }
    }
}

/// How a source's YUV becomes RGB, and whether each half came from a tag or from the rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceColour {
    pub matrix: SourceMatrix,
    pub full_range: bool,
    pub matrix_tagged: bool,
    pub range_tagged: bool,
}

impl SourceColour {
    /// What `sws_getContext` converts with when told nothing: BT.601, limited range unless the
    /// pixel format is a `yuvj*` one. A source resolving to this leaves swscale untouched, so its
    /// pictures stay the former bytes.
    pub fn swscale_default(jpeg_format: bool) -> (SourceMatrix, bool) {
        (SourceMatrix::Bt601, jpeg_format)
    }
}

/// A picture is HD by the rule players use for an untagged one (mpv, browsers): 1280 wide or
/// more than 576 lines.
pub fn is_hd(width: u32, height: u32) -> bool {
    width >= 1280 || height > 576
}

impl ColourTags {
    /// Each unspecified field taken from `stream` (the container's or the codec parameters' tag):
    /// the picture's own tag wins.
    pub fn or(self, stream: ColourTags) -> ColourTags {
        ColourTags {
            matrix: if self.matrix == MATRIX_UNSPECIFIED { stream.matrix } else { self.matrix },
            range: if self.range == RANGE_UNSPECIFIED { stream.range } else { self.range },
            primaries: if self.primaries == PRIMARIES_UNSPECIFIED { stream.primaries } else { self.primaries },
        }
    }

    /// The matrix and range for a `width`×`height` picture. A tag swscale cannot convert
    /// (unspecified, reserved, RGB, YCgCo, ICtCp…) falls to the rule: HD → BT.709, SD → BT.601;
    /// an unspecified range is full only for a `yuvj*` format.
    pub fn resolve(self, width: u32, height: u32, jpeg_format: bool) -> SourceColour {
        let tagged = match self.matrix {
            1 => Some(SourceMatrix::Bt709),
            4 => Some(SourceMatrix::Fcc),
            5 | 6 => Some(SourceMatrix::Bt601),
            7 => Some(SourceMatrix::Smpte240),
            // constant luminance has no swscale matrix: the non-constant one is the nearest
            9 | 10 => Some(SourceMatrix::Bt2020),
            _ => None,
        };
        let matrix = tagged.unwrap_or(if is_hd(width, height) { SourceMatrix::Bt709 } else { SourceMatrix::Bt601 });
        let (full_range, range_tagged) = match self.range {
            1 => (false, true),
            2 => (true, true),
            _ => (jpeg_format, false),
        };
        SourceColour { matrix, full_range, matrix_tagged: tagged.is_some(), range_tagged }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// t2890: the tag wins, an unspecified one falls to HD → 709 / SD → 601, range from the tag
    /// or the `yuvj` format; a picture's unspecified tag takes the stream's.
    #[test]
    fn t2890_resolve_rule() {
        let t = |m, r| ColourTags { matrix: m, range: r, primaries: PRIMARIES_UNSPECIFIED };
        let un = ColourTags::default();
        assert_eq!(un.resolve(1920, 1080, false).matrix, SourceMatrix::Bt709);
        assert_eq!(un.resolve(1280, 720, false).matrix, SourceMatrix::Bt709);
        assert_eq!(un.resolve(720, 576, false).matrix, SourceMatrix::Bt601);
        assert_eq!(un.resolve(640, 480, false).matrix, SourceMatrix::Bt601);
        assert_eq!(un.resolve(720, 1280, false).matrix, SourceMatrix::Bt709, "a portrait phone video is HD");
        assert!(!un.resolve(1920, 1080, false).full_range);
        assert!(un.resolve(320, 180, true).full_range, "yuvj is full range");
        assert_eq!(t(6, 0).resolve(1920, 1080, false).matrix, SourceMatrix::Bt601, "a tag beats the size");
        assert_eq!(t(1, 0).resolve(320, 180, false).matrix, SourceMatrix::Bt709);
        assert_eq!(t(5, 0).resolve(1920, 1080, false).matrix, SourceMatrix::Bt601);
        assert_eq!(t(9, 0).resolve(3840, 2160, false).matrix, SourceMatrix::Bt2020);
        assert_eq!(t(0, 0).resolve(1920, 1080, false).matrix, SourceMatrix::Bt709, "RGB tag on YUV: the rule");
        assert!(t(1, 2).resolve(320, 180, false).full_range);
        assert!(!t(1, 1).resolve(320, 180, true).full_range, "a limited tag beats the yuvj format");
        let c = t(1, 1).resolve(1920, 1080, false);
        assert!(c.matrix_tagged && c.range_tagged);
        let c = un.resolve(1920, 1080, false);
        assert!(!c.matrix_tagged && !c.range_tagged);
        let stream = ColourTags { matrix: 1, range: 2, primaries: 1 };
        assert_eq!(un.or(stream), stream);
        assert_eq!(t(6, 1).or(stream), ColourTags { matrix: 6, range: 1, primaries: 1 });
        assert_eq!(SourceColour::swscale_default(false), (SourceMatrix::Bt601, false));
        assert_eq!(SourceMatrix::Bt601.sws_cs(), 5);
        assert_eq!(SourceMatrix::Bt709.sws_cs(), 1);
    }

    /// t2891: the numbers above are libavutil's on every major's headers.
    #[test]
    fn t2891_numbers_equal_every_major() {
        macro_rules! check {
            ($m:ident) => {{
                use crate::$m::ffi as f;
                assert_eq!(f::AVCOL_SPC_UNSPECIFIED as u32, MATRIX_UNSPECIFIED);
                assert_eq!(f::AVCOL_RANGE_UNSPECIFIED as u32, RANGE_UNSPECIFIED);
                assert_eq!(f::AVCOL_PRI_UNSPECIFIED as u32, PRIMARIES_UNSPECIFIED);
                assert_eq!(f::AVCOL_RANGE_MPEG as u32, 1);
                assert_eq!(f::AVCOL_RANGE_JPEG as u32, 2);
                assert_eq!(f::AVCOL_SPC_BT709 as u32, 1);
                assert_eq!(f::AVCOL_SPC_FCC as u32, 4);
                assert_eq!(f::AVCOL_SPC_BT470BG as u32, 5);
                assert_eq!(f::AVCOL_SPC_SMPTE170M as u32, 6);
                assert_eq!(f::AVCOL_SPC_SMPTE240M as u32, 7);
                assert_eq!(f::AVCOL_SPC_BT2020_NCL as u32, 9);
                assert_eq!(f::AVCOL_SPC_BT2020_CL as u32, 10);
            }};
        }
        check!(v6);
        check!(v7);
        check!(v8);
        check!(v9);
    }
}
