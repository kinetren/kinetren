//! Stereo peak meters + master fader (port of AudioLevelMeterWidget/
//! PreviewVolumeFader): color ladder, peak-hold caps with decay,
//! fader −60..+24 dB, double-click = 0 dB, button M = mute.

use crate::app::KtApp;
use crate::i18n::{k, tr};
use eframe::egui;

pub const METER_MIN_DB: f32 = -48.0;
pub const METER_MAX_DB: f32 = 6.0;
pub const FADER_MIN_DB: f32 = -60.0;
/// Stiffness of the fader’s approach (rad/s) and the convergence threshold.
pub const FADER_OMEGA: f32 = 26.0;
pub const FADER_EPS_DB: f32 = 0.02;
pub const FADER_MAX_DB: f32 = 24.0;
/// Decay of the bar/caps, dB per tick of 25 ms (port: 2.8 / 0.2).
pub const BAR_FALL_DB: f32 = 2.8;
pub const CAP_FALL_DB: f32 = 0.2;

pub struct MeterState {
    pub bar_db: [f32; 2],
    pub cap_db: [f32; 2],
    /// Target level: the bars decay TOWARD IT, not toward the minimum. In pause
    /// it holds the last sample — without a “saw” between rare measurements (item 2).
    pub target_db: [f32; 2],
    pub fader_db: f32,
    /// Where the mouse/wheel pulls: the fader moves toward the target with inertia, like a
    /// physical slider (item 2, round 8).
    pub fader_target: f32,
    /// Speed of the fader’s movement, dB/s (critical damping).
    pub fader_vel: f32,
    pub muted: bool,
    last_tick: std::time::Instant,
    last_sample: std::time::Instant,
    /// The cursor position of the previous frame and the moment of its last change:
    /// on pause the meter fades, but on scrub/seek it shows the level
    /// UNDER THE CURSOR (the user’s decision, item 4, round 16).
    last_playhead: i64,
    last_move: std::time::Instant,
}

/// How long after the last cursor movement the meter still shows the level
/// under it (on pause). After that — it fades.
///
/// 1.2 s, not half a second: the measurement is taken FROM THE CACHE (`sample_levels_at`
/// reads CacheFetcher), and at a fresh position the first probes return
/// silence until prefetch delivers the burst. Too narrow a window faded the
/// meter before data appeared under the cursor.
pub const SCRUB_SHOW_MS: u128 = 1200;

/// Whether the meter holds the measurement under the cursor (item 4, round 16). During play — no (there is
/// the live stream); on pause — only while the cursor is being MOVED: scrub and
/// seek show the level under the cursor, while a motionless pause fades the
/// meter instead of the former “freezing”.
pub fn meter_holds_level(playing: bool, since_move_ms: u128) -> bool {
    !playing && since_move_ms < SCRUB_SHOW_MS
}

impl MeterState {
    /// Whether there is anything to animate (for the repaint policy, item 1 v1.4): the bars/caps
    /// are MOVING, not just lit. The static display of the level at the cursor in
    /// pause (feature, item 2 v1.1) must not hold 60 fps: previously the condition was
    /// “bar above the minimum” and it stuck at true forever after the first sound —
    /// the app kept drawing 60 fps when idle and heated the whole system.
    /// Test 38.
    pub fn is_active(&self) -> bool {
        // the fader in motion also holds the redraw — but exactly until
        // convergence (the snap in fader_tick), otherwise the eternal 60 fps in
        // idle would return, for which t38 was written
        if (self.fader_db - self.fader_target).abs() > FADER_EPS_DB
            || self.fader_vel.abs() > FADER_EPS_DB
        {
            return true;
        }
        (0..2).any(|ch| {
            let floor = self.target_db[ch].max(METER_MIN_DB);
            (self.bar_db[ch] - floor).abs() > 0.05
                || (self.cap_db[ch] - self.bar_db[ch] > 0.05
                    && self.cap_db[ch] > METER_MIN_DB + 0.05)
        })
    }

    /// The fader’s inertia step (critical damping, dt in seconds).
    /// A pure function for test 80: it must CONVERGE and stop exactly
    /// on the target, not crawl on forever.
    pub fn fader_tick(&mut self, dt: f32) {
        let dt = dt.clamp(0.0, 0.1);
        if dt <= 0.0 {
            return;
        }
        // EXACT solution of x'' + 2ωx' + ω²x = 0 at ζ=1: x(t) = (A + Bt)e^{−ωt}.
        // Explicit Euler here DIVERGED for ω·dt > 2√2−2, that is already at
        // dt > 32 ms: and stable_dt after an idle comes in equal to 100 ms
        // (clamp) — the deviation was multiplied by −10 per frame, the fader flew off by
        // hundreds of dB and this went into the sound (item 7, round 9). The exponential does not diverge
        // at any dt.
        let w = FADER_OMEGA;
        let x0 = self.fader_db - self.fader_target;
        let b = self.fader_vel + w * x0;
        let e = (-w * dt).exp();
        let xt = (x0 + b * dt) * e;
        self.fader_db = (self.fader_target + xt).clamp(FADER_MIN_DB, FADER_MAX_DB);
        self.fader_vel = (b - w * (x0 + b * dt)) * e;
        if (self.fader_target - self.fader_db).abs() <= FADER_EPS_DB
            && self.fader_vel.abs() <= FADER_EPS_DB
        {
            self.fader_db = self.fader_target;
            self.fader_vel = 0.0;
        }
    }

