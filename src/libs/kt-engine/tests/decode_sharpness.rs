//! Тест 315 (Р.46, п.321): РЕЗКОСТЬ. Смысл всего раунда одной метрикой —
//! зум больше 100 % на файле с запасом натива обязан показывать детали
//! файла, а не мыло из кадра, ужатого в Quality ещё при декоде.
//!
//! Файл самодостаточен НАМЕРЕННО (свой источник, только стабильные API):
//! его копируют в worktree на предыдущем коммите и убеждаются, что там он
//! красный — иначе тест ничего не доказывает.

use kt_media::{MediaError, MediaInfo, MediaSource, Quality, VideoFrame};
use kt_model::*;
use kt_time::Tick;

const SEC: i64 = 90_000;
const W: u32 = 128;
const H: u32 = 72;
/// Натив вчетверо крупнее проекта: запас для ступени 2 есть с избытком.
const NW: u32 = 512;
const NH: u32 = 288;

/// Источник с настоящей фактурой: полосы шириной 2 px В НАТИВЕ и честная
/// box-минификация. Ужатие вдвое их сохраняет, вчетверо — гасит в серый.
/// (Копия `kt_media::synthetic::CheckerSource`: тест обязан компилироваться
/// на коммите ДО раунда, где её ещё нет.)
struct Checker;

impl Checker {
    fn native_px(x: u32) -> u8 {
        if (x / 2) % 2 == 0 { 16 } else { 239 }
    }
}

impl MediaSource for Checker {
    fn info(&self) -> &MediaInfo {
        static INFO: std::sync::OnceLock<MediaInfo> = std::sync::OnceLock::new();
        INFO.get_or_init(|| MediaInfo {
            duration: Tick(10 * SEC),
            has_video: true,
            has_audio: false,
            fps: Some((30, 1)),
            width: NW,
            height: NH,
        })
    }

    fn read_audio_burst(&mut self, idx: i64) -> Result<kt_media::AudioBurst, MediaError> {
        Ok(kt_media::AudioBurst::silence(idx))
    }

    fn read_video_frame(
        &mut self,
        pts: Tick,
        quality: Option<Quality>,
    ) -> Result<VideoFrame, MediaError> {
        let q = quality.unwrap_or(Quality { width: NW, height: NH });
        let (w, h) = (q.width.max(2) & !1, q.height.max(2) & !1);
        let mut rgba = vec![0u8; (w * h * 4) as usize];
        for x in 0..w {
            let x0 = (x as u64 * NW as u64 / w as u64) as u32;
            let x1 = (((x as u64 + 1) * NW as u64).div_ceil(w as u64) as u32).max(x0 + 1).min(NW);
            let mut sum = 0u32;
            for nx in x0..x1 {
                sum += Self::native_px(nx) as u32;
            }
            let v = (sum / (x1 - x0)) as u8;
            for y in 0..h {
                let i = ((y * w + x) * 4) as usize;
                rgba[i] = v;
                rgba[i + 1] = v;
                rgba[i + 2] = v;
                rgba[i + 3] = 255;
            }
        }
        Ok(VideoFrame { pts, width: w, height: h, rgba: rgba.into_boxed_slice(), has_alpha: false })
    }

    fn keyframe_before(&self, pts: Tick) -> Option<Tick> {
        Some(Tick(pts.0.max(0)))
    }
}

/// Средний горизонтальный контраст красного канала (0..255): мера того,
/// что фактура вообще дожила до выхода.
fn contrast(rgba: &[u8], w: u32, h: u32) -> f64 {
    let mut sum = 0u64;
    let mut n = 0u64;
    for y in 0..h {
        for x in 1..w {
            let i = ((y * w + x) * 4) as usize;
            let j = ((y * w + x - 1) * 4) as usize;
            sum += (rgba[i] as i64 - rgba[j] as i64).unsigned_abs();
            n += 1;
        }
    }
    sum as f64 / n as f64
}

fn project(fx: Vec<FxInstance>) -> Project {
    let mut p = Project::new(ProjectSettings { fps: 30, width: W, height: H, sample_rate: 48000 });
    p.media.push(MediaRef { id: MediaId(1), path: "checker".into() });
    p.clips.push(ClipEvent {
        id: ClipId(1),
        link_group: None,
        kind: ClipKind::Video,
        lane: 0,
        start: Tick(0),
        duration: Tick(SEC),
        source_in: Tick(0),
        media: MediaId(1),
        gain_db: 0.0,
        reversed: false,
        fx,
        fx_bypass: vec![],
        fx_auto: vec![],
        opacity: 1.0,
        fx_off: false,
    });
    p.validate().unwrap();
    p
}

/// Первый кадр экспорта проекта (тот же путь, что у превью).
fn first_frame(p: &Project) -> Vec<u8> {
    let sources: Vec<(MediaId, Box<dyn MediaSource>)> = vec![(MediaId(1), Box::new(Checker))];
    let mut out = Vec::new();
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    kt_engine::export::render_raw_video_range_with(
        p,
        sources,
        Quality { width: W, height: H },
        Some((Tick(0), Tick(3000))),
        &Default::default(),
        &cancel,
        |_| {},
        |frame| {
            if out.is_empty() {
                out.extend_from_slice(frame);
            }
            Ok(())
        },
    )
    .expect("render");
    out
}

/// Тест 315: зум ×2 на файле с запасом натива обязан быть РЕЗКИМ.
///
/// Проект 128×72, файл 512×288. До Р.46 декодер отдавал кадр сразу в
/// 128×72 — фактура файла (полосы по 2 px натива) усреднялась в ровный
/// серый ещё ДО цепочки, и зумить было уже нечего. Со ступенью декода
/// кадр приходит в 256×144, где полосы живы, и зум ×2 кладёт их на выход
/// один в один.
#[test]
fn t315_zoom_beyond_quality_is_sharp() {
    let zoomed = first_frame(&project(vec![FxInstance::Pan {
        mode: PanMode::Stretch,
        pan_x: 0.0,
        pan_y: 0.0,
        zoom: 2.0,
    }]));
    let c = contrast(&zoomed, W, H);
    println!("t315 contrast: {c:.1}");
    // до Р.46 здесь ровно 0.0 (серый); порог — с большим запасом ниже
    // измеренного (полный контраст полос ≈ 223)
    assert!(c >= 100.0, "зум за 100 % обязан показать фактуру файла, контраст {c:.1}");

    // контроль: без зума ступени нет, и кадр честно мыльный — фичу нельзя
    // «выиграть», раздувая декод всем подряд
    let plain = first_frame(&project(vec![]));
    let cp = contrast(&plain, W, H);
    println!("t315 plain contrast: {cp:.1}");
    assert!(cp < 1.0, "без нетто-зума декод остаётся в Quality, контраст {cp:.1}");
}
