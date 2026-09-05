//! Плейсхолдер отсутствующего исходника (Р.33 п.10, требование
//! пользователя: «при рендере и превью отсутсвие исходников не должно быть
//! блокером, нужно уведомлять юзера и рендерить, для отсутствующего видео
//! надо подставлять placeholder»).
//!
//! Источник ведёт себя как обычный: отдаёт тёмный кадр с ИМЕНЕМ файла
//! (сразу видно, чего не хватает) и тишину в аудио. Владеет им, как всеми
//! остальными, ровно один MediaWorker-поток.
//!
//! Текст рисуется встроенным пиксельным шрифтом 3×5: он не зависит ни от
//! системных шрифтов, ни от рендерера egui, поэтому кадр детерминирован и
//! одинаков в превью, экспорте и тестах. Символы вне таблицы (кириллица,
//! иероглифы) показываются как «?» — имя файла остаётся узнаваемым по
//! латинице, цифрам и расширению.

use crate::{AudioBurst, MediaError, MediaInfo, MediaSource, Quality, VideoFrame};
use kt_time::Tick;

/// «Практически бесконечная» длительность: 1000 часов. Плейсхолдер не
/// знает, сколько длился пропавший файл, а обрезать монтаж он не должен.
const PLACEHOLDER_DURATION: Tick = Tick(90_000 * 3600 * 1000);
const DEFAULT_W: u32 = 640;
const DEFAULT_H: u32 = 360;

const BG: [u8; 4] = [42, 44, 48, 255];
const INK: [u8; 4] = [188, 192, 200, 255];
const MARK: [u8; 4] = [214, 122, 96, 255];

const GLYPH_W: usize = 3;
const GLYPH_H: usize = 5;

/// Шрифт 3×5: строки глифа разделены '/', '#' — пиксель. Строчные буквы
/// приводятся к заглавным при отрисовке.
static FONT: &[(char, &str)] = &[
    (' ', ".../.../.../.../..."),
    ('0', "###/#.#/#.#/#.#/###"),
    ('1', ".#./##./.#./.#./###"),
    ('2', "###/..#/###/#../###"),
    ('3', "###/..#/###/..#/###"),
    ('4', "#.#/#.#/###/..#/..#"),
    ('5', "###/#../###/..#/###"),
    ('6', "###/#../###/#.#/###"),
    ('7', "###/..#/..#/..#/..#"),
    ('8', "###/#.#/###/#.#/###"),
    ('9', "###/#.#/###/..#/###"),
    ('A', "###/#.#/###/#.#/#.#"),
    ('B', "##./#.#/##./#.#/##."),
    ('C', "###/#../#../#../###"),
    ('D', "##./#.#/#.#/#.#/##."),
    ('E', "###/#../###/#../###"),
    ('F', "###/#../###/#../#.."),
    ('G', "###/#../#.#/#.#/###"),
    ('H', "#.#/#.#/###/#.#/#.#"),
    ('I', "###/.#./.#./.#./###"),
    ('J', "..#/..#/..#/#.#/###"),
    ('K', "#.#/#.#/##./#.#/#.#"),
    ('L', "#../#../#../#../###"),
    ('M', "#.#/###/###/#.#/#.#"),
    ('N', "#.#/##./###/.##/#.#"),
    ('O', "###/#.#/#.#/#.#/###"),
    ('P', "###/#.#/###/#../#.."),
    ('Q', "###/#.#/#.#/###/..#"),
    ('R', "###/#.#/###/##./#.#"),
    ('S', "###/#../###/..#/###"),
    ('T', "###/.#./.#./.#./.#."),
    ('U', "#.#/#.#/#.#/#.#/###"),
    ('V', "#.#/#.#/#.#/#.#/.#."),
    ('W', "#.#/#.#/###/###/#.#"),
    ('X', "#.#/#.#/.#./#.#/#.#"),
    ('Y', "#.#/#.#/.#./.#./.#."),
    ('Z', "###/..#/.#./#../###"),
    ('.', ".../.../.../.../.#."),
    (',', ".../.../.../.#./#.."),
    ('-', ".../.../###/.../..."),
    ('_', ".../.../.../.../###"),
    (':', ".../.#./.../.#./..."),
    (';', ".../.#./.../.#./#.."),
    ('\'', ".#./.#./.../.../..."),
    ('"', "#.#/#.#/.../.../..."),
    ('(', "..#/.#./.#./.#./..#"),
    (')', "#../.#./.#./.#./#.."),
    ('[', ".##/.#./.#./.#./.##"),
    (']', "##./.#./.#./.#./##."),
    ('{', ".##/.#./##./.#./.##"),
    ('}', "##./.#./.##/.#./##."),
    ('/', "..#/..#/.#./#../#.."),
    ('\\', "#../#../.#./..#/..#"),
    ('+', ".../.#./###/.#./..."),
    ('=', ".../###/.../###/..."),
    ('!', ".#./.#./.#./.../.#."),
    ('?', "###/..#/.#./.../.#."),
    ('#', "#.#/###/#.#/###/#.#"),
    ('@', "###/#.#/###/#../.##"),
    ('%', "#.#/..#/.#./#../#.#"),
    ('&', "##./##./###/#.#/.##"),
    ('*', "#.#/.#./#.#/.../..."),
    ('~', ".../.##/##./.../..."),
    ('$', ".#./###/##./.##/.#."),
    ('<', "..#/.#./#../.#./..#"),
    ('>', "#../.#./..#/.#./#.."),
    ('|', ".#./.#./.#./.#./.#."),
    ('^', ".#./#.#/.../.../..."),
    ('`', "#../.#./.../.../..."),
];