    /// One decay step (tick 25 ms, the v1 decays) — pure logic for test 38.
    /// `release` releases the target itself too (otherwise when the feeding stops the
    /// bars would hang forever). We release during play AND on pause when the
    /// cursor is not moved: “on pause the monitor does not fade but freezes” —
    /// the complaint of item 4, round 16; while scrubbing the target holds the measurement under the cursor.
    pub fn tick(&mut self, ticks: f32, release: bool) {
        for ch in 0..2 {
            let floor = self.target_db[ch].max(METER_MIN_DB);
            self.bar_db[ch] = (self.bar_db[ch] - BAR_FALL_DB * ticks).max(floor);
            self.cap_db[ch] = (self.cap_db[ch] - CAP_FALL_DB * ticks).max(METER_MIN_DB);
            if release {
                self.target_db[ch] =
                    (self.target_db[ch] - BAR_FALL_DB * 0.7 * ticks).max(METER_MIN_DB);
            }
        }
        for ch in 0..2 {
            self.bar_db[ch] = self.bar_db[ch].max(self.target_db[ch]);
            self.cap_db[ch] = self.cap_db[ch].max(self.bar_db[ch]);
        }
    }

    /// Feeding of the level measurement (linear 0..1 per channel).
    pub fn feed(&mut self, pl: f32, pr: f32) {
        self.target_db[0] = db_of(pl);
        self.target_db[1] = db_of(pr);
    }
}

impl Default for MeterState {
    fn default() -> Self {
        MeterState {
            bar_db: [METER_MIN_DB; 2],
            cap_db: [METER_MIN_DB; 2],
            target_db: [METER_MIN_DB; 2],
            fader_db: 0.0,
            fader_target: 0.0,
            fader_vel: 0.0,
            muted: false,
            last_tick: std::time::Instant::now(),
            last_sample: std::time::Instant::now(),
            last_playhead: i64::MIN,
            last_move: std::time::Instant::now(),
        }
    }
}

fn db_of(linear: f32) -> f32 {
    if linear <= 1e-6 { METER_MIN_DB } else { (20.0 * linear.log10()).max(METER_MIN_DB) }
}

pub(crate) fn zone_color(db: f32) -> egui::Color32 {
    if db > 0.0 {
        egui::Color32::from_rgb(255, 60, 40)
    } else if db >= -3.0 {
        egui::Color32::from_rgb(235, 80, 60)
    } else if db >= -9.0 {
        egui::Color32::from_rgb(240, 180, 60)
    } else if db >= -18.0 {
        egui::Color32::from_rgb(120, 230, 90)
    } else if db >= -36.0 {
        egui::Color32::from_rgb(70, 190, 80)
    } else {
        egui::Color32::from_rgb(40, 120, 60)
    }
}

/// Permitted steps of the LED grid in DECIBELS (item 8, round 11, refined by item 1, round 12).
/// Each divides EXACTLY all the color zones (6/3/6/9/18/12 dB), therefore a
/// segment is never two-colored and always lights entirely or not
/// at all. Pixel slicing (6 px + 2 px), as in the first version,
/// did not guarantee this: the zone boundaries cut a segment in half.
pub const SEG_STEPS: [f32; 5] = [0.5, 0.75, 1.0, 1.5, 3.0];
/// The desired grid step on screen: v1 drew a 6 px segment + 2 px gap.
pub const SEG_TARGET_PX: f32 = 7.0;

/// How many segments on the whole scale at a given step.
pub fn seg_count(seg_db: f32) -> usize {
    ((METER_MAX_DB - METER_MIN_DB) / seg_db).round() as usize
}

/// The grid step for the bar’s height: we take the one from `SEG_STEPS` whose step on screen
/// is closest to `SEG_TARGET_PX`. Otherwise on a tall bar the segments come out
/// huge and rare (complaint of item 1, round 12: “there are very few segments”).
pub fn seg_db_for(bar_h: f32) -> f32 {
    let span = METER_MAX_DB - METER_MIN_DB;
    *SEG_STEPS
        .iter()
        .min_by(|a, b| {
            let px = |s: f32| (bar_h * s / span - SEG_TARGET_PX).abs();
            px(**a).total_cmp(&px(**b))
        })
        .expect("SEG_STEPS is not empty")
}

/// How many segments are lit at level db (pure, test 97).
pub fn led_lit(db: f32, seg_db: f32) -> usize {
    if db <= METER_MIN_DB {
        return 0;
    }
    let n = ((db - METER_MIN_DB) / seg_db).ceil() as i32;
    n.clamp(0, seg_count(seg_db) as i32) as usize
}

/// The middle of segment i (0 — the lowest) in dB: it is used to take the zone color.
pub fn seg_mid_db(i: usize, seg_db: f32) -> f32 {
    METER_MIN_DB + (i as f32 + 0.5) * seg_db
}

/// The dB zone bands (top, bottom) from top to bottom: adjacent, without holes or
/// overlaps, they cover the whole meter range (test 40).
pub(crate) const ZONE_BANDS: [(f32, f32); 6] =
    [(6.0, 0.0), (0.0, -3.0), (-3.0, -9.0), (-9.0, -18.0), (-18.0, -36.0), (-36.0, -48.0)];

/// The dB stops of the vertical glow gradient: the bottom of the meter, the zone boundaries
/// below the level, the level itself. Sorted bottom to top (test 40).
pub(crate) fn glow_stops(level_db: f32) -> Vec<f32> {
    let level = level_db.clamp(METER_MIN_DB, METER_MAX_DB);
    let mut v = vec![METER_MIN_DB];
    for &(top, _bot) in ZONE_BANDS.iter().rev() {
        if top > METER_MIN_DB && top < level {
            v.push(top);
        }
    }
    v.push(level);
    v
}

/// The base alphas of the glow (item 4, round 16 — “tone down the glow”): the zone mesh
/// along the bottom of the bar, the head highlight (two ellipses), the glow of the lit part,
/// the halo of the cap and the overlay to the left of the panel. We keep them in one place so that
/// “make it quieter” does not turn into a hunt through the file.
pub(crate) const GLOW_ZONE_A: u8 = 70;
pub(crate) const GLOW_HEAD_A: (u8, u8) = (68, 44);
pub(crate) const GLOW_LIT_A: u8 = 48;
pub(crate) const GLOW_CAP_A: u8 = 18;
pub(crate) const GLOW_OVERLAY_A: u8 = 40;

/// Glow alpha of the row at a FIXED height of the scale (item 4, round 16).
///
/// Complaint: “the glow flickers along the whole height, which is strange for example for
/// the very lowest segments — they are practically always lit.” The reason
/// was that the alpha AND the colour of the whole column were taken from the CURRENT level:
/// the rows stood at a fraction of the head and were recoloured entirely on crossing a
/// zone. Now a row below the head glows EXACTLY, only the narrow
/// edge (3 dB) at the head depends on the level — that is where the motion should be.
pub fn glow_row_alpha(row_db: f32, level_db: f32, base: u8) -> u8 {
    let below = level_db - row_db;
    if below < -1.0 {
        return 0; // above the head the scale is switched off
    }
    let k = ((below + 1.0) / 3.0).clamp(0.0, 1.0);
    (base as f32 * k).round() as u8
}

/// The halo to the left of the meter panel for the overlay over the UI. The right edge is
/// exactly the left edge of the panel: previously +8 px ran into the first bar by the
/// dB label (item 10 v1.4, test 40).
pub(crate) fn overlay_glow_rect(panel: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(panel.left() - 78.0, panel.top()),
        egui::pos2(panel.left(), panel.bottom()),
    )
}

/// Radial glow (port of drawGlowEllipse: QRadialGradient, Plus).
/// 7 nested ellipses with a Gaussian alpha profile DECREASING outward —
/// a smooth gradient without hard edges (item 7: with three flat rings the
/// alpha grew toward the centre and the edges read as steps).
fn glow_ellipse(
    painter: &egui::Painter,
    center: egui::Pos2,
    radius: egui::Vec2,
    col: egui::Color32,
    alpha: u8,
) {
    // 7 rings instead of 4: at the former alpha the falloff was not noticeable, and on a bright
    // one (item 1, round 13) 4 rings read as concentric “pancakes”
    const RINGS: usize = 7;
    for i in (0..RINGS).rev() {
        let k = (i + 1) as f32 / RINGS as f32; // 1/7 .. 1
        // Gaussian: brightness is higher toward the centre, a soft falloff toward the edge
        let g = (-(k * k) * 2.2f32).exp();
        let a = (alpha as f32 * g * 0.62).min(255.0) as u8;
        if a == 0 {
            continue;
        }
        painter.add(egui::Shape::from(egui::epaint::EllipseShape {
            center,
            radius: radius * k,
            fill: egui::Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), a),
            stroke: egui::Stroke::NONE,
            angle: 0.0,
        }));
    }
}

/// The colour of the cap glow (port of capGlowColor: cream-white base, tinted by zone).
fn cap_glow_color(db: f32) -> egui::Color32 {
    if db > 0.0 {
        egui::Color32::from_rgb(255, 120, 95)
    } else if db >= -3.0 {
        egui::Color32::from_rgb(255, 85, 70)
    } else if db >= -9.0 {
        egui::Color32::from_rgb(255, 205, 60)
    } else if db >= -18.0 {
        egui::Color32::from_rgb(245, 242, 230)
    } else {
        zone_color(db)
    }
}