fn glyph(c: char) -> &'static str {
    let up = c.to_ascii_uppercase();
    FONT.iter()
        .find(|(g, _)| *g == up)
        .or_else(|| FONT.iter().find(|(g, _)| *g == '?'))
        .map(|(_, rows)| *rows)
        .unwrap_or(".../.../.../.../...")
}

/// Плейсхолдер вместо недоступного файла: тёмный кадр с именем и значком
/// плёнки, тишина в аудио.
pub struct PlaceholderSource {
    info: MediaInfo,
    label: String,
    /// Кадр строится один раз на запрошенное качество: он не зависит ни от
    /// времени, ни от чего-либо ещё, а рисовать его на каждый тик — трата.
    cache: Option<(Quality, Box<[u8]>)>,
}

impl PlaceholderSource {
    /// `label` — то, чего не хватает (обычно имя файла).
    pub fn new(label: impl Into<String>) -> PlaceholderSource {
        PlaceholderSource {
            info: MediaInfo {
                duration: PLACEHOLDER_DURATION,
                has_video: true,
                has_audio: true,
                // своей сетки кадров нет: движок планирует по сетке ПРОЕКТА
                fps: None,
                width: DEFAULT_W,
                height: DEFAULT_H,
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

/// Холст кадра: буфер RGBA и его размеры. Отдельный тип вместо пачки
/// параметров в каждой рисовалке — иначе у них по восемь аргументов.
struct Canvas {
    buf: Vec<u8>,
    w: i64,
    h: i64,
}

impl Canvas {
    fn new(w: u32, h: u32, bg: [u8; 4]) -> Canvas {
        let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
        for px in buf.chunks_exact_mut(4) {
            px.copy_from_slice(&bg);
        }
        Canvas { buf, w: w as i64, h: h as i64 }
    }

    fn rect(&mut self, x0: i64, y0: i64, rw: i64, rh: i64, c: [u8; 4]) {
        for y in y0.max(0)..(y0 + rh).min(self.h) {
            for x in x0.max(0)..(x0 + rw).min(self.w) {
                let i = ((y * self.w + x) * 4) as usize;
                self.buf[i..i + 4].copy_from_slice(&c);
            }
        }
    }

    /// Строка встроенным шрифтом с масштабом `s`, левый верхний угол (x, y).
    fn text(&mut self, x: i64, y: i64, s: i64, text: &str, c: [u8; 4]) {
        let mut cx = x;
        for ch in text.chars() {
            for (ry, row) in glyph(ch).split('/').enumerate() {
                for (rx, px) in row.chars().enumerate() {
                    if px == '#' {
                        self.rect(cx + rx as i64 * s, y + ry as i64 * s, s, s, c);
                    }
                }
            }
            cx += (GLYPH_W as i64 + 1) * s;
        }
    }

    /// Значок плёнки: рамка кадра с перфорацией по краям.
    fn film(&mut self, x: i64, y: i64, s: i64, c: [u8; 4]) {
        let (fw, fh) = (14 * s, 10 * s);
        self.rect(x, y, fw, s, c);
        self.rect(x, y + fh - s, fw, s, c);
        self.rect(x, y, s, fh, c);
        self.rect(x + fw - s, y, s, fh, c);
        for i in 0..4 {
            let px = x + s * 2 + i * 3 * s;
            self.rect(px, y + s * 2, s, s, c);
            self.rect(px, y + fh - s * 3, s, s, c);
        }
    }
}

fn text_width(len: usize, s: i64) -> i64 {
    if len == 0 { 0 } else { (len as i64 * (GLYPH_W as i64 + 1) - 1) * s }
}

fn render(label: &str, w: u32, h: u32) -> Box<[u8]> {
    let (w, h) = (w.max(1), h.max(1));
    let mut c = Canvas::new(w, h, BG);
    // рамка кадра — видно, что это заглушка, а не чёрный провал
    let b = (w.min(h) as i64 / 90).max(1);
    c.rect(0, 0, c.w, b, MARK);
    c.rect(0, c.h - b, c.w, b, MARK);
    c.rect(0, 0, b, c.h, MARK);
    c.rect(c.w - b, 0, b, c.h, MARK);

    // имя: масштаб подбирается под ширину кадра, слишком длинное режется
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
    // значок над надписью
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

    /// Тест 208 (Р.33 п.10): кадр плейсхолдера детерминирован, реально
    /// содержит надпись и не зависит от системных шрифтов.
    #[test]
    fn t208_placeholder_frame_is_deterministic_and_labeled() {
        let q = Quality { width: 320, height: 180 };
        let a = frame("clip-01.mp4", q);
        let b = frame("clip-01.mp4", q);
        assert_eq!(a.rgba, b.rgba, "кадр обязан быть одинаковым при каждом запросе");
        assert_eq!((a.width, a.height), (320, 180));

        // второй запрос ТОГО ЖЕ источника берётся из кэша и совпадает
        let mut src = PlaceholderSource::new("clip-01.mp4");
        let f1 = src.read_video_frame(Tick(0), Some(q)).unwrap();
        let f2 = src.read_video_frame(Tick(90_000), Some(q)).unwrap();
        assert_eq!(f1.rgba, f2.rgba, "кадр не должен зависеть от времени");
        assert_eq!(f2.pts.0, 90_000, "pts обязан быть запрошенный");

        // надпись видна: пикселей цвета текста заметно много и кадр не однотонный
        let ink = a.rgba.chunks_exact(4).filter(|p| p[0] == INK[0] && p[1] == INK[1]).count();
        assert!(ink > 50, "надписи на кадре нет: {ink} пикселей");
        let other = frame("zzzz-99.mov", q);
        assert_ne!(a.rgba, other.rgba, "разные имена обязаны давать разные кадры");
    }

    /// Не-ASCII (кириллица, иероглифы) рисуется как «?» — кадр остаётся
    /// детерминированным и не зависит от установленных шрифтов.
    #[test]
    fn t208_non_ascii_label_falls_back_to_question_marks() {
        let q = Quality { width: 320, height: 180 };
        let cyr = frame("Ролик.mp4", q);
        let subs = frame("?????.mp4", q);
        assert_eq!(cyr.rgba, subs.rgba, "не-ASCII обязан заменяться на «?»");
    }

    #[test]
    fn t208_audio_is_silence_and_source_is_long() {
        let mut s = PlaceholderSource::new("gone.wav");
        let b = s.read_audio_burst(7).unwrap();
        assert_eq!(b.idx, 7);
        assert!(b.data.iter().all(|v| *v == 0.0), "плейсхолдер обязан молчать");
        assert!(s.info().duration.0 > 90_000 * 3600, "длительность не должна резать монтаж");
        assert!(s.info().has_video && s.info().has_audio);
    }
}