impl KtApp {
    /// The meter + fader panel to the right of the preview. Returns the height no more than
    /// available; width ~64 px.
    pub fn draw_meters(&mut self, ui: &mut egui::Ui) {
        // 1) level feeding: during play — every frame; on pause — a measurement at the
        // cursor every 120 ms (v1), the target HOLDS the last measurement
        let playing = self.player.transport.is_playing();
        // on pause the meter FADES, but on scrub/seek it shows the level
        // under the cursor (the user’s decision, item 4, round 16): a “fading”
        // meter looked broken
        let ph = self.player.transport.playhead().0;
        if ph != self.meter.last_playhead {
            self.meter.last_playhead = ph;
            self.meter.last_move = std::time::Instant::now();
        }
        let scrubbing = meter_holds_level(playing, self.meter.last_move.elapsed().as_millis());
        let sample = if playing {
            Some(self.player.meters.take())
        } else if scrubbing && self.meter.last_sample.elapsed().as_millis() >= 120 {
            self.meter.last_sample = std::time::Instant::now();
            Some(self.player.sample_levels_at(self.player.transport.playhead()))
        } else {
            None // between measurements we do not touch the target (previously zeros went here)
        };
        if let Some((pl, pr)) = sample {
            self.meter.feed(pl, pr);
        }
        // KT_METER_DEMO=1 — feed the meter fictitious levels: the look of the meter
        // (glow, zones, peak) otherwise cannot be checked without the sound hardware
        if std::env::var_os("KT_METER_DEMO").is_some() {
            let t = self.meter.last_sample.elapsed().as_secs_f32();
            self.meter.feed(0.75, 0.42 + 0.2 * (t * 3.0).sin().abs());
        }
        // 2) decay to the target and peak-hold (tick 25 ms, v1 decays) — pure logic
        // in MeterState::tick (test 38)
        let ticks = (self.meter.last_tick.elapsed().as_millis() / 25).min(40) as f32;
        if ticks >= 1.0 {
            self.meter.last_tick = std::time::Instant::now();
            // the target is released on pause too — but only when the cursor stands still:
            // while scrubbing it holds the measurement under the cursor
            self.meter.tick(ticks, playing || !scrubbing);
        } else {
            // clamps to the target — and between ticks (feeding above the decay)
            self.meter.tick(0.0, false);
        }

        // item 3, round 9: we occupy the WHOLE width of the panel — fixed 64 px with a panel of
        // 70 left a dead gap at the right edge of the window (and it grew if the
        // panel was widened). The inner geometry below is relative.
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), ui.available_height()),
            egui::Sense::click_and_drag(),
        );
        {
            // the halo for the overlay over the UI (item 8)
            let loud = self.meter.bar_db[0].max(self.meter.bar_db[1]);
            let k = ((loud - METER_MIN_DB) / (METER_MAX_DB - METER_MIN_DB)).clamp(0.0, 1.0);
            // (panel, intensity, dB level) — the overlay paints by zones
            // a threshold by LEVEL, not by overall brightness: so that the halo does not
            // flare up/fade out entirely on quiet material (item 4, round 16)
            self.meter_glow = (loud > METER_MIN_DB + 0.5).then_some((rect, k, loud));
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, egui::Color32::from_gray(16));

        // dB label at the top
        let label = if self.meter.muted {
            "MUTE".to_string()
        } else if self.meter.fader_db.abs() < 10.0 {
            format!("{:+.1} dB", self.meter.fader_db)
        } else {
            format!("{:+.0} dB", self.meter.fader_db)
        };
        painter.text(
            egui::pos2(rect.center().x, rect.top() + 4.0),
            egui::Align2::CENTER_TOP,
            label,
            egui::FontId::proportional(11.0),
            if self.meter.muted {
                egui::Color32::from_rgb(255, 90, 90)
            } else {
                egui::Color32::from_gray(210)
            },
        );

        let bars = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 5.0, rect.top() + 22.0),
            egui::pos2(rect.right() - 16.0, rect.bottom() - 30.0),
        );
        let db_to_y = |db: f32| -> f32 {
            let t = (db - METER_MIN_DB) / (METER_MAX_DB - METER_MIN_DB);
            bars.bottom() - t.clamp(0.0, 1.0) * bars.height()
        };
        // the LED grid step matched to the actual height of the bar (item 1, round 12)
        let seg_db = seg_db_for(bars.height());
        let segs = seg_count(seg_db);
        let seg_h = bars.height() / segs as f32;
        // two bars with a gap in the middle — the fader lives in the gap (port of the old one)
        let gap = 10.0;
        let bar_w = (bars.width() - gap) / 2.0;
        let bar_x = [bars.left(), bars.left() + bar_w + gap];

        let loud_db = self.meter.bar_db[0].max(self.meter.bar_db[1]);

        // Layer order — as in the “beautiful” version before round 11 (item 1, round 13):
        // underlay → glow → LED → highlight and peak ON TOP. In round 11 the underlay
        // moved on top of the glow and painted it over entirely, and the highlight went under the
        // opaque segments — the meter became flat and “fake”.
        let lit_n: [usize; 2] =
            [led_lit(self.meter.bar_db[0], seg_db), led_lit(self.meter.bar_db[1], seg_db)];
        let cap_seg: [Option<usize>; 2] = std::array::from_fn(|ch| {
            let cap = self.meter.cap_db[ch];
            (cap > self.meter.bar_db[ch] + 0.4 && cap > METER_MIN_DB + 1.0)
                .then(|| led_lit(cap, seg_db).saturating_sub(1))
        });
        let back = egui::Color32::from_rgb(8, 9, 11);
        // 1) the bottoms of the bars
        for &x0 in &bar_x {
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x0 - 1.0, bars.top() - 1.0),
                    egui::pos2(x0 + bar_w + 1.0, bars.bottom() + 1.0),
                ),
                2.0,
                back,
            );
        }

        // 2) the zone glow along the bottom of the bar (the gaps between the LEDs glow)
        if loud_db > METER_MIN_DB + 1.0 {
            let cx = bars.center().x;
            let half_w = bars.width() * 1.4;
            let level_y = db_to_y(loud_db);
            let mut mesh = egui::Mesh::default();
            let stops = glow_stops(loud_db);
            let push_row = |mesh: &mut egui::Mesh, y: f32, col: egui::Color32, a: u8| {
                for (x, alpha) in [(cx - half_w, 0u8), (cx, a), (cx + half_w, 0u8)] {
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: egui::pos2(x, y),
                        uv: egui::epaint::WHITE_UV,
                        color: egui::Color32::from_rgba_unmultiplied(
                            col.r(),
                            col.g(),
                            col.b(),
                            alpha,
                        ),
                    });
                }
            };
            for &db in &stops {
                push_row(
                    &mut mesh,
                    db_to_y(db),
                    zone_color(db),
                    glow_row_alpha(db, loud_db, GLOW_ZONE_A),
                );
            }
            // the tail is SHORT (8 px): the former 22 px made the unlit
            // part “half-lit” — that was the real cause of the complaint of item 8, round 11,
            // and not the partial fill of the segments at all
            push_row(&mut mesh, (level_y - 8.0).max(bars.top()), zone_color(loud_db), 0);
            let rows = stops.len() + 1;
            for r in 0..rows - 1 {
                let b = (r * 3) as u32;
                for c in 0..2u32 {
                    mesh.indices.extend_from_slice(&[
                        b + c,
                        b + c + 1,
                        b + c + 4,
                        b + c,
                        b + c + 4,
                        b + c + 3,
                    ]);
                }
            }
            painter.add(egui::Shape::mesh(mesh));
            glow_ellipse(
                &painter,
                egui::pos2(cx, bars.bottom()),
                egui::vec2(half_w * 0.85, 30.0),
                zone_color(METER_MIN_DB + 6.0),
                30,
            );
        }

        // 3) above the level of EACH channel the bar is opaque: the glow of the neighbouring
        // (louder) channel must not light the unlit scale
        for (ch, &x0) in bar_x.iter().enumerate() {
            let top_lit = db_to_y(METER_MIN_DB + lit_n[ch] as f32 * seg_db);
            if top_lit > bars.top() {
                painter.rect_filled(
                    egui::Rect::from_min_max(
                        egui::pos2(x0 - 1.0, bars.top() - 1.0),
                        egui::pos2(x0 + bar_w + 1.0, top_lit),
                    ),
                    0.0,
                    back,
                );
            }
        }

        // 4) LED segments: duty cycle ~55 %, as in v1 (there were 4 px
        // drawn on an 8 px step) — in rounds 11–12 it reached 82 %, which made the
        // scale look like a solid “sausage with seams”
        for (ch, &x0) in bar_x.iter().enumerate() {
            let gap_y = (seg_h * 0.38).clamp(1.0, 2.5);
            for i in 0..segs {
                let y_bot = db_to_y(METER_MIN_DB + i as f32 * seg_db);
                let y_top = db_to_y(METER_MIN_DB + (i + 1) as f32 * seg_db);
                let seg = egui::Rect::from_min_max(
                    egui::pos2(x0, y_top),
                    egui::pos2(x0 + bar_w, y_bot - gap_y),
                )
                .shrink2(egui::vec2(0.5, 0.0));
                let lit = i < lit_n[ch];
                let col = if lit {
                    zone_color(seg_mid_db(i, seg_db))
                } else {
                    egui::Color32::from_rgb(13, 15, 18)
                };
                painter.rect_filled(seg, 1.0, col);
                if lit {
                    painter.line_segment(
                        [
                            egui::pos2(seg.left() + 1.0, seg.top() + 0.5),
                            egui::pos2(seg.right() - 1.0, seg.top() + 0.5),
                        ],
                        egui::Stroke::new(1.0, col.gamma_multiply(1.12)),
                    );
                }
            }
            // the 0 dB line across the bar (port: 95, 105, 118, 140)
            let zero_y = db_to_y(0.0);
            painter.line_segment(
                [egui::pos2(x0, zero_y), egui::pos2(x0 + bar_w, zero_y)],
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(95, 105, 118, 140)),
            );
        }

        // 5) the head highlight ON TOP of the lit LEDs (the same bloom as in v1)
        if loud_db > METER_MIN_DB + 1.0 {
            let cx = bars.center().x;
            let wide = bars.width() * 1.15;
            let level_y = db_to_y(loud_db);
            glow_ellipse(
                &painter,
                egui::pos2(cx, level_y),
                egui::vec2(wide * 1.15, 38.0),
                zone_color(loud_db),
                GLOW_HEAD_A.0,
            );
            glow_ellipse(
                &painter,
                egui::pos2(cx, level_y - 12.0),
                egui::vec2(wide * 0.95, 32.0),
                zone_color(loud_db),
                GLOW_HEAD_A.1,
            );
        }

        // 5b) the glow of the LIT part of the bar itself: without it the LEDs are flat,
        // and the unlit part is not lit — the mesh breaks exactly at
        // the channel level (item 1, round 13: “the glow is very weak”)
        for (ch, &x0) in bar_x.iter().enumerate() {
            if lit_n[ch] == 0 {
                continue;
            }
            let top_lit = db_to_y(METER_MIN_DB + lit_n[ch] as f32 * seg_db);
            let level = self.meter.bar_db[ch];
            let mut mesh = egui::Mesh::default();
            let cx = x0 + bar_w * 0.5;
            let half = bar_w * 1.25;
            let mut row = |y: f32, col: egui::Color32, a: u8| {
                for (x, k) in [(cx - half, 0u8), (cx, a), (cx + half, 0u8)] {
                    mesh.vertices.push(egui::epaint::Vertex {
                        pos: egui::pos2(x, y),
                        uv: egui::epaint::WHITE_UV,
                        color: egui::Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), k),
                    });
                }
            };
            // The rows stand at FIXED dB stops, the colour and alpha — from
            // THEIR OWN dB (item 4, round 16). Previously the middle row stood at a fraction of the
            // head, and the colour of the whole column was taken from the current level —
            // the column flickered entirely, including the lower segments, which
            // are practically always lit.
            let stops = glow_stops(level);
            for &db in &stops {
                row(db_to_y(db), zone_color(db), glow_row_alpha(db, level, GLOW_LIT_A));
            }
            row((top_lit - 10.0).max(bars.top()), zone_color(level), 0);
            let rows = stops.len() + 1;
            for r in 0..rows as u32 - 1 {
                let b = r * 3;
                for c in 0..2u32 {
                    mesh.indices.extend_from_slice(&[
                        b + c,
                        b + c + 1,
                        b + c + 4,
                        b + c,
                        b + c + 4,
                        b + c + 3,
                    ]);
                }
            }
            painter.add(egui::Shape::mesh(mesh));
        }

        // 6) peak-hold: a cream LINE along the top edge of its segment
        // (the LED grid is not broken) + halo — ON TOP of the segments
        for (ch, &x0) in bar_x.iter().enumerate() {
            let Some(seg_i) = cap_seg[ch] else { continue };
            let cap_db = self.meter.cap_db[ch];
            let col = cap_glow_color(cap_db);
            let cap_y = db_to_y(METER_MIN_DB + (seg_i + 1) as f32 * seg_db);
            glow_ellipse(
                &painter,
                egui::pos2(x0 + bar_w / 2.0, cap_y),
                egui::vec2(bar_w * 2.0, 13.0),
                col,
                GLOW_CAP_A,
            );
            painter.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x0, cap_y),
                    egui::pos2(x0 + bar_w, cap_y + 2.5),
                ),
                1.0,
                col,
            );
        }

        // scale 0/-12/-24/-48
        for mark in [0.0f32, -12.0, -24.0, -48.0] {
            let y = db_to_y(mark);
            painter.text(
                egui::pos2(rect.right() - 2.0, y),
                egui::Align2::RIGHT_CENTER,
                format!("{mark:.0}"),
                egui::FontId::proportional(8.5),
                egui::Color32::from_gray(120),
            );
        }

        // --- the fader BETWEEN the bars (port of PreviewVolumeFader as the central
        //     overlay): the track in the gap + a wide knob across both bars
        let track = egui::Rect::from_min_max(
            egui::pos2(bars.left() + bar_w + 2.5, bars.top()),
            egui::pos2(bars.left() + bar_w + gap - 2.5, bars.bottom()),
        );
        painter.rect_filled(track, 2.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 120));
        let ft = (self.meter.fader_db - FADER_MIN_DB) / (FADER_MAX_DB - FADER_MIN_DB);
        let hy = bars.bottom() - ft.clamp(0.0, 1.0) * bars.height();
        // The v1 knob (PreviewVolumeFader): vertical gradient, notches,
        // central groove, shadow below, dark outline.
        let handle = egui::Rect::from_center_size(
            egui::pos2(bars.center().x, hy),
            egui::vec2(bars.width() + 6.0, 14.0),
        );
        let (g_top, g_mid, g_bot) = if self.meter.muted {
            (
                egui::Color32::from_rgb(210, 90, 90),
                egui::Color32::from_rgb(172, 52, 52),
                egui::Color32::from_rgb(110, 32, 32),
            )
        } else {
            (
                egui::Color32::from_rgb(128, 132, 142),
                egui::Color32::from_rgb(78, 82, 90),
                egui::Color32::from_rgb(48, 50, 56),
            )
        };
        // shadow under the knob
        painter.rect_filled(
            handle.translate(egui::vec2(0.0, 2.0)),
            3.0,
            egui::Color32::from_rgba_unmultiplied(0, 0, 0, 70),
        );
        // 3-stop gradient in two meshes (stop 0.45 as in v1)
        let mid_y = handle.top() + handle.height() * 0.45;
        crate::toolbar::gradient_rect(
            &painter,
            egui::Rect::from_min_max(handle.left_top(), egui::pos2(handle.right(), mid_y)),
            g_top,
            g_mid,
        );
        crate::toolbar::gradient_rect(
            &painter,
            egui::Rect::from_min_max(egui::pos2(handle.left(), mid_y), handle.right_bottom()),
            g_mid,
            g_bot,
        );
        painter.rect(
            handle,
            3.0,
            egui::Color32::TRANSPARENT,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(12, 14, 18)),
            egui::StrokeKind::Outside,
        );
        // notches every 4 px (white, alpha 40)
        let mut nx = handle.left() + 7.0;
        while nx <= handle.right() - 7.0 {
            painter.line_segment(
                [egui::pos2(nx, handle.top() + 3.0), egui::pos2(nx, handle.bottom() - 3.0)],
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 40)),
            );
            nx += 4.0;
        }
        // central groove
        painter.line_segment(
            [
                egui::pos2(handle.left() + 3.0, handle.center().y),
                egui::pos2(handle.right() - 3.0, handle.center().y),
            ],
            egui::Stroke::new(2.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 45)),
        );

        // button M (mute) at the bottom
        let mute_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left() + 6.0, rect.bottom() - 26.0),
            egui::pos2(rect.right() - 6.0, rect.bottom() - 4.0),
        );
        painter.rect_filled(
            mute_rect,
            3.0,
            if self.meter.muted {
                egui::Color32::from_rgb(160, 40, 40)
            } else {
                egui::Color32::from_gray(45)
            },
        );
        painter.text(
            mute_rect.center(),
            egui::Align2::CENTER_CENTER,
            "M",
            egui::FontId::proportional(12.0),
            egui::Color32::WHITE,
        );

        // Fader interactions (item 2, round 8). The hit zone is WIDER than the widget (the drawing
        // does not change): previously one had to aim at a narrow strip, and
        // any miss teleported the volume to the cursor.
        let hit = egui::Rect::from_x_y_ranges(
            rect.x_range(),
            egui::Rangef::new(bars.top() - 10.0, bars.bottom() + 10.0),
        );
        let fader = ui.interact(hit, ui.id().with("fader"), egui::Sense::click_and_drag());
        let db_at = |y: f32| {
            let t = ((bars.bottom() - y) / bars.height()).clamp(0.0, 1.0);
            (FADER_MIN_DB + t * (FADER_MAX_DB - FADER_MIN_DB)).clamp(FADER_MIN_DB, FADER_MAX_DB)
        };
        if response.clicked()
            && response.interact_pointer_pos().is_some_and(|p| mute_rect.contains(p))
        {
            self.meter.muted = !self.meter.muted;
        } else if fader.double_clicked() {
            self.meter.fader_target = 0.0;
        } else if let Some(pos) = fader.interact_pointer_pos() {
            if fader.drag_started() {
                // pressed next to the knob — we drag RELATIVELY (without a jerk);
                // pressed on the track — we move toward the point
                self.fader_grab = if (pos.y - hy).abs() <= 12.0 { Some(hy - pos.y) } else { None };
                if self.fader_grab.is_none() {
                    self.meter.fader_target = db_at(pos.y);
                }
            }
            if fader.dragged() {
                let y = pos.y + self.fader_grab.unwrap_or(0.0);
                self.meter.fader_target = db_at(y);
            }
            if fader.clicked() && !mute_rect.contains(pos) {
                self.meter.fader_target = db_at(pos.y);
            }
        }
        // Right click on the meters (R.49, item 365). The panel responded with hints
        // to hovering and was SILENT to the right button, although all three of its actions
        // already exist — only they are hidden in the M button, the double click on the fader and
        // the peak decay. A menu on both the scales and the fader: “paired ways behave
        // the same” (rule 17). No new keys.
        {
            let muted = self.meter.muted;
            let mut act: Option<u8> = None;
            let mut picked = false;
            let menu = |ui: &mut egui::Ui, act: &mut Option<u8>| {
                if ui.button(tr(k::meter::MUTE)).clicked() {
                    *act = Some(0);
                    ui.close();
                }
                if ui.button(tr(k::meter::FADER)).clicked() {
                    *act = Some(1);
                    ui.close();
                }
                if ui.button(tr(k::meter::PEAK)).clicked() {
                    *act = Some(2);
                    ui.close();
                }
            };
            response.clone().context_menu(|ui| {
                picked = true;
                menu(ui, &mut act);
            });
            fader.clone().context_menu(|ui| {
                picked = true;
                menu(ui, &mut act);
            });
            if picked {
                self.set_panel_target(crate::ctx_menu::PanelTarget::Meters);
            }
            match act {
                Some(0) => self.meter.muted = !muted,
                Some(1) => self.meter.fader_target = 0.0,
                // reset of the held peak: the “ribbons” move back to the current level
                Some(2) => self.meter.cap_db = self.meter.bar_db,
                _ => {}
            }
        }
        if fader.drag_stopped() {
            self.fader_grab = None;
        }
        // wheel over the meter: step 1 dB, with Shift — a precise 0.2 dB
        if fader.contains_pointer() {
            let (scroll, shift) = ui.input(|i| (i.smooth_scroll_delta.y, i.modifiers.shift));
            if scroll != 0.0 {
                let step = if shift { 0.2 } else { 1.0 };
                self.meter.fader_target = (self.meter.fader_target + scroll.signum() * step)
                    .clamp(FADER_MIN_DB, FADER_MAX_DB);
            }
        }
        // inertia: the fader catches up with the target between frames
        let dt = ui.input(|i| i.stable_dt);
        self.meter.fader_tick(dt);
        let gain = if self.meter.muted || self.meter.fader_db <= FADER_MIN_DB + 0.01 {
            0.0
        } else {
            10f32.powf(self.meter.fader_db / 20.0)
        };
        self.player.set_master_gain(gain);
        // the hint depends on the zone under the cursor (item 1, round 9)
        let over_mute = fader.hover_pos().is_some_and(|p| mute_rect.contains(p));
        let text = if over_mute { tr(k::meter::MUTE) } else { tr(k::meter::FADER) };
        self.hint(fader, text);
        self.hint(response, tr(k::meter::PEAK));
    }
}

#[cfg(test)]
mod repaint_tests {
    use super::*;

    /// Test 84 (item 7, round 9): the fader must withstand ANY dt from the clamp.
    /// The former scheme (explicit Euler) diverged already at 30 fps, and after an idle
    /// dt comes in equal to 0.1 — the fader flew off by over 400 dB and this
    /// turned into a gain of 10^20 at the engine output.
    #[test]
    fn t84_fader_never_diverges_on_bad_dt() {
        let series: Vec<Vec<f32>> = vec![
            vec![0.1; 60],
            vec![1.0 / 30.0; 300],
            vec![0.05; 200],
            // alternation — the worst case for explicit schemes
            (0..300).map(|i| if i % 2 == 0 { 0.1 } else { 1.0 / 240.0 }).collect(),
        ];
        for dts in series {
            let mut m = MeterState { fader_target: -20.0, ..Default::default() };
            for dt in &dts {
                m.fader_tick(*dt);
                assert!(m.fader_db.is_finite() && m.fader_vel.is_finite(), "NaN/inf at dt={dt}");
                assert!(
                    (FADER_MIN_DB..=FADER_MAX_DB).contains(&m.fader_db),
                    "fader flew away: {} dB at dt={dt}",
                    m.fader_db
                );
            }
            assert!((m.fader_db + 20.0).abs() < 0.2, "did not converge: {}", m.fader_db);
        }
    }

    /// Test 80 (item 2, round 8): the fader’s inertia — converges to the target in a reasonable
    /// time and STOPS (otherwise an eternal repaint, for which
    /// t38 was written).
    #[test]
    fn t80_fader_inertia_settles() {
        let mut m = MeterState { fader_target: -20.0, ..Default::default() };
        assert!(m.is_active(), "it moved — a redraw is needed");
        let mut steps = 0;
        while m.is_active() && steps < 400 {
            m.fader_tick(1.0 / 60.0);
            steps += 1;
        }
        assert!(steps < 120, "fader moves too long: {steps} frames");
        assert_eq!(m.fader_db, -20.0, "did not settle exactly on the target");
        assert_eq!(m.fader_vel, 0.0);
        assert!(!m.is_active(), "after convergence the redraw is released");
        // without a target nothing happens
        m.fader_tick(1.0 / 60.0);
        assert_eq!(m.fader_db, -20.0);
        // movement toward the target is monotonic (no overshoot): critical damping
        m.fader_target = 0.0;
        let mut prev = m.fader_db;
        for _ in 0..200 {
            m.fader_tick(1.0 / 60.0);
            assert!(m.fader_db >= prev - 1e-3, "overshoot down");
            assert!(m.fader_db <= 0.0 + 1e-3, "overshoot up");
            prev = m.fader_db;
        }
    }

    /// Test 40 (items 8–10 v1.4): the glow layout. The zone bands are adjacent, without
    /// holes or overlaps, they cover the whole range of the meter; the gradient stops are
    /// sorted and include all zone boundaries below the level; the halo of the overlay
    /// does not reach the panel (previously +8 px ran into the first bar).
    #[test]
    fn t40_glow_layout() {
        // zones: top to bottom, the top of the first = MAX, the bottom of the last = MIN, butted edge to edge
        assert_eq!(ZONE_BANDS[0].0, METER_MAX_DB);
        assert_eq!(ZONE_BANDS[ZONE_BANDS.len() - 1].1, METER_MIN_DB);
        for w in ZONE_BANDS.windows(2) {
            assert_eq!(w[0].1, w[1].0, "bars join edge to edge: {w:?}");
        }
        for (top, bot) in ZONE_BANDS {
            assert!(top > bot, "bar is degenerate: ({top}, {bot})");
        }
        // gradient stops: bottom, zone boundaries below the level, the level
        for level in [-40.0f32, -20.0, -10.0, -5.0, -1.0, 3.0, 99.0] {
            let stops = glow_stops(level);
            assert_eq!(stops[0], METER_MIN_DB);
            let clamped = level.clamp(METER_MIN_DB, METER_MAX_DB);
            assert_eq!(*stops.last().unwrap(), clamped);
            assert!(stops.windows(2).all(|w| w[0] < w[1]), "strictly up: {stops:?}");
            for &(top, _bot) in &ZONE_BANDS {
                if top > METER_MIN_DB && top < clamped {
                    assert!(stops.contains(&top), "zone boundary {top} in stops {stops:?}");
                }
            }
        }
        // overlay halo: right edge = the left edge of the panel (no further)
        let panel = egui::Rect::from_min_max(egui::pos2(100.0, 0.0), egui::pos2(164.0, 400.0));
        let g = overlay_glow_rect(panel);
        assert!(g.right() <= panel.left(), "the glow does not reach the panel");
        assert!(g.left() < panel.left(), "and it really shines to the left");
    }

    /// Test 121 (item 4, round 16): the glow does not flicker along the whole height, and on
    /// pause the meter FADES (showing the level under the cursor only when
    /// scrubbing/seeking).
    ///
    /// Complaint: “tone down the glow of the sound monitor, and also — it has a glow
    /// that flickers along the whole height, which is strange, for example, for the lowest
    /// segments — they are practically always lit. And also — on pause
    /// for some reason the monitor does not fade but freezes.”
    #[test]
    fn t121_glow_is_steady_and_pause_fades() {
        // --- the row below the head glows the SAME at any level above it ---
        for row in [-46.0f32, -36.0, -24.0, -12.0] {
            let a = glow_row_alpha(row, row + 3.0, GLOW_LIT_A);
            for level in [row + 3.0, row + 9.0, row + 20.0, METER_MAX_DB] {
                assert_eq!(
                    glow_row_alpha(row, level, GLOW_LIT_A),
                    a,
                    "row {row} dB flickers after the level {level} dB"
                );
            }
            assert_eq!(a, GLOW_LIT_A, "the row under the head must shine at full base");
        }
        // above the head — dark; at the head itself a soft edge, not a step
        assert_eq!(glow_row_alpha(0.0, -12.0, GLOW_LIT_A), 0, "the dimmed part shines");
        let edge = glow_row_alpha(-12.0, -12.0, GLOW_LIT_A);
        assert!(edge > 0 && edge < GLOW_LIT_A, "the edge at the head must be soft: {edge}");
        // “tone down”: the glow bases are lower than before (96/96/62/40/22/58)
        const {
            assert!(GLOW_ZONE_A < 96 && GLOW_HEAD_A.0 < 96 && GLOW_HEAD_A.1 < 62);
            assert!(GLOW_LIT_A < 70 && GLOW_CAP_A < 22 && GLOW_OVERLAY_A < 58);
        }

        // --- pause: the cursor stands still → the measurement is released; the cursor is moved →
        // we hold the level under it
        assert!(!meter_holds_level(true, 0), "during playback there is nothing to hold the measurement on");
        assert!(meter_holds_level(false, 100), "scrubbing must show the level under the cursor");
        assert!(
            !meter_holds_level(false, SCRUB_SHOW_MS + 1),
            "a still pause must switch off the meter"
        );
        // and the decay itself reaches the floor and RELEASES the redraw (otherwise t38)
        let mut m = MeterState::default();
        m.feed(0.9, 0.9);
        m.tick(1.0, false);
        assert!(m.bar_db[0] > METER_MIN_DB + 1.0, "level did not show up");
        for _ in 0..2000 {
            m.tick(1.0, true);
        }
        assert!((m.bar_db[0] - METER_MIN_DB).abs() < 0.1, "in pause the meter did not go out: {:?}", m.bar_db);
        assert!(!m.is_active(), "a meter that went out must release the redraw");
    }

    /// Test 38 (item 1 v1.4): the meters do not hold 60 fps forever. After feeding stops
    /// (pause, static target) the bars/caps converge and
    /// is_active() goes out; during motion — it is lit. Previously the condition
    /// “bar above the minimum” stuck at true forever after the first sound.
    #[test]
    fn t38_meters_settle_and_release_repaint() {
        let mut m = MeterState::default();
        assert!(!m.is_active(), "silence from zero — inactive");

        // a loud measurement during play: motion has started
        m.feed(0.9, 0.9);
        m.tick(1.0, true);
        assert!(m.is_active(), "the bar moves toward the target — active");

        // pause: the target holds the level at the cursor (feature, item 2 v1.1) — the bars
        // converge to it, the caps decay down to the bars, the motion ends
        m.feed(0.5, 0.5);
        for _ in 0..2000 {
            m.tick(1.0, false);
        }
        assert!(
            !m.is_active(),
            "a static level display in pause must not require animation: \
             bar={:?} cap={:?} target={:?}",
            m.bar_db,
            m.cap_db,
            m.target_db
        );
        // at that the level IS SHOWN (not faded to the minimum — the feature)
        assert!(m.bar_db[0] > METER_MIN_DB + 1.0, "the level under the cursor is visible");

        // play without signal: the target is released, everything fades to the minimum
        m.feed(0.0, 0.0);
        for _ in 0..2000 {
            m.tick(1.0, true);
        }
        assert!(!m.is_active());
        assert!((m.bar_db[0] - METER_MIN_DB).abs() < 0.1, "after playback they faded to the floor");
    }
}
