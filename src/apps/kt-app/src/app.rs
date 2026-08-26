//! KtApp: состояние приложения, панели, клавиши, дропы, lifecycle.

use crate::audio_out;
use crate::timeline::{CtxTarget, Drag};
use crate::util::*;
use eframe::egui;
use kt_engine::Player;
use kt_model::*;
use kt_time::Tick;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// Важность уведомления (п.18).
#[derive(Clone, Copy, PartialEq)]
pub enum Severity {
    Info,
    Warn,
}

pub struct Notice {
    pub at: String,
    pub severity: Severity,
    pub text: String,
}

pub struct KtApp {
    pub model: TimelineModel,
    pub project_path: Option<PathBuf>,
    pub dirty: bool,
    pub player: Player,
    /// Keep-alive хэндл cpal-стрима (Drop = остановка вывода).
    #[allow(dead_code)]
    pub audio: Option<audio_out::AudioOutput>,
    pub peaks: Arc<Mutex<HashMap<MediaId, Peaks>>>,
    pub texture: Option<egui::TextureHandle>,
    pub view_start: f64, // тики
    pub ticks_per_px: f64,
    pub selected: Option<ClipId>,
    /// Выделенный morph-блок (H1; приоритет над клипом при Del).
    pub selected_morph: Option<MorphId>,
    /// Режим размещения morpher'а (порт pendingMorpherPlacement v1):
    /// замороженные цели + вид; Esc — отмена.
    pub pending_morpher: Option<PendingMorph>,
    pub drag: Option<Drag>,
    /// Основная (зафиксированная) скорость воспроизведения (пп.4-5 v1.4).
    pub base_rate: f32,
    /// Позиция shuttle-слайдера, пока его держат (None = не держат).
    pub shuttle_override: Option<f32>,
    /// Какая полоса спид-контрола схвачена драгом: Some(true)=верхняя
    /// (постоянная), Some(false)=нижняя (shuttle). Пп.18-19 р.7.
    pub speed_drag_upper: Option<bool>,
    /// Отложенный одиночный клик по превью (п.11 р.7): ждём окно
    /// double-click, потом play/pause.
    pub preview_click_at: Option<std::time::Instant>,
    /// Позиция мыши во время DnD файла (X11-опрос; None на Wayland).
    pub dnd_hover: Option<egui::Pos2>,
    /// Кэш геометрии таймлайна последнего кадра (дроп, п.2 v1.4).
    pub tl_geom: Option<crate::timeline::TlGeom>,
    /// Display-порядок дорожек (п.6 v1.4); нормализуется каждый кадр,
    /// сохраняется в UiState проекта.
    pub lane_order: Vec<(LaneKind, u32)>,
    /// Превью отделено в собственное окно (п.7 v1.4, второй монитор).
    pub preview_detached: bool,
    /// Экономный UI: 30 fps при воспроизведении (п.1 раунда 5, слабые GPU).
    pub ui_economy: bool,
    /// Момент последней паузы (двойной Space = стоп, п.15 раунда 6).
    pub paused_at: Option<std::time::Instant>,
    /// Окно следует за курсором при игре; гаснет при ручном пане/зуме
    /// (п.14 раунда 6) — используется в L2, ставится в on_play.
    pub follow_playhead: bool,
    /// Момент последнего РУЧНОГО скролла/зума (п.30 р.7: grace для follow_rearm).
    pub last_user_scroll: Option<std::time::Instant>,
    /// Драг окна начат из развёрнутого состояния: сначала восстановить
    /// окно, StartDrag — СЛЕДУЮЩИМ кадром (п.10 р.8, X11).
    pub pending_unmax_drag: bool,
    /// Вертикальный скролл зоны дорожек, px (п.12 р.6).
    pub tl_scroll: f32,
    /// Явная высота строки (Shift+Ctrl+колесо); None = авто-подгонка.
    pub tl_v_zoom: Option<f32>,
    /// Последний удачный бакет превью в бине (скраб без миганий, п.11 р.6).
    pub bin_last_good: HashMap<MediaId, crate::thumbs::ThumbKey>,
    /// Прошлый offline-снапшот бина: детект offline→online (п.1 р.7).
    pub bin_last_offline: std::collections::HashSet<kt_model::MediaId>,
    /// Медиа-бин: свободная раскладка (п.7 р.8) вместо сетки.
    pub bin_free: bool,
    /// Позиции карточек свободной раскладки (живут в проекте, UiState).
    pub bin_layout: std::collections::HashMap<kt_model::MediaId, egui::Pos2>,
    /// Какую карточку тащим внутри бина (свободный режим).
    pub bin_drag: Option<(kt_model::MediaId, egui::Vec2)>,
    /// Захват ручки фейдера: смещение курсора относительно ручки —
    /// тянем «относительно», без рывка к позиции (п.2 р.8).
    pub fader_grab: Option<f32>,
    /// Последний отправленный заголовок окна (шлём Title только при смене).
    pub(crate) last_title: String,
    /// Переиспользуемый буфер композиции превью (без аллокаций на кадр).
    preview_buf: Vec<u8>,
    /// Троттлинг glow-мипа: каждые N новых кадров (амбиент не мерцает).
    glow_skip: u8,
    /// Имя адаптера/бэкенда wgpu (диагностика перф-панели).
    pub gpu_info: Option<String>,
    pub snap_line: Option<i64>,
    pub ctx_target: Option<CtxTarget>,
    /// Live-громкость volume rail до отпускания (id, dB).
    pub live_gain: Option<(ClipId, f32)>,
    pub status: Option<(String, std::time::Instant)>,
    /// Внутренний буфер обмена (блок клипов, ребейзнутых к нулю).
    pub clipboard: Vec<ClipEvent>,
    /// Буфер огибающей автоматизации (п.5 v1.3, порт «Copy envelopes»).
    pub automation_clipboard: Option<Vec<SlowedKeyframe>>,
    /// Медиа, чьи файлы сейчас недоступны (watchdog, 900 мс).
    pub offline: Arc<Mutex<std::collections::HashSet<MediaId>>>,
    /// Drop останавливает watchdog-поток (переоткрытие без утечек).
    #[allow(dead_code)]
    pub watchdog: Option<crate::util::WatchdogHandle>,
    pub meter: crate::meters::MeterState,
    pub smooth_scaling: bool,
    pub fx_editors: Vec<crate::fx_editor::FxEditor>,
    pub morph_editors: Vec<crate::morpher_editor::MorphEditor>,
    pub gesture_counter: u64,
    pub render_job: Option<crate::render_dialog::RenderJob>,
    pub render_setup: crate::render_dialog::RenderSetup,
    pub last_autosave: std::time::Instant,
    /// Хинт наведённой кнопки тулбара — дублируется в статусбар (порт).
    pub hover_hint: Option<String>,
    /// Rect таймлайна прошлого кадра (для плавающего тулбара).
    pub timeline_rect: egui::Rect,
    /// Ретроспектива уведомлений (п.18): кольцо + счётчик непрочитанных,
    /// окно открывается колокольчиком в статусбаре.
    pub notices: std::collections::VecDeque<Notice>,
    pub notices_unread: usize,
    pub notices_open: bool,
    /// Софт-рендер (llvmpipe/Cpu) — детект один раз (пп. 17, 19).
    pub sw_render: Option<bool>,
    pub perf: PerfState,
    /// Ключ последнего ЗАГРУЖЕННОГО в текстуру кадра превью: (playhead-тик,
    /// качество, полный ли). Текстура обновляется только при смене —
    /// раньше кадр заливался в GPU 60 раз/с даже в паузе (п.1).
    pub preview_key: Option<(i64, (u32, u32), bool)>,
    /// Когда превью впервые стало неполным (скрим «Загрузка» — после 180 мс,
    /// как в v1, чтобы не мигал на быстрых промахах; п.14).
    pub incomplete_since: Option<std::time::Instant>,
    /// Последняя попытка композита неполного кадра (троттлинг ретраев, п.1).
    pub last_compose_retry: std::time::Instant,
    /// Размытая мини-копия кадра для ambient glow (п.18): 32×18,
    /// обновляется вместе с кадром, рисуется LINEAR-растяжкой.
    pub glow_texture: Option<egui::TextureHandle>,
    /// Rect превью прошлого кадра (для оверлей-свечения поверх UI, п.8).
    pub preview_rect: egui::Rect,
    /// Rect метров прошлого кадра + цвет доминирующей зоны.
    /// (панель метров, интенсивность 0..1, уровень dB) — оверлей красит
    /// ореол по ЗОНАМ высоты (п.8 v1.4), не одним цветом.
    pub meter_glow: Option<(egui::Rect, f32, f32)>,
    pub thumbs: crate::thumbs::ThumbService,
    /// Текстуры филмстрипа (LRU держит ThumbService, тут egui-хэндлы).
    pub thumb_textures: HashMap<crate::thumbs::ThumbKey, egui::TextureHandle>,
    /// Порядок вставки текстур — LRU-вытеснение вместо clear() (п.2).
    pub thumb_lru: std::collections::VecDeque<crate::thumbs::ThumbKey>,
    /// Версия модели (бамп в apply/undo/redo) — ключ кэшей отрисовки (п.1).
    pub model_rev: u64,
    /// Кэш coverage-опросов: (cache.epoch, model_rev, quality) → per-clip
    /// runs. Раньше draw_timeline брал лок кэша ДВАЖДЫ на клип на кадр.
    pub coverage_cache: CoverageCache,
    /// Кэш имён/подавления имён клипов по model_rev (были 2 HashMap +
    /// O(n²) на каждый кадр).
    pub names_cache: NamesCache,
}

impl KtApp {
    fn empty_project() -> Project {
        Project::new(ProjectSettings { fps: 30, width: 1280, height: 720, sample_rate: 48000 })
    }

    fn build_player(
        project: &Project,
        base: Option<&Path>,
        index_db: Option<&Path>,
    ) -> (Player, Option<audio_out::AudioOutput>, Option<String>) {
        let mut sources = Vec::new();
        let mut err = None;
        for m in &project.media {
            match open_source_indexed(&resolve_media_path(base, &m.path), index_db) {
                Ok(s) => sources.push((m.id, s)),
                Err(e) => err = Some(format!("{}: {e}", m.path)),
            }
        }
        let (player, ring) = Player::new(project.clone(), sources);
        let audio = match audio_out::start(ring, player.shared.clone(), player.transport.clone()) {
            Ok(a) => Some(a),
            Err(e) => {
                eprintln!("аудио-вывод недоступен: {e}");
                None
            }
        };
        (player, audio, err)
    }

    pub(crate) fn new_empty(error: Option<String>) -> KtApp {
        let project = Self::empty_project();
        let project2 = project.clone();
        let (player, audio, _) = Self::build_player(&project, None, None);
        KtApp {
            model: TimelineModel::new(project),
            project_path: None,
            dirty: false,
            player,
            audio,
            peaks: Arc::new(Mutex::new(HashMap::new())),
            texture: None,
            view_start: 0.0,
            ticks_per_px: 900.0,
            selected: None,
            selected_morph: None,
            pending_morpher: None,
            drag: None,
            base_rate: 1.0,
            shuttle_override: None,
            speed_drag_upper: None,
            preview_click_at: None,
            dnd_hover: None,
            tl_geom: None,
            lane_order: Vec::new(),
            // KT_DETACH_PREVIEW=1 — старт с отделённым превью (отладка)
            preview_detached: std::env::var_os("KT_DETACH_PREVIEW").is_some(),
            ui_economy: false,
            paused_at: None,
            follow_playhead: false,
            last_user_scroll: None,
            pending_unmax_drag: false,
            tl_scroll: 0.0,
            tl_v_zoom: None,
            bin_last_good: HashMap::new(),
            bin_last_offline: std::collections::HashSet::new(),
            bin_free: true,
            bin_layout: std::collections::HashMap::new(),
            bin_drag: None,
            fader_grab: None,
            last_title: String::new(),
            preview_buf: Vec::new(),
            glow_skip: 0,
            gpu_info: None,
            snap_line: None,
            ctx_target: None,
            live_gain: None,
            status: error.map(|e| (e, std::time::Instant::now())),
            clipboard: Vec::new(),
            automation_clipboard: None,
            offline: Arc::new(Mutex::new(std::collections::HashSet::new())),
            watchdog: None,
            meter: Default::default(),
            smooth_scaling: true,
            glow_texture: None,
            preview_rect: egui::Rect::NOTHING,
            meter_glow: None,
            thumbs: crate::thumbs::ThumbService::start(&project2, None),
            fx_editors: Vec::new(),
            morph_editors: Vec::new(),
            gesture_counter: 0,
            render_job: None,
            render_setup: Default::default(),
            last_autosave: std::time::Instant::now(),
            hover_hint: None,
            timeline_rect: egui::Rect::NOTHING,
            notices: std::collections::VecDeque::new(),
            notices_unread: 0,
            notices_open: false,
            sw_render: None,
            perf: PerfState::new(),
            preview_key: None,
            incomplete_since: None,
            last_compose_retry: std::time::Instant::now(),
            thumb_textures: HashMap::new(),
            thumb_lru: std::collections::VecDeque::new(),
            model_rev: 0,
            coverage_cache: Default::default(),
            names_cache: Default::default(),
        }
    }

    fn open(path: &Path) -> Result<KtApp, String> {
        let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let project = Project::from_json(&text).map_err(|e| e.to_string())?;
        let base = path.parent();
        let (player, audio, err) =
            Self::build_player(&project, base, Some(&path.with_extension("ktindex.db")));
        let (offline, watchdog) = spawn_media_watchdog(&project, base);
        let thumbs_svc = crate::thumbs::ThumbService::start(&project, Some(path.to_path_buf()));
        let peaks = Arc::new(Mutex::new(HashMap::new()));
        let media: Vec<_> =
            project.media.iter().map(|m| (m.id, resolve_media_path(base, &m.path))).collect();
        spawn_index_worker(Some(path.with_extension("ktindex.db")), media, peaks.clone());
        let ui_state = project.ui.clone();
        let mut app = KtApp {
            model: TimelineModel::new(project),
            project_path: Some(path.to_path_buf()),
            dirty: false,
            player,
            audio,
            peaks,
            texture: None,
            view_start: 0.0,
            ticks_per_px: 900.0,
            selected: None,
            selected_morph: None,
            pending_morpher: None,
            drag: None,
            base_rate: 1.0,
            shuttle_override: None,
            speed_drag_upper: None,
            preview_click_at: None,
            dnd_hover: None,
            tl_geom: None,
            lane_order: Vec::new(),
            // KT_DETACH_PREVIEW=1 — старт с отделённым превью (отладка)
            preview_detached: std::env::var_os("KT_DETACH_PREVIEW").is_some(),
            ui_economy: false,
            paused_at: None,
            follow_playhead: false,
            last_user_scroll: None,
            pending_unmax_drag: false,
            tl_scroll: 0.0,
            tl_v_zoom: None,
            bin_last_good: HashMap::new(),
            bin_last_offline: std::collections::HashSet::new(),
            bin_free: true,
            bin_layout: std::collections::HashMap::new(),
            bin_drag: None,
            fader_grab: None,
            last_title: String::new(),
            preview_buf: Vec::new(),
            glow_skip: 0,
            gpu_info: None,
            snap_line: None,
            ctx_target: None,
            live_gain: None,
            status: err.map(|e| (e, std::time::Instant::now())),
            clipboard: Vec::new(),
            automation_clipboard: None,
            offline,
            watchdog: Some(watchdog),
            meter: Default::default(),
            smooth_scaling: true,
            glow_texture: None,
            preview_rect: egui::Rect::NOTHING,
            meter_glow: None,
            thumbs: thumbs_svc,
            fx_editors: Vec::new(),
            morph_editors: Vec::new(),
            gesture_counter: 0,
            render_job: None,
            render_setup: Default::default(),
            last_autosave: std::time::Instant::now(),
            hover_hint: None,
            timeline_rect: egui::Rect::NOTHING,
            notices: std::collections::VecDeque::new(),
            notices_unread: 0,
            notices_open: false,
            sw_render: None,
            perf: PerfState::new(),
            preview_key: None,
            incomplete_since: None,
            last_compose_retry: std::time::Instant::now(),
            thumb_textures: HashMap::new(),
            thumb_lru: std::collections::VecDeque::new(),
            model_rev: 0,
            coverage_cache: Default::default(),
            names_cache: Default::default(),
        };
        // loop-регион загруженного проекта — в плеер сразу (раньше
        // синхронизация была только в on_region_changed, т.е. только при
        // ПРАВКЕ региона — загруженный loop не зацикливался вовсе)
        {
            let p = app.model.project();
            let region = p
                .region
                .filter(|_| p.loop_enabled)
                .map(|r| kt_time::TickRange::new(r.start, r.end));
            app.player.set_loop_region(region);
        }
        // KT_AUTOPLAY=1 — сразу играть (отладка перф без ввода)
        if std::env::var_os("KT_AUTOPLAY").is_some() {
            app.follow_playhead = true;
            app.player.play();
        }
        // восстановление вьюпорта/курсора (п.13, v1: fromJson)
        if let Some(ui) = ui_state {
            // сперва tpp (единые пределы п.17), потом view_start — раньше
            // view_start клампился по ещё-дефолтному tpp
            app.ticks_per_px =
                ui.ticks_per_px.clamp(crate::timeline::TPP_MIN, crate::timeline::TPP_MAX);
            app.view_start = ui.view_start.max(-40.0 * app.ticks_per_px);
            app.player.seek(Tick(ui.cursor.max(0)));
            app.lane_order = ui.lane_order; // п.6 v1.4 (нормализуется в draw)
            app.bin_free = ui.bin_free; // п.7 р.8
            app.bin_layout =
                ui.bin_layout.iter().map(|(id, x, y)| (*id, egui::pos2(*x, *y))).collect();
        }
        Ok(app)
    }

    #[cfg(test)]
    pub(crate) fn ui_state_for_test(&self) -> kt_model::UiState {
        self.ui_state()
    }

    /// Снимок вьюпорта для записи в проект (п.13).
    fn ui_state(&self) -> kt_model::UiState {
        kt_model::UiState {
            view_start: self.view_start,
            ticks_per_px: self.ticks_per_px,
            cursor: self.player.transport.playhead().0,
            lane_order: self.lane_order.clone(),
            bin_free: self.bin_free,
            bin_layout: self.bin_layout.iter().map(|(id, p)| (*id, p.x, p.y)).collect(),
        }
    }

    /// Полная отрисовка кадра. Вынесена из `eframe::App::ui`, чтобы её
    /// мог вызывать harness-тест вёрстки (правило п.9 р.8: вёрстка и
    /// интеракции проверяются egui_kittest, не только скриншотом).
    pub(crate) fn draw_all(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        // имя проекта — в заголовок окна (п.19 р.6), не каждый кадр
        let title = format!(
            "{}{} — kt {}",
            self.project_path
                .as_ref()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .unwrap_or_else(|| "безымянный".into()),
            if self.dirty { " ●" } else { "" },
            env!("CARGO_PKG_VERSION")
        );
        if title != self.last_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_title = title;
        }
        self.handle_keys(&ctx);
        self.handle_drops(&ctx);
        self.hover_hint = None;

        // п.10 р.8: окно восстановлено из maximized прошлым кадром —
        // теперь можно начинать перетаскивание
        if self.pending_unmax_drag {
            self.pending_unmax_drag = false;
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
            Self::release_pointer_after_wm_grab(&ctx);
        }
        // статусбар (низ)
        egui::Panel::bottom("status")
            .resizable(false)
            .frame(egui::Frame::new().fill(crate::theme::PANEL).inner_margin(egui::Margin {
                left: 0,
                right: 0,
                top: 0,
                bottom: 1,
            }))
            .exact_size(20.0)
            .show(ui, |ui| {
                self.draw_statusbar(ui);
            });
        // таймлайн (низ, на всю ширину) + плавающий тулбар поверх
        egui::Panel::bottom("timeline").frame(egui::Frame::new().fill(crate::theme::PANEL)).resizable(true).default_size(300.0).show(ui, |ui| {
            let full = ui.available_rect_before_wrap();
            self.timeline_rect = full;
            let strip_h = 34.0;
            let tl_rect = egui::Rect::from_min_max(
                full.min,
                egui::pos2(full.right(), full.bottom() - strip_h),
            );
            ui.scope_builder(egui::UiBuilder::new().max_rect(tl_rect), |ui| {
                self.draw_timeline(ui);
            });
            ui.scope_builder(
                egui::UiBuilder::new().max_rect(egui::Rect::from_min_max(
                    egui::pos2(full.left(), full.bottom() - strip_h),
                    full.max,
                )),
                |ui| {
                    self.draw_toolbar_strip(ui);
                },
            );
        });
        // полоса времени над таймлайном (порт Timeline time strip)
        // левая колонка: медиа-бин + перфоманс-бокс (порт widget_3)
        egui::Panel::left("left_col").frame(crate::theme::panel_frame()).resizable(true).default_size(232.0).min_size(72.0).max_size(420.0).show(ui, |ui| {
            // шапка: лого + меню + имя проекта (пп.15,6,7 р.8)
            self.draw_column_header(ui);
            ui.add_space(2.0);
            // п.27 р.7: контент в ScrollArea — фрейм панели не перерастает
            // высоту, и сепаратор egui больше не залезает на таймлайн
            // (panel.rs берёт ФАКТИЧЕСКИЙ rect фрейма); заодно колонка
            // скроллится при малой высоте окна
            // auto_shrink по ШИРИНЕ обязателен: [false] заставлял контент
            // требовать всю доступную ширину, панель раздувалась по
            // контенту и съедала превью (регрессия п.3 р.7.1). По высоте
            // false — ради п.27 (фрейм не перерастает панель).
            egui::ScrollArea::vertical().auto_shrink([true, false]).show(ui, |ui| {
                egui::Frame::group(ui.style())
                    .fill(crate::theme::WINDOW)
                    .corner_radius(1.0)
                    .show(ui, |ui| {
                        self.draw_bin(ui);
                    });
                ui.add_space(8.0);
                egui::Frame::group(ui.style())
                    .fill(crate::theme::WINDOW)
                    .corner_radius(1.0)
                    .show(ui, |ui| {
                        self.draw_perf_box(ui);
                    });
            });
        });
        // центр: превью + метры справа, под ними контролы.
        // Фрейм без воздуха: дефолтные 8 px по периметру съедали место и
        // отодвигали кнопки окна от угла (п.15 р.8).
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(crate::theme::PANEL))
            .show(ui, |ui| {
            let p = self.model.project();
            if let Some(r) = p.region
                && !p.loop_enabled
                && self.player.transport.is_playing()
                && self.player.transport.playhead() >= r.end
                && self.player.transport.playhead() < r.end + Tick(9000)
            {
                self.player.pause();
                // как ручная пауза: двойной Space после авто-паузы = стоп
                // в начало региона (п.29 р.7)
                self.paused_at = Some(std::time::Instant::now());
            }
            egui::Panel::bottom("prev_controls").show(ui, |ui| {
                ui.add_space(3.0);
                self.draw_preview_controls(ui);
                ui.add_space(3.0);
            });
            egui::Panel::right("meters_col").resizable(true).default_size(70.0).frame(egui::Frame::new().fill(crate::theme::PANEL)).show(ui, |ui| {
                // кнопки окна — здесь, в вёрстке (п.15 р.8)
                self.draw_window_buttons(ui);
                self.draw_meters(ui);
            });
            egui::CentralPanel::default().show(ui, |ui| {
                if self.preview_detached {
                    ui.centered_and_justified(|ui| {
                        ui.weak("превью в отдельном окне (закрой его, чтобы вернуть)");
                    });
                } else {
                    self.draw_preview(ui);
                }
            });
        });

        // ---- отделённое окно превью (п.7 v1.4): второй монитор ------------
        // show_viewport_immediate: один egui::Context на все вьюпорты →
        // текстура превью работает в чужом окне без переезда; deferred не
        // годится (требует Send+Sync при &mut self).
        if self.preview_detached {
            let vp = egui::ViewportId::from_hash_of("kt-preview-window");
            ctx.show_viewport_immediate(
                vp,
                egui::ViewportBuilder::default()
                    .with_title("Kinotrain — превью")
                    .with_inner_size([960.0, 560.0]),
                |ui, _class| {
                    // input этого вьюпорта: пробел/шорткаты работают из
                    // окна; Esc здесь ТОЛЬКО возвращает превью (не трогает
                    // глобальный каскад — регион/окна главного не страдают)
                    let child_ctx = ui.ctx().clone();
                    let esc = child_ctx.input(|i| i.key_pressed(egui::Key::Escape));
                    if !esc {
                        self.handle_keys(&child_ctx);
                    }
                    // без отступов, чёрный фон — только видео (п.7 раунда 5)
                    egui::CentralPanel::default()
                        .frame(egui::Frame::NONE.fill(egui::Color32::from_rgb(8, 8, 10)))
                        .show(ui, |ui| {
                            self.draw_preview(ui);
                        });
                    // Esc в окне превью возвращает его в главное
                    if esc || child_ctx.input(|i| i.viewport().close_requested()) {
                        self.preview_detached =
                            preview_detached_next(self.preview_detached, false, true);
                    }
                },
            );
        }

        self.sample_perf();
        self.draw_fx_editors(&ctx);
        self.draw_morph_editors(&ctx);
        self.draw_render_dialog(&ctx);
        self.draw_diagnostics(&ctx);

        // автосейв каждые 4 c (порт autosaveTimer); безымянный проект —
        // в recovery-файл ~/.local/share/kinotrain/ (п.20, v1)
        if self.dirty && self.last_autosave.elapsed().as_secs() >= 4 {
            self.last_autosave = std::time::Instant::now();
            let ui = Some(self.ui_state());
            self.model.set_ui_state(ui);
            let target = self.project_path.clone().unwrap_or_else(autosave_path);
            if let Some(dir) = target.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            match std::fs::write(&target, self.model.project().to_json()) {
                Ok(()) => {
                    if self.project_path.is_some() {
                        self.dirty = false;
                    }
                }
                Err(e) => self.warn(format!("автосейв не удался: {e}")),
            }
        }

        // окно уведомлений (п.18)
        if self.notices_open {
            let mut open = self.notices_open;
            egui::Window::new("Уведомления")
                .open(&mut open)
                .default_size([460.0, 340.0])
                .min_size([320.0, 200.0])
                .resizable(true)
                .vscroll(true)
                .show(ui.ctx(), |ui| {
                    if self.notices.is_empty() {
                        ui.weak("Пока пусто");
                    }
                    for n in self.notices.iter().rev() {
                        let col = match n.severity {
                            Severity::Warn => egui::Color32::from_rgb(255, 190, 120),
                            Severity::Info => egui::Color32::from_gray(200),
                        };
                        ui.horizontal_wrapped(|ui| {
                            ui.weak(&n.at);
                            ui.colored_label(col, &n.text);
                        });
                    }
                });
            self.notices_open = open;
        }

        // ---- ресайз borderless-окна за кромки и углы (пп.11-13 р.8) ----
        // ВАЖНО: не виджет и не в начале кадра. Виджет проигрывал hit-test
        // всем панелям (egui при равном расстоянии отдаёт клик виджету,
        // зарегистрированному ПОЗЖЕ), а `drag_started` у click_and_drag
        // наступал только после сдвига >6 px — курсор к тому моменту уже
        // покидал зону, и BeginResize не уходил НИКОГДА. Теперь читаем
        // указатель напрямую в конце кадра: курсор выставляется последним
        // (побеждает), команда шлётся на самом нажатии.
        if !ctx.input(|i| i.viewport().maximized.unwrap_or(false))
            && std::env::var_os("KT_DECORATIONS").is_none()
        {
            let win = ctx.content_rect();
            if let Some(pos) = ctx.input(|i| i.pointer.hover_pos())
                && let Some(dir) = resize_zone(win, pos, WIN_EDGE, WIN_CORNER)
            {
                ctx.set_cursor_icon(resize_cursor(dir));
                if ctx.input(|i| i.pointer.primary_pressed()) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
                    Self::release_pointer_after_wm_grab(&ctx);
                }
            }
        }

        // Оверлей-свечение поверх интерфейса (п.8): превью и метры мягко
        // «светят» за границы своих панелей, не мешая чтению UI.
        if self.sw_render == Some(false) {
            let fg = ui.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("ambient-overlay"),
            ));
            // превью: размытая текстура чуть шире панели, очень низкая альфа
            if let (Some(glow), true, false) =
                (&self.glow_texture, self.preview_rect.is_finite(), self.preview_detached)
            {
                let r = self.preview_rect.expand(28.0);
                fg.image(
                    glow.id(),
                    r,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 14),
                );
            }
            // метры: ореол влево от панели, цвет по ЗОНАМ высоты (п.8);
            // правая кромка — ровно левый край панели (п.10: раньше +8 px
            // наезжали на первый бар у подписи dB)
            if let Some((r, k, loud_db)) = self.meter_glow {
                let a = (k * 22.0) as u8;
                if a > 2 {
                    let glow_rect = crate::meters::overlay_glow_rect(r);
                    // строки на границах зон: цвет зоны, альфа по k;
                    // выше уровня — затухание в прозрачность
                    let mut mesh = egui::Mesh::default();
                    let y_of = |db: f32| -> f32 {
                        let t = (db - crate::meters::METER_MIN_DB)
                            / (crate::meters::METER_MAX_DB - crate::meters::METER_MIN_DB);
                        glow_rect.bottom() - t.clamp(0.0, 1.0) * glow_rect.height()
                    };
                    let mut push_row = |y: f32, col: egui::Color32, alpha: u8| {
                        for (x, aa) in [(glow_rect.left(), 0u8), (glow_rect.right(), alpha)] {
                            mesh.vertices.push(egui::epaint::Vertex {
                                pos: egui::pos2(x, y),
                                uv: egui::epaint::WHITE_UV,
                                color: egui::Color32::from_rgba_unmultiplied(
                                    col.r(),
                                    col.g(),
                                    col.b(),
                                    aa,
                                ),
                            });
                        }
                    };
                    let stops = crate::meters::glow_stops(loud_db);
                    for &db in &stops {
                        push_row(y_of(db), crate::meters::zone_color(db), a);
                    }
                    let top_col = crate::meters::zone_color(loud_db);
                    push_row((y_of(loud_db) - 24.0).max(glow_rect.top()), top_col, 0);
                    let rows = stops.len() + 1;
                    for row in 0..rows - 1 {
                        let b = (row * 2) as u32;
                        mesh.indices.extend_from_slice(&[b, b + 1, b + 3, b, b + 3, b + 2]);
                    }
                    fg.add(egui::Shape::mesh(mesh));
                }
            }
        }

        // Перерисовка ПО ПОТРЕБНОСТИ (п.1: безусловные 60 fps + заливка
        // текстуры каждый кадр грели систему даже с пустым проектом; egui
        // и так перерисовывает мгновенно на любой ввод).
        let animating = self.player.transport.is_playing()
            || self.drag.is_some()
            || self.render_job.is_some()
            || self.meter.is_active();
        let delay = if animating {
            std::time::Duration::from_millis(if self.ui_economy { 33 } else { 16 })
        } else if self.incomplete_since.is_some() {
            std::time::Duration::from_millis(100) // ретраи неполного кадра
        } else {
            repaint_delay(false)
        };
        ctx.request_repaint_after(delay);
    }


    /// После системного драга/ресайза окна (X11 `_NET_WM_MOVERESIZE`) WM
    /// забирает указатель и ButtonReleased в приложение НЕ приходит — egui
    /// вечно считает кнопку зажатой, `dragged` держит виджет драга, и весь
    /// остальной UI лишается hovered (скролл таймлайна умирал до клика,
    /// п.28 р.7). Звать сразу после send_viewport_cmd(StartDrag/BeginResize).
    pub(crate) fn release_pointer_after_wm_grab(ctx: &egui::Context) {
        ctx.stop_dragging();
        ctx.input_mut(|i| i.pointer = Default::default());
    }

    /// Зум кнопками — якорь на PLAYHEAD (п.26 р.7: раньше якорился левый
    /// край окна). Курсор остаётся на той же экранной позиции; если он вне
    /// окна — центрируем.
    pub(crate) fn zoom_at_playhead(&mut self, factor: f64) {
        self.follow_playhead = false; // ручной зум (п.14 р.6)
        self.last_user_scroll = Some(std::time::Instant::now());
        let ph = self.player.transport.playhead().0 as f64;
        let old_tpp = self.ticks_per_px;
        let new_tpp =
            (old_tpp * factor).clamp(crate::timeline::TPP_MIN, crate::timeline::TPP_MAX);
        let w = (self.timeline_rect.width() as f64 - crate::timeline::GUTTER_W as f64).max(1.0);
        let px = (ph - self.view_start) / old_tpp; // экранная позиция курсора
        let px = if (0.0..=w).contains(&px) { px } else { w * 0.5 };
        self.view_start = ph - px * new_tpp;
        self.ticks_per_px = new_tpp;
    }

    /// Подсказка: попап + строка в правом низу статусбара (п.16 р.7 —
    /// hover_hint читает draw_statusbar). Все on_hover_text ходят сюда.
    pub(crate) fn hint(&mut self, r: egui::Response, text: &str) -> egui::Response {
        if r.hovered() {
            self.hover_hint = Some(text.to_string());
        }
        // п.14 р.8: НИКАКИХ всплывающих попапов — подсказка живёт ровно в
        // одном месте, в правом низу строки состояния
        r
    }

    /// Транспорт-машина Space/кнопки (п.15 раунда 6, тест 58; заменяет
    /// прежние правила «из паузы при регионе — всегда в начало», санкция
    /// пользователя):
    /// - играет → пауза, курсор на месте;
    /// - пауза + повторный Space в течение DOUBLE_PAUSE_MS → «стоп»:
    ///   курсор в начало региона (или 0);
    /// - пауза + Space позже → resume С ТОГО ЖЕ места; но при включённом
    ///   loop и курсоре ВНЕ региона — с начала региона (wrap транспорта
    ///   работает только изнутри региона).
    pub fn transport_toggle(
        playing: bool,
        pause_recent: bool,
        cursor: i64,
        region: Option<(i64, i64)>,
        loop_enabled: bool,
    ) -> TransportAction {
        if playing {
            return TransportAction::Pause;
        }
        let region = region.filter(|(s, e)| e > s);
        if pause_recent {
            return TransportAction::SeekStart(region.map(|(s, _)| s).unwrap_or(0));
        }
        match region {
            Some((s, e)) if loop_enabled && !(cursor >= s && cursor < e) => {
                TransportAction::Resume { seek: Some(s) }
            }
            // п.29 р.7: регион отыгран (авто-пауза оставила курсор на/за
            // концом) → Play играет регион заново; иначе resume попадал в
            // окно авто-паузы тем же кадром и «ничего не делал». Только
            // cursor >= e: игра ДО региона не трогается.
            Some((s, e)) if !loop_enabled && cursor >= e => {
                TransportAction::Resume { seek: Some(s) }
            }
            _ => TransportAction::Resume { seek: None },
        }
    }

    /// Единая точка Space/кнопки Play-Pause (обе идут сюда).
    pub fn on_transport_toggle(&mut self) {
        let p = self.model.project();
        let region = p.region.map(|r| (r.start.0, r.end.0));
        let pause_recent = self
            .paused_at
            .map(|t| t.elapsed().as_millis() < DOUBLE_PAUSE_MS)
            .unwrap_or(false);
        match Self::transport_toggle(
            self.player.transport.is_playing(),
            pause_recent,
            self.player.transport.playhead().0,
            region,
            p.loop_enabled,
        ) {
            TransportAction::Pause => {
                self.player.pause();
                self.paused_at = Some(std::time::Instant::now());
            }
            TransportAction::SeekStart(at) => {
                self.player.seek(Tick(at));
                self.paused_at = None;
            }
            TransportAction::Resume { seek } => {
                if let Some(at) = seek {
                    self.player.seek(Tick(at));
                }
                self.paused_at = None;
                self.on_play();
            }
        }
    }

    /// Запуск воспроизведения с текущей позиции (resume-семантика п.15).
    pub fn on_play(&mut self) {
        self.follow_playhead = true;
        self.player.play();
    }

    /// Регион отпущен мышью (конец драга). Во время воспроизведения курсор
    /// немедленно уводится в начало нового региона (правило «выделил во
    /// время плея — отпустил — играем регион», п.4/v1-clamp).
    pub fn on_region_released(&mut self) {
        if self.player.transport.is_playing()
            && let Some(r) = self.model.project().region
            && r.end > r.start
        {
            self.player.seek(r.start);
        }
    }

    /// Путь sidecar индекс-БД текущего проекта (None у безымянного).
    fn index_db_path(&self) -> Option<std::path::PathBuf> {
        self.project_path.as_ref().map(|p| p.with_extension("ktindex.db"))
    }

    /// Покомпонентный максимум разрешения по видео-источникам проекта
    /// (порт computeMaxSourceVideoResolution; короткоживущие пробы).
    pub fn max_source_resolution(&self) -> Option<(u32, u32)> {
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let db = self.index_db_path();
        let (mut mw, mut mh) = (0u32, 0u32);
        for m in &self.model.project().media {
            if let Ok(src) =
                open_source_indexed(&resolve_media_path(base.as_deref(), &m.path), db.as_deref())
            {
                let i = src.info();
                if i.has_video {
                    mw = mw.max(i.width);
                    mh = mh.max(i.height);
                }
            }
        }
        (mw >= 2 && mh >= 2).then_some((mw & !1, mh & !1))
    }

    /// Esc-каскад (п.11): окна → активный драг → регион+выделение.
    pub fn escape_action(&mut self) {
        // 0) отмена размещения morpher'а (v1: Esc cancels placement)
        if self.pending_morpher.take().is_some() {
            self.note("Размещение morpher'а отменено");
            return;
        }
        // 1) закрыть верхнее наше окно
        if self.render_setup.open {
            self.render_setup.open = false;
            return;
        }
        if let Some(ed) = self.fx_editors.iter_mut().rev().find(|e| e.open) {
            ed.open = false;
            return;
        }
        if let Some(ed) = self.morph_editors.iter_mut().rev().find(|e| e.open) {
            ed.open = false;
            return;
        }
        if self.notices_open {
            self.notices_open = false;
            return;
        }
        if self.perf.diag_open {
            self.perf.diag_open = false;
            return;
        }
        // 2) отменить активный драг
        if self.drag.is_some() {
            self.drag = None;
            self.snap_line = None;
            return;
        }
        // 3) снять регион и выделение
        if self.model.project().region.is_some() {
            self.clear_region();
        }
        self.selected = None;
        self.selected_morph = None;
    }

    /// Перестановка display-строк дорожек (п.6 v1.4): между строками
    /// одного КЛИП-вида содержимое перекладывается существующим
    /// reorder_lane (z-order видео соответствует экрану, аудио-шины — как
    /// раньше, вектор не меняется — лейблы V1..Vn остаются по порядку);
    /// в остальных случаях (перемешивание видов, morph) — чисто
    /// визуальный сдвиг элемента вектора.
    pub fn reorder_rows(&mut self, from: usize, to: usize) {
        if from == to || from >= self.lane_order.len() || to >= self.lane_order.len() {
            return;
        }
        let (a, b) = (self.lane_order[from], self.lane_order[to]);
        match (a.0, b.0) {
            (LaneKind::Video, LaneKind::Video) => self.reorder_lane(ClipKind::Video, a.1, b.1),
            (LaneKind::Audio, LaneKind::Audio) => self.reorder_lane(ClipKind::Audio, a.1, b.1),
            _ => {
                let e = self.lane_order.remove(from);
                self.lane_order.insert(to, e);
                self.dirty = true;
            }
        }
    }

    /// Реордер дорожки (п.7): батч MoveClip перекладывает клипы между
    /// lane `from` и `to` (сдвигом), один undo-шаг.
    pub fn reorder_lane(&mut self, kind: ClipKind, from: u32, to: u32) {
        if from == to {
            return;
        }
        let mut cmds = Vec::new();
        for c in &self.model.project().clips {
            if c.kind != kind {
                continue;
            }
            let new_lane = if c.lane == from {
                to
            } else if from < to && c.lane > from && c.lane <= to {
                c.lane - 1
            } else if from > to && c.lane >= to && c.lane < from {
                c.lane + 1
            } else {
                continue;
            };
            cmds.push(Command::MoveClip { id: c.id, new_start: c.start, new_lane });
        }
        if cmds.is_empty() {
            self.note("на этих дорожках нет событий — порядок пустых не важен");
            return;
        }
        self.apply_batch(cmds);
    }

    /// Удаление дорожки (п.8, решение пользователя: вместе с событиями,
    /// один undo-шаг): RemoveClip событий + сдвиг верхних + SetLanes−1.
    pub fn delete_lane(&mut self, kind: ClipKind, lane: u32) {
        let p = self.model.project();
        let (v, a) = (p.video_lanes.max(1), p.audio_lanes.max(1));
        let lanes_of_kind = match kind {
            ClipKind::Video => v,
            ClipKind::Audio => a,
        };
        if lanes_of_kind <= 1 {
            self.note("последнюю дорожку удалить нельзя");
            return;
        }
        let mut cmds = Vec::new();
        for c in &p.clips {
            if c.kind == kind && c.lane == lane {
                cmds.push(Command::RemoveClip { id: c.id });
            }
        }
        for c in &p.clips {
            if c.kind == kind && c.lane > lane {
                cmds.push(Command::MoveClip { id: c.id, new_start: c.start, new_lane: c.lane - 1 });
            }
        }
        let (nv, na) = match kind {
            ClipKind::Video => (v - 1, a),
            ClipKind::Audio => (v, a - 1),
        };
        cmds.push(Command::SetLanes { video: nv, audio: na });
        self.apply_batch(cmds);
    }

    pub fn note(&mut self, msg: impl Into<String>) {
        let text = msg.into();
        self.push_notice(Severity::Info, text.clone());
        self.status = Some((text, std::time::Instant::now()));
    }

    /// Warning-уведомление: в статусбар и в ретроспективу (п.18).
    pub fn warn(&mut self, msg: impl Into<String>) {
        let text = msg.into();
        self.push_notice(Severity::Warn, text.clone());
        self.status = Some((text, std::time::Instant::now()));
    }

    fn push_notice(&mut self, severity: Severity, text: String) {
        // не плодим дубликаты подряд
        if self.notices.back().map(|n| n.text == text).unwrap_or(false) {
            return;
        }
        let at = chrono_like_now();
        self.notices.push_back(Notice { at, severity, text });
        if self.notices.len() > 200 {
            self.notices.pop_front();
        }
        self.notices_unread += 1;
    }

    pub fn apply(&mut self, cmd: Command) -> bool {
        self.model_rev += 1;
        match self.model.apply(cmd) {
            Ok(()) => {
                self.player.set_project(self.model.project().clone());
                self.dirty = true;
                true
            }
            Err(e) => {
                self.note(e.to_string());
                false
            }
        }
    }

    /// Пакет команд = один undo-снапшот на жест (атомарно).
    pub fn apply_batch(&mut self, cmds: Vec<Command>) -> bool {
        self.model_rev += 1;
        match self.model.apply_batch(cmds) {
            Ok(()) => {
                self.player.set_project(self.model.project().clone());
                self.dirty = true;
                true
            }
            Err(e) => {
                self.note(e.to_string());
                false
            }
        }
    }

    // ---- операции паритета (регион/маркеры/клипборд/дорожки) ----------------

    pub fn marker_toggle_at_cursor(&mut self) {
        let at = self.player.transport.playhead();
        // магнит как в старом: удалить ближайший в пределах порога, иначе добавить
        let threshold = ((crate::timeline::SNAP_PX * self.ticks_per_px) as i64).max(900);
        let near = self
            .model
            .project()
            .markers
            .iter()
            .copied()
            .min_by_key(|m| (m.0 - at.0).abs())
            .filter(|m| (m.0 - at.0).abs() <= threshold);
        match near {
            Some(m) => self.apply(Command::RemoveMarker { at: m }),
            None => self.apply(Command::AddMarker { at }),
        };
    }

    /// Регион in/out от курсора; без региона — затравка [cursor, cursor+1с].
    pub fn set_region_edge_from_cursor(&mut self, set_start: bool) {
        let at = self.player.transport.playhead();
        let cur =
            self.model.project().region.unwrap_or(Region { start: at, end: Tick(at.0 + 90_000) });
        let r = if set_start {
            Region::normalized(at, cur.end.max(Tick(at.0 + 90)))
        } else {
            Region::normalized(cur.start.min(at), at)
        };
        self.model.set_region(r);
        self.on_region_changed();
    }

    pub fn clear_region(&mut self) {
        self.model.set_region(None);
        // п.16 раунда 6: без региона включённый loop — «лампочка горит,
        // не зацикливает» (и переживал рестарт) — гасим вместе
        self.model.set_loop_enabled(false);
        self.on_region_changed();
    }

    pub fn toggle_loop(&mut self) {
        let on = !self.model.project().loop_enabled;
        self.model.set_loop_enabled(on);
        self.on_region_changed();
    }

    /// Синхронизация региона/loop с плеером.
    pub fn on_region_changed(&mut self) {
        self.player.set_project(self.model.project().clone());
        let p = self.model.project();
        let region =
            p.region.filter(|_| p.loop_enabled).map(|r| kt_time::TickRange::new(r.start, r.end));
        self.player.set_loop_region(region);
        self.dirty = true;
    }

    pub fn copy_selection(&mut self) {
        let block = match self.model.project().region {
            Some(r) => kt_model::ops::copy_region_block(self.model.project(), r),
            None => match self.selected {
                Some(id) => kt_model::ops::copy_group(self.model.project(), id),
                None => Vec::new(),
            },
        };
        if block.is_empty() {
            self.note("нечего копировать (нет региона и выделения)");
        } else {
            self.note(format!("скопировано клипов: {}", block.len()));
            self.clipboard = block;
        }
    }

    pub fn paste_at_cursor(&mut self) {
        if self.clipboard.is_empty() {
            self.note("буфер пуст");
            return;
        }
        let at = self.player.transport.playhead();
        let mut next_clip = self.next_clip_id();
        let mut next_group = self.next_group_id();
        let (cmds, end) =
            kt_model::ops::paste_block(&self.clipboard, at, &mut next_clip, &mut next_group);
        let first = cmds.iter().find_map(|c| match c {
            Command::AddClip(ev) => Some(ev.id),
            _ => None,
        });
        if self.apply_batch(cmds) {
            self.selected = first;
            // курсор в конец блока — chain-paste (семантика старого)
            self.player.seek(end);
        }
    }

    /// S: ВСЕГДА разрез по курсору (п.5): выделенное+группа, без
    /// выделения — все события под курсором. Вырезка региона — только из
    /// контекст-меню (punch_out_action).
    pub fn cut_action(&mut self) {
        if self.selected.is_some() {
            self.split_selected();
        } else {
            self.split_all_at_cursor();
        }
    }

    /// Разрез ВСЕХ клипов, пересекающих курсор (S без выделения).
    pub fn split_all_at_cursor(&mut self) {
        let at = self.player.transport.playhead();
        let targets: Vec<ClipId> = self
            .model
            .project()
            .clips
            .iter()
            .filter(|c| {
                c.start + MIN_CLIP_DURATION <= at && at + MIN_CLIP_DURATION <= c.start + c.duration
            })
            .map(|c| c.id)
            .collect();
        if targets.is_empty() {
            self.note("под курсором нет событий");
            return;
        }
        let mut next_id = self.next_clip_id();
        let mut next_group = self.next_group_id();
        // одна новая группа на каждую старую (правые половины пар остаются парами)
        let mut group_map: std::collections::HashMap<GroupId, GroupId> =
            std::collections::HashMap::new();
        let mut cmds = Vec::new();
        #[allow(clippy::explicit_counter_loop)]
        for id in targets {
            let old_group = self.model.clip(id).and_then(|c| c.link_group);
            let new_group = old_group.map(|g| {
                *group_map.entry(g).or_insert_with(|| {
                    let ng = GroupId(next_group);
                    next_group += 1;
                    ng
                })
            });
            cmds.push(Command::Split { id, at, new_id: ClipId(next_id), new_group });
            next_id += 1;
        }
        self.apply_batch(cmds);
    }

    /// Вырезка интерьера региона (контекст-меню).
    pub fn punch_out_action(&mut self) {
        if let Some(r) = self.model.project().region {
            let mut next = self.next_clip_id();
            let cmds = kt_model::ops::punch_out_region(self.model.project(), r, &mut next);
            if cmds.is_empty() {
                self.note("регион не пересекает клипы");
            } else {
                self.apply_batch(cmds);
                self.selected = None;
            }
        }
    }

    /// Del: с регионом = удалить пересекающиеся (+партнёры), без — выделение.
    pub fn delete_action(&mut self) {
        // выделенный morph-блок — приоритет (фикс D6 v1: при нарисованном
        // регионе morpher было не удалить с клавиатуры)
        if let Some(id) = self.selected_morph {
            if self.apply(Command::RemoveMorpher { id }) {
                self.selected_morph = None;
            }
            return;
        }
        if let Some(r) = self.model.project().region {
            let cmds = kt_model::ops::delete_region(self.model.project(), r);
            if cmds.is_empty() {
                self.note("регион не пересекает клипы");
            } else {
                self.apply_batch(cmds);
                self.selected = None;
            }
        } else {
            self.delete_selected();
        }
    }

    pub fn trim_selected_start_to_cursor(&mut self) {
        let Some(sel) = self.selected else { return };
        let at = self.player.transport.playhead();
        let cmds = kt_model::ops::trim_group_start_to(self.model.project(), sel, at);
        if cmds.is_empty() {
            self.note("курсор вне выбранного клипа");
        } else {
            self.apply_batch(cmds);
        }
    }

    pub fn trim_selected_end_to_cursor(&mut self) {
        let Some(sel) = self.selected else { return };
        let at = self.player.transport.playhead();
        let cmds = kt_model::ops::trim_group_end_to(self.model.project(), sel, at);
        if cmds.is_empty() {
            self.note("курсор вне выбранного клипа");
        } else {
            self.apply_batch(cmds);
        }
    }

    /// Перенос выделенного на соседнюю дорожку своего типа (партнёр на месте).
    pub fn move_selected_lane(&mut self, delta: i32) {
        let Some(sel) = self.selected else { return };
        let Some(c) = self.model.clip(sel).cloned() else { return };
        let lanes = match c.kind {
            ClipKind::Video => self.model.project().video_lanes,
            ClipKind::Audio => self.model.project().audio_lanes,
        };
        let new_lane = (c.lane as i64 + delta as i64).clamp(0, lanes.saturating_sub(1) as i64);
        if new_lane as u32 != c.lane {
            // MoveClip двигает всю группу по времени; здесь время не меняется,
            // а lane меняется только у выбранного — семантика старого
            self.apply(Command::MoveClip {
                id: sel,
                new_start: c.start,
                new_lane: new_lane as u32,
            });
        }
    }

    pub fn add_track(&mut self, kind: ClipKind) {
        let p = self.model.project();
        let (v, a) = (p.video_lanes, p.audio_lanes);
        let cmd = match kind {
            ClipKind::Video => Command::SetLanes { video: v + 1, audio: a },
            ClipKind::Audio => Command::SetLanes { video: v, audio: a + 1 },
        };
        self.apply(cmd);
    }

    /// Магнит A/V: ближайший клип противоположного типа того же медиа
    /// в пределах 2×порога → общий старт/длительность/линк (старый magnetize).
    pub fn magnetize_selected(&mut self) {
        let Some(sel) = self.selected else {
            self.note("выбери клип");
            return;
        };
        let mut next_group = self.next_group_id();
        match kt_model::ops::magnetize_commands(
            self.model.project(),
            sel,
            Tick(90_000 * 5), // партнёр в пределах 5 c
            &mut next_group,
        ) {
            None => self.note("рядом нет события другого типа"),
            Some(cmds) if cmds.is_empty() => self.note("уже намагничены"),
            Some(cmds) => {
                if self.apply_batch(cmds) {
                    self.note("A/V связаны");
                }
            }
        }
    }

    pub fn unlink_selected(&mut self) {
        let Some(sel) = self.selected else { return };
        let group = self.model.clip(sel).and_then(|c| c.link_group);
        let ids: Vec<ClipId> = self
            .model
            .project()
            .clips
            .iter()
            .filter(|c| c.id == sel || (group.is_some() && c.link_group == group))
            .map(|c| c.id)
            .collect();
        self.apply_batch(ids.into_iter().map(|id| Command::UnlinkClip { id }).collect());
    }

    pub fn undo(&mut self) {
        self.model_rev += 1;
        if self.model.undo().is_ok() {
            self.player.set_project(self.model.project().clone());
            self.dirty = true;
        }
    }

    pub fn redo(&mut self) {
        self.model_rev += 1;
        if self.model.redo().is_ok() {
            self.player.set_project(self.model.project().clone());
            self.dirty = true;
        }
    }

    /// Меню Файл → Новый (п.3 р.7): свежий безымянный проект на месте.
    /// Старое состояние (плеер/потоки/watchdog) корректно дропается ПОСЛЕ
    /// постройки нового.
    pub fn new_project(&mut self) {
        *self = Self::new_empty(None);
    }

    /// Меню Файл → Открыть…: диалог + пересоздание приложения на месте.
    pub fn open_project_dialog(&mut self) {
        let Some(p) = rfd::FileDialog::new()
            .set_title("Открыть проект")
            .add_filter("Kinotrain project", &["ktproj"])
            .pick_file()
        else {
            return;
        };
        match Self::open(&p) {
            Ok(app) => *self = app,
            Err(e) => self.note(format!("не открылось: {e}")),
        }
    }

    /// Меню Файл → Сохранить как…: диалог принудительно (save() без
    /// project_path сам его открывает); отмена возвращает старый путь.
    pub fn save_as(&mut self) {
        let old = self.project_path.take();
        self.save();
        if self.project_path.is_none() {
            self.project_path = old; // пользователь передумал
        }
    }

    pub fn save(&mut self) {
        let path = match self.project_path.clone() {
            Some(p) => p,
            // безымянный проект: Save-As диалог (п.12 — отказ здесь был
            // «это не должно быть проблемой»)
            None => {
                let Some(p) = rfd::FileDialog::new()
                    .set_title("Сохранить проект")
                    .add_filter("Kinotrain project", &["ktproj"])
                    .set_file_name("untitled.ktproj")
                    .save_file()
                else {
                    return; // пользователь передумал
                };
                let p = if p.extension().is_none() { p.with_extension("ktproj") } else { p };
                self.project_path = Some(p.clone());
                // индекс-БД и тумбы теперь могут жить рядом с проектом
                spawn_index_worker(
                    Some(p.with_extension("ktindex.db")),
                    self.model.project().media.iter().map(|m| (m.id, m.path.clone())).collect(),
                    self.peaks.clone(),
                );
                p
            }
        };
        let ui = Some(self.ui_state());
        self.model.set_ui_state(ui);
        match std::fs::write(&path, self.model.project().to_json()) {
            Ok(()) => {
                self.dirty = false;
                self.note(format!("сохранено: {}", path.display()));
                // именованный проект сохранён — recovery-файл безымянного
                // больше не должен «воскресать» при старте (п.12)
                let _ = std::fs::remove_file(autosave_path());
            }
            Err(e) => self.note(format!("ошибка сохранения: {e}")),
        }
    }

    pub fn next_morph_id(&self) -> u64 {
        self.model.project().morphers.iter().map(|m| m.id.0).max().unwrap_or(0) + 1
    }

    /// Сбор целей morpher'а (порт collectMorpherTargetClipIds v1): регион
    /// приоритетнее выделения (клипы всех дорожек, пересекающие регион),
    /// иначе выделенный клип; в обоих случаях цели расширяются link-группами.
    pub fn morph_target_ids(&self) -> Vec<ClipId> {
        let p = self.model.project();
        let mut ids: Vec<ClipId> = match (p.region, self.selected) {
            (Some(r), _) => p.clips.iter().filter(|c| r.intersects_clip(c)).map(|c| c.id).collect(),
            (None, Some(sel)) => vec![sel],
            (None, None) => Vec::new(),
        };
        let groups: Vec<GroupId> = p
            .clips
            .iter()
            .filter(|c| ids.contains(&c.id))
            .filter_map(|c| c.link_group)
            .collect();
        for c in &p.clips {
            if let Some(g) = c.link_group
                && groups.contains(&g)
                && !ids.contains(&c.id)
            {
                ids.push(c.id);
            }
        }
        ids.sort_by_key(|i| i.0);
        ids.dedup();
        ids
    }

    /// Запуск режима размещения morpher'а (v1: beginMorpherPlacementCommon).
    /// Morph-дорожка создаётся автоматически, если её нет.
    pub fn begin_morph_placement(&mut self, batch: bool) {
        let targets = self.morph_target_ids();
        if targets.is_empty() {
            self.note("Нечего морфить: выдели регион над клипами или клип");
            return;
        }
        if self.model.project().morph_lanes == 0 {
            self.apply(Command::InsertMorphLane { at: 0 });
        }
        self.pending_morpher = Some(PendingMorph { targets, batch });
        self.note("Morpher: кликни по фиолетовой Morph-дорожке (Esc — отмена)");
    }

    /// Коммит размещения (v1: commitPendingMorpherPlacement + фикс D3:
    /// дропнутый файл становится новым источником Replace-morpher'а).
    pub fn commit_morph_placement(&mut self, lane: u32, at: Tick, drop_path: Option<String>) {
        let Some(pm) = self.pending_morpher.take() else { return };
        let id = MorphId(self.next_morph_id());
        let mut cmds = Vec::new();
        let kind = if pm.batch {
            MorphKind::EffectBatch { video_fx: Vec::new(), audio_fx: Vec::new() }
        } else {
            let media = drop_path.map(|path| {
                let mid = MediaId(self.next_media_id());
                cmds.push(Command::AddMedia { id: mid, path });
                mid
            });
            MorphKind::Replace { media, source_in_delta: Tick(0), absolute_source_in: None }
        };
        cmds.push(Command::AddMorpher(MorpherEvent {
            id,
            lane,
            start: Tick(at.0.max(0)),
            duration: Tick(90 * 1500), // v1: 1500 мс
            targets: pm.targets,
            kind,
        }));
        if self.apply_batch(cmds) {
            self.selected_morph = Some(id);
            self.selected = None;
            // v1: окно редактора открывается сразу после размещения
            self.open_morph_editor(id);
        }
    }

    pub fn next_clip_id(&self) -> u64 {
        self.model.project().clips.iter().map(|c| c.id.0).max().unwrap_or(0) + 1
    }

    pub fn next_group_id(&self) -> u64 {
        self.model
            .project()
            .clips
            .iter()
            .filter_map(|c| c.link_group.map(|g| g.0))
            .max()
            .unwrap_or(0)
            + 1
    }

    pub fn next_media_id(&self) -> u64 {
        self.model.project().media.iter().map(|m| m.id.0).max().unwrap_or(0) + 1
    }

    pub fn split_selected(&mut self) {
        let Some(sel) = self.selected else {
            self.note("нечего резать: клип не выбран");
            return;
        };
        let at = self.player.transport.playhead();
        let Some(clip) = self.model.clip(sel).cloned() else { return };
        let group = clip.link_group;
        let new_group = group.map(|_| GroupId(self.next_group_id()));
        // linked-партнёры, содержащие точку разреза, режутся тем же разрезом
        let targets: Vec<ClipId> = self
            .model
            .project()
            .clips
            .iter()
            .filter(|c| {
                (c.id == sel || (group.is_some() && c.link_group == group))
                    && c.start < at
                    && at < c.start + c.duration
            })
            .map(|c| c.id)
            .collect();
        if targets.is_empty() {
            self.note("курсор вне выбранного клипа");
            return;
        }
        let mut next_id = self.next_clip_id();
        let cmds: Vec<Command> = targets
            .into_iter()
            .map(|id| {
                let cmd = Command::Split { id, at, new_id: ClipId(next_id), new_group };
                next_id += 1;
                cmd
            })
            .collect();
        self.apply_batch(cmds);
    }

    pub fn delete_selected(&mut self) {
        let Some(sel) = self.selected else { return };
        // удаляем весь link-групп (как Del по A/V-паре в старом редакторе)
        let group = self.model.clip(sel).and_then(|c| c.link_group);
        let ids: Vec<ClipId> = self
            .model
            .project()
            .clips
            .iter()
            .filter(|c| c.id == sel || (group.is_some() && c.link_group == group))
            .map(|c| c.id)
            .collect();
        self.apply_batch(ids.into_iter().map(|id| Command::RemoveClip { id }).collect());
        self.selected = None;
    }

    pub fn add_media_clip(&mut self, path: &str) {
        let at = self.player.transport.playhead();
        self.add_media_clip_at(path, at, None);
    }

    /// Медиа + клип(ы) в конкретную позицию (дроп в позицию мыши, п.2 v1.4).
    pub fn add_media_clip_at(
        &mut self,
        path: &str,
        at: Tick,
        lane_hint: Option<(ClipKind, u32)>,
    ) {
        let media_id = MediaId(self.next_media_id());
        if !self.apply(Command::AddMedia { id: media_id, path: path.to_string() }) {
            return;
        }
        self.place_media_at(media_id, at, lane_hint);
        // Инкрементально: новый воркер на общий кэш. Пересоздание плеера
        // здесь сбрасывало ВЕСЬ прогретый кэш (п.27) и ломало дельты
        // perf-счётчиков (п.24: «10^19 evictions/с»).
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let resolved = resolve_media_path(base.as_deref(), path);
        match open_source_indexed(&resolved, self.index_db_path().as_deref()) {
            Ok(src) => self.player.add_media(media_id, src),
            Err(e) => self.note(format!("{path}: {e}")),
        }
        self.thumbs.add_media(media_id, resolved.clone());
        spawn_index_worker(
            self.project_path.as_ref().map(|p| p.with_extension("ktindex.db")),
            vec![(media_id, path.to_string())],
            self.peaks.clone(),
        );
        self.note(format!("добавлено: {path}"));
    }

    /// Клип(ы) существующего медиа на курсор (клик в бине).
    pub fn place_media_at_cursor(&mut self, media_id: MediaId) {
        let at = self.player.transport.playhead();
        self.place_media_at(media_id, at, None);
    }

    /// Клип(ы) медиа в позицию `at`; `lane_hint` — дорожка под мышкой при
    /// дропе (п.2 v1.4): её kind получает эту lane, парная половина — lane 0.
    pub fn place_media_at(
        &mut self,
        media_id: MediaId,
        at: Tick,
        lane_hint: Option<(ClipKind, u32)>,
    ) {
        let Some(path) =
            self.model.project().media.iter().find(|m| m.id == media_id).map(|m| m.path.clone())
        else {
            return;
        };
        let base = self.project_path.as_deref().and_then(|p| p.parent().map(|b| b.to_path_buf()));
        let resolved = resolve_media_path(base.as_deref(), &path);
        let info = match open_source_indexed(&resolved, self.index_db_path().as_deref()) {
            Ok(s) => s.info().clone(),
            Err(e) => {
                self.note(format!("{path}: {e}"));
                return;
            }
        };
        let dur = if info.duration.0 > 0 { info.duration } else { Tick(90_000 * 5) };
        let group = (info.has_video && info.has_audio).then(|| GroupId(self.next_group_id()));
        let mut next_id = self.next_clip_id();
        let mut cmds = Vec::new();
        for (kind, present) in
            [(ClipKind::Video, info.has_video), (ClipKind::Audio, info.has_audio)]
        {
            if !present {
                continue;
            }
            let lane = match lane_hint {
                Some((k, l)) if k == kind => l,
                _ => 0,
            };
            cmds.push(Command::AddClip(ClipEvent {
                id: ClipId(next_id),
                link_group: group,
                kind,
                lane,
                start: at,
                duration: dur,
                source_in: Tick(0),
                media: media_id,
                gain_db: 0.0,
                reversed: false,
                fx_bypass: vec![],
                fx: vec![],
            }));
            next_id += 1;
        }
        self.apply_batch(cmds);
    }

    // -- снап ---------------------------------------------------------------

    fn draw_preview(&mut self, ui: &mut egui::Ui) {
        let t = self.player.video_playhead();
        let q = self.player.preview_quality();
        // Ключ — тик, ВЫРОВНЕННЫЙ по сетке fps (тест 39): раньше ключом
        // был сырой тик — при игре он менялся каждый UI-кадр и текстура
        // перезаливалась в GPU 60 раз/с независимо от fps проекта
        // (до ~500 МБ/с на 1080p — главный источник «визуально <30 fps»).
        let grid = self.player.video_grid();
        let mut key_tick = preview_key_tick(grid, t);
        // при игре ключ не откатывается назад на мелкий джиттер latency
        // (откат больше 2 ячеек = loop-wrap/seek — пропускаем легитимно);
        // пропуск ВПЕРЁД больше чем на ячейку — считаем (V-skip, п.1 р.5)
        if self.player.transport.is_playing()
            && let Some((prev, _, _)) = self.preview_key
        {
            let step = grid.frame_step();
            if key_tick < prev && prev - key_tick <= 2 * step {
                key_tick = prev;
            } else if key_tick > prev + step {
                self.perf.video_cells_skipped += ((key_tick - prev) / step - 1) as u64;
            }
        }
        let key_now = (key_tick, (q.width, q.height));
        let need = match self.preview_key {
            Some((pt, pq, was_complete)) => pt != key_now.0 || pq != key_now.1 || !was_complete,
            None => true,
        };
        let new_frame = self.preview_key.map(|k| k.0) != Some(key_now.0);
        let mut complete = self.preview_key.map(|k| k.2).unwrap_or(false);
        if need {
            self.last_compose_retry = std::time::Instant::now();
            let mut rgba = std::mem::take(&mut self.preview_buf);
            let (cq, c) = self.player.compose_preview_into(t, &mut rgba);
            complete = c;
            // держим последний ПОЛНЫЙ кадр: неполный композит не затирает
            // картинку (мигание «чёрный кадр ↔ Загрузка», п.14/15)
            if c || self.texture.is_none() {
                // glow-мип: грубый box-даунскейл 32×18; при игре — каждый
                // 4-й новый кадр (амбиент-свечение мерцания не заметит)
                self.glow_skip = self.glow_skip.wrapping_add(1);
                if self.sw_render == Some(false) && self.glow_skip % 4 == 1 {
                    let (gw, gh) = (32usize, 18usize);
                    let mut small = vec![0u8; gw * gh * 4];
                    let (sw, sh) = (cq.width as usize, cq.height as usize);
                    for gy in 0..gh {
                        let sy = gy * sh / gh;
                        for gx in 0..gw {
                            let sx = gx * sw / gw;
                            let si = (sy * sw + sx) * 4;
                            let di = (gy * gw + gx) * 4;
                            small[di..di + 4].copy_from_slice(&rgba[si..si + 4]);
                        }
                    }
                    let gimg = egui::ColorImage::from_rgba_unmultiplied([gw, gh], &small);
                    match &mut self.glow_texture {
                        Some(t) => t.set(gimg, egui::TextureOptions::LINEAR),
                        None => {
                            self.glow_texture = Some(ui.ctx().load_texture(
                                "preview-glow",
                                gimg,
                                egui::TextureOptions::LINEAR,
                            ));
                        }
                    }
                }
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [cq.width as usize, cq.height as usize],
                    &rgba,
                );
                let opts = if self.smooth_scaling {
                    egui::TextureOptions::LINEAR
                } else {
                    egui::TextureOptions::NEAREST
                };
                match &mut self.texture {
                    Some(tex) => tex.set(image, opts),
                    None => {
                        self.texture = Some(ui.ctx().load_texture("preview", image, opts));
                    }
                }
                // честная метрика V-fps: реальные показы НОВЫХ кадров
                if new_frame {
                    self.perf.video_frames_shown += 1;
                }
            }
            self.preview_key = Some((key_now.0, key_now.1, c));
            self.preview_buf = rgba; // буфер обратно (переиспользование)
        }
        if complete {
            self.incomplete_since = None;
        } else if self.incomplete_since.is_none() {
            self.incomplete_since = Some(std::time::Instant::now());
        }
        let (rect, resp) = ui.allocate_exact_size(ui.available_size(), egui::Sense::click());
        self.preview_rect = rect;
        // пп.10-11 р.7: одиночный клик (отложенный на окно double-click) =
        // play/pause; двойной = detach/attach. Работает и в отдельном окне
        // превью (двойной клик там возвращает превью обратно).
        {
            let pending_ms = self.preview_click_at.map(|t| t.elapsed().as_millis());
            let (action, op) = preview_click_step(
                pending_ms,
                resp.clicked(),
                resp.double_clicked(),
                PREVIEW_CLICK_DELAY_MS,
            );
            match op {
                PendingOp::Arm => self.preview_click_at = Some(std::time::Instant::now()),
                PendingOp::Clear => self.preview_click_at = None,
                PendingOp::Keep => {}
            }
            match action {
                PreviewClick::TogglePlay => self.on_transport_toggle(),
                PreviewClick::ToggleDetach => {
                    self.preview_detached =
                        preview_detached_next(self.preview_detached, true, false);
                }
                PreviewClick::None => {}
            }
            if let Some(t0) = self.preview_click_at {
                // дождаться истечения окна без ввода
                let left = PREVIEW_CLICK_DELAY_MS.saturating_sub(t0.elapsed().as_millis());
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(left as u64 + 5));
            }
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(10, 10, 12));
        if let Some(tex) = &self.texture {
            let scale =
                (rect.width() / q.width as f32).min(rect.height() / q.height as f32).max(0.05);
            let size = egui::vec2(q.width as f32 * scale, q.height as f32 * scale);
            let img_rect = egui::Rect::from_center_size(rect.center(), size);
            // ambient glow как на youtube (п.18): РАЗМЫТАЯ мини-копия
            // (32×18, LINEAR-растяжка = сильный блюр) + виньетка к фону
            if self.sw_render == Some(false)
                && let Some(glow) = &self.glow_texture
            {
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                for (expand, alpha) in [(0.35f32, 40u8), (0.18, 60)] {
                    let glow_rect = egui::Rect::from_center_size(
                        img_rect.center(),
                        img_rect.size() * (1.0 + expand),
                    )
                    .intersect(rect);
                    painter.image(
                        glow.id(),
                        glow_rect,
                        uv,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha),
                    );
                }
                // виньетка: края области затухают в цвет фона
                let bg = egui::Color32::from_rgb(10, 10, 12);
                let fade = 36.0f32;
                let mut vin = egui::Mesh::default();
                let mut quad = |a: egui::Pos2,
                                b: egui::Pos2,
                                ca: egui::Color32,
                                cb: egui::Color32,
                                horiz: bool| {
                    let i = vin.vertices.len() as u32;
                    let pts = if horiz {
                        [
                            (egui::pos2(a.x, a.y), ca),
                            (egui::pos2(b.x, a.y), cb),
                            (egui::pos2(b.x, b.y), cb),
                            (egui::pos2(a.x, b.y), ca),
                        ]
                    } else {
                        [
                            (egui::pos2(a.x, a.y), ca),
                            (egui::pos2(b.x, a.y), ca),
                            (egui::pos2(b.x, b.y), cb),
                            (egui::pos2(a.x, b.y), cb),
                        ]
                    };
                    for (p, c) in pts {
                        vin.vertices.push(egui::epaint::Vertex {
                            pos: p,
                            uv: egui::epaint::WHITE_UV,
                            color: c,
                        });
                    }
                    vin.indices.extend_from_slice(&[i, i + 1, i + 2, i, i + 2, i + 3]);
                };
                let t = egui::Color32::from_rgba_unmultiplied(10, 10, 12, 0);
                quad(rect.left_top(), egui::pos2(rect.left() + fade, rect.bottom()), bg, t, true);
                quad(egui::pos2(rect.right() - fade, rect.top()), rect.right_bottom(), t, bg, true);
                quad(rect.left_top(), egui::pos2(rect.right(), rect.top() + fade), bg, t, false);
                quad(
                    egui::pos2(rect.left(), rect.bottom() - fade),
                    rect.right_bottom(),
                    t,
                    bg,
                    false,
                );
                painter.add(egui::Shape::mesh(vin));
            }
            painter.image(
                tex.id(),
                img_rect,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
            // тонкая рамка кадра
            painter.rect(
                img_rect,
                2.0,
                egui::Color32::TRANSPARENT,
                egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 24)),
                egui::StrokeKind::Outside,
            );
            // скрим — только если неполнота держится >180 мс (v1)
            let show_overlay = self
                .incomplete_since
                .map(|t0| t0.elapsed() > std::time::Duration::from_millis(180))
                .unwrap_or(false);
            if show_overlay {
                painter.rect_filled(img_rect, 2.0, egui::Color32::from_black_alpha(140));
                painter.text(
                    img_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Загрузка…",
                    egui::FontId::proportional(16.0),
                    egui::Color32::from_gray(230),
                );
            }
        }
    }
    /// Компактный контрол скорости (пп.4-5 раунда 5): точное поле основной
    /// скорости + ОДНА полоса — drag по ней = shuttle (упругий, отпустил —
    /// вернулся к основной, Vegas), double-click = 1×; маркер основной
    /// скорости виден на полосе. Маппинг полосы — slider_to_rate (тест 46).
    /// Контрол скорости (пп.18-19 р.7, решение пользователя): ДВЕ
    /// параллельные полосы в прежнем футпринте 170×20 — верхняя двигает
    /// ПОСТОЯННУЮ скорость (остаётся), нижняя — shuttle с автовозвратом.
    /// Значение при драге — оверлеем у курсора; поле ввода и правая
    /// надпись «+1.00×» убраны (очевидные надписи не делаем).
    pub(crate) fn draw_speed_control(&mut self, ui: &mut egui::Ui) {
        let (rect, resp) =
            ui.allocate_exact_size(egui::vec2(170.0, 20.0), egui::Sense::click_and_drag());
        let p = ui.painter_at(rect);
        let overriding = self.shuttle_override.is_some();
        let mid = rect.center().y;
        let upper = egui::Rect::from_min_max(rect.min, egui::pos2(rect.right(), mid));
        let lower = egui::Rect::from_min_max(egui::pos2(rect.left(), mid), rect.max);
        p.rect_filled(rect, 2.0, egui::Color32::from_gray(24));
        p.line_segment(
            [egui::pos2(rect.left(), mid), egui::pos2(rect.right(), mid)],
            egui::Stroke::new(1.0, crate::theme::SEAM),
        );
        let to_x = |t: f32| rect.left() + (t + 1.0) * 0.5 * rect.width();
        // засечки: центр (0/стоп) и ±1×, ±2× — на всю высоту
        for (t, strong) in [
            (0.0f32, true),
            (rate_to_slider(1.0), true),
            (rate_to_slider(-1.0), true),
            (1.0, false),
            (-1.0, false),
        ] {
            let x = to_x(t);
            p.line_segment(
                [
                    egui::pos2(x, rect.top() + if strong { 2.0 } else { 5.0 }),
                    egui::pos2(x, rect.bottom() - if strong { 2.0 } else { 5.0 }),
                ],
                egui::Stroke::new(1.0, egui::Color32::from_gray(if strong { 70 } else { 48 })),
            );
        }
        // полоса выбирается в кадре нажатия и держится до конца драга
        if resp.drag_started()
            && let Some(pos) = resp.interact_pointer_pos()
        {
            self.speed_drag_upper = Some(pos.y < mid);
        }
        if (resp.dragged() || resp.drag_started())
            && let Some(pos) = resp.interact_pointer_pos()
        {
            let raw = (pos.x - rect.left()) / rect.width() * 2.0 - 1.0;
            let t = raw.clamp(-1.0, 1.0);
            // ушли за пределы виджета — «второе дыхание» (п.4 р.8)
            let over_px = ((raw.abs() - 1.0) * rect.width() * 0.5).max(0.0);
            let rate_of = |t: f32| overshoot_rate(slider_to_rate(t), over_px, OVERSHOOT_PX_PER_DOUBLE);
            if self.speed_drag_upper == Some(true) {
                // верхняя полоса: постоянная скорость (остаётся)
                self.base_rate = rate_of(t);
                if self.shuttle_override.is_none() {
                    self.player.set_playback_rate(self.base_rate);
                }
            } else {
                // нижняя: временный shuttle с автовозвратом
                self.shuttle_override = Some(t);
                self.player.set_playback_rate(rate_of(t));
            }
        }
        if resp.drag_stopped() {
            self.speed_drag_upper = None;
            if self.shuttle_override.take().is_some() {
                self.player.set_playback_rate(self.base_rate);
            }
        }
        if resp.double_clicked() {
            self.base_rate = 1.0;
            self.shuttle_override = None;
            self.player.set_playback_rate(1.0);
        }
        let base_t = rate_to_slider(self.base_rate);
        let cur_t = self.shuttle_override.unwrap_or(base_t);
        // верхняя полоса: заливка от центра до base + маркер
        p.rect_filled(
            egui::Rect::from_min_max(
                egui::pos2(to_x(0.0).min(to_x(base_t)), upper.top() + 3.0),
                egui::pos2(to_x(0.0).max(to_x(base_t)), upper.bottom() - 1.0),
            ),
            1.0,
            egui::Color32::from_rgba_unmultiplied(120, 160, 255, 70),
        );
        p.line_segment(
            [
                egui::pos2(to_x(base_t), upper.top() + 1.0),
                egui::pos2(to_x(base_t), upper.bottom()),
            ],
            egui::Stroke::new(2.0, egui::Color32::from_rgba_unmultiplied(150, 180, 255, 220)),
        );
        // нижняя полоса: shuttle-заливка + кружок текущей скорости
        let (x0, x1) = (to_x(0.0).min(to_x(cur_t)), to_x(0.0).max(to_x(cur_t)));
        if overriding {
            p.rect_filled(
                egui::Rect::from_min_max(
                    egui::pos2(x0, lower.top() + 1.0),
                    egui::pos2(x1, lower.bottom() - 3.0),
                ),
                1.0,
                egui::Color32::from_rgba_unmultiplied(255, 190, 90, 90),
            );
        }
        p.circle_filled(
            egui::pos2(to_x(cur_t), lower.center().y),
            4.5,
            if overriding {
                egui::Color32::from_rgb(255, 200, 100)
            } else {
                egui::Color32::from_gray(210)
            },
        );
        // значение — оверлеем у курсора ТОЛЬКО при драге (п.19 р.7)
        if self.speed_drag_upper.is_some()
            && let Some(pos) = resp.interact_pointer_pos()
        {
            let cur = self.player.playback_rate();
            let shown =
                if self.speed_drag_upper == Some(true) { self.base_rate } else { cur };
            egui::Area::new(egui::Id::new("speed-drag-overlay"))
                .order(egui::Order::Foreground)
                .fixed_pos(pos + egui::vec2(14.0, -26.0))
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.monospace(format_rate(shown));
                    });
                });
        }
        self.hint(
            resp,
            "Верхняя полоса — постоянная скорость, нижняя — shuttle (отпустишь — вернётся), double-click — 1×",
        );
    }

    /// Контролы превью (п.17 р.7: без очевидных надписей — значения
    /// самодостаточны; п.20: Разрешение ПЕРЕД Качеством — качество есть
    /// доля разрешения проекта, движок превью видит только произведение).
    fn draw_preview_controls(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let proj = self.model.project().settings.clone();
            let cur_q = self.player.preview_quality();
            let fracs = [1.0f32 / 16.0, 1.0 / 8.0, 1.0 / 4.0, 1.0 / 2.0, 1.0];
            let mut frac_idx = {
                let ratio = cur_q.width as f32 / proj.width.max(1) as f32;
                fracs
                    .iter()
                    .enumerate()
                    .min_by(|a, b| (a.1 - ratio).abs().partial_cmp(&(b.1 - ratio).abs()).unwrap())
                    .map(|(i, _)| i)
                    .unwrap_or(2)
            };
            let q_for = |w: u32, h: u32, f: f32| kt_media::Quality {
                width: (((w as f32 * f) as u32).max(2)) & !1,
                height: (((h as f32 * f) as u32).max(2)) & !1,
            };
            let mut fps = proj.fps;
            let before_fps = fps;
            egui::ComboBox::from_id_salt("fps").selected_text(fps.to_string()).show_ui(ui, |ui| {
                for f in [24u32, 25, 30, 60] {
                    ui.selectable_value(&mut fps, f, f.to_string());
                }
            });
            if fps != before_fps {
                let mut st = proj.clone();
                st.fps = fps;
                self.apply(Command::SetSettings { settings: st });
            }
            ui.separator();
            let mut res = (proj.width, proj.height);
            let before_res = res;
            egui::ComboBox::from_id_salt("res")
                .selected_text(format!("{}×{}", res.0, res.1))
                .show_ui(ui, |ui| {
                    for r in
                        [(640u32, 360u32), (1280, 720), (1920, 1080), (2560, 1440), (3840, 2160)]
                    {
                        ui.selectable_value(&mut res, r, format!("{}×{}", r.0, r.1));
                    }
                    // «Max (W×H)» — покомпонентный максимум по источникам (v1)
                    if let Some((mw, mh)) = self.max_source_resolution() {
                        ui.selectable_value(&mut res, (mw, mh), format!("Max ({mw}×{mh})"));
                    }
                });
            if res != before_res {
                let mut st = proj.clone();
                st.width = res.0;
                st.height = res.1;
                self.apply(Command::SetSettings { settings: st });
                // качество — доля разрешения: сохранить долю, а не пиксели
                // (раньше показанная доля молча съезжала, п.20 р.7)
                self.player.set_preview_quality(q_for(res.0, res.1, fracs[frac_idx]));
            }
            ui.separator();
            let labels = ["1/16", "1/8", "1/4", "1/2", "1/1"];
            let before = frac_idx;
            egui::ComboBox::from_id_salt("quality").selected_text(labels[frac_idx]).show_ui(
                ui,
                |ui| {
                    for (i, l) in labels.iter().enumerate() {
                        ui.selectable_value(&mut frac_idx, i, *l);
                    }
                },
            );
            if frac_idx != before {
                self.player.set_preview_quality(q_for(proj.width, proj.height, fracs[frac_idx]));
            }
            ui.separator();
            ui.checkbox(&mut self.smooth_scaling, "Сглаживание");
        });
    }

    fn draw_perf_box(&mut self, ui: &mut egui::Ui) {
        // длинные строки не должны задавать минимум ширины панели (п.18 р.6)
        ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
        ui.label(egui::RichText::new("Производительность").strong());
        ui.add_space(2.0);
        let ord = std::sync::atomic::Ordering::Relaxed;
        let q = self.player.preview_quality();
        // п.15 р.7: короткие метрики — двумя колонками (экономия высоты)
        let s = self.model.project().settings.clone();
        let rows: [(String, String); 4] = [
            (
                format!(
                    "Кэш {:.0}/{:.0} МБ",
                    self.player.cache.audio_bytes() as f64 / 1e6,
                    self.player.cache.video_bytes() as f64 / 1e6
                ),
                format!(
                    "Underruns {}/{}",
                    self.player.underruns.load(ord),
                    self.player.shared.callback_underruns.load(ord)
                ),
            ),
            (
                format!("Превью {}×{}", q.width, q.height),
                format!("{} fps, {}×{}", s.fps, s.width, s.height),
            ),
            (
                format!("Клипов {}", self.model.project().clips.len()),
                format!("Медиа {}", self.model.project().media.len()),
            ),
            (
                format!(
                    "V-показано {}",
                    self.perf.video_frames_shown
                ),
                format!("V-пропущено {}", self.perf.video_cells_skipped),
            ),
        ];
        // ui.columns делит ДОСТУПНУЮ ширину пополам и не требует минимума —
        // Grid с Extend раздувал панель по контенту и съедал превью
        // (регрессия п.3 р.7.1; тот же дефект, что чинил п.18 р.6)
        ui.columns(2, |cols| {
            for c in cols.iter_mut() {
                c.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
                c.spacing_mut().item_spacing.y = 1.0;
            }
            for (a, b) in rows {
                cols[0].weak(a);
                cols[1].weak(b);
            }
        });
        if let Some(g) = &self.gpu_info {
            ui.weak(format!("GPU: {g}"));
        }
        let r = ui.checkbox(&mut self.ui_economy, "Экономный UI (30 fps при игре)");
        self.hint(r, "Для слабых GPU: вдвое меньше перерисовок при воспроизведении");
        // кумулятивы FFmpeg-слоя (порт строки avcpp open/read/vdec/adec v1)
        let (opens, reads) = kt_media::ffmpeg::io_stats();
        ui.weak(format!(
            "FFmpeg: open {opens} | read {reads} | vdec {} | adec {}",
            self.player.decode_stats.video_decodes.load(ord),
            self.player.decode_stats.audio_decodes.load(ord)
        ));
        ui.add_space(4.0);
        self.perf.quality.draw(ui, 96.0);
        ui.add_space(4.0);
        self.perf.decode.draw(ui, 96.0);
    }

    fn draw_statusbar(&mut self, ui: &mut egui::Ui) {
        // п.5 р.7: за статусбар окно ТАСКАЕТСЯ (виджеты поверх перехватят
        // свои клики; пустые зоны — драг)
        let strip = egui::Rect::from_min_size(
            ui.available_rect_before_wrap().min,
            egui::vec2(ui.available_width(), 22.0),
        );
        let bg = ui.interact(strip, ui.id().with("status-drag"), egui::Sense::click_and_drag());
        if bg.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
            Self::release_pointer_after_wm_grab(ui.ctx());
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let ord = std::sync::atomic::Ordering::Relaxed;
            ui.add_space(6.0); // п.6 р.7: колокольчик не липнет к краю
            // колокольчик уведомлений (п.18): бейдж = непрочитанные
            let bell = if self.notices_unread > 0 {
                format!("{} {}", crate::icons::BELL, self.notices_unread)
            } else {
                crate::icons::BELL.to_string()
            };
            let has_warn = self
                .notices
                .iter()
                .rev()
                .take(self.notices_unread)
                .any(|n| n.severity == Severity::Warn);
            let bell_btn = egui::Button::new(egui::RichText::new(bell).color(if has_warn {
                egui::Color32::from_rgb(255, 170, 80)
            } else if self.notices_unread > 0 {
                egui::Color32::from_rgb(160, 200, 255)
            } else {
                egui::Color32::from_gray(140)
            }))
            .frame(false);
            let bell_resp = ui.add(bell_btn);
            if self.hint(bell_resp, "Уведомления").clicked() {
                self.notices_open = !self.notices_open;
                if self.notices_open {
                    self.notices_unread = 0;
                }
            }
            ui.separator();
            // пп.21,24,25 р.7: «зум:» стёрт (значение с единицей само себя
            // объясняет), «слои» удалены, «(loop)» не пишем — горит кнопка
            ui.weak(format!("{:.0} px/s", 90_000.0 / self.ticks_per_px));
            if let Some(r) = self.model.project().region {
                ui.separator();
                ui.weak(format!(
                    "регион {:.2}–{:.2}с",
                    r.start.as_secs_f64(),
                    r.end.as_secs_f64(),
                ));
            }
            // п.22 р.7: кэш/underruns после региона
            ui.separator();
            ui.weak(format!(
                "кэш: {:.0}/{:.0} МБ",
                self.player.cache.audio_bytes() as f64 / 1e6,
                self.player.cache.video_bytes() as f64 / 1e6,
            ));
            ui.separator();
            ui.weak(format!(
                "underruns: {}/{}",
                self.player.underruns.load(ord),
                self.player.shared.callback_underruns.load(ord)
            ));
            ui.separator();
            // п.1 р.7.1: V-fps (показанные НОВЫЕ кадры) — здесь, не в тулбаре
            ui.weak(format!("{:.0} vfps", self.perf.vfps));
            // ЕДИНСТВЕННОЕ место любых подсказок (п.14 р.8): наведение →
            // активный режим → временное сообщение → дежурная строка.
            // Длинная строка спокойно затирает метрики слева — она важнее.
            let mode = if self.pending_morpher.is_some() {
                Some("Morpher: кликни по фиолетовой Morph-дорожке, куда поставить блок (Esc — отмена)")
            } else if self.dnd_hover.is_some() {
                Some("Отпусти — клип встанет под мышку (дорожка подсвечена)")
            } else {
                None
            };
            let status = self.status.as_ref().map(|(m, at)| (m.as_str(), at.elapsed().as_millis() as u64));
            let (text, kind) = status_line(
                self.hover_hint.as_deref(),
                mode,
                status,
                STATUS_TTL_MS,
                "перетащи медиафайл на окно — клип встанет под мышку",
            );
            if kind != HintKind::Status && self.status.as_ref().is_some_and(|(_, at)| at.elapsed().as_millis() as u64 >= STATUS_TTL_MS) {
                self.status = None;
            }
            let col = match kind {
                HintKind::Status => egui::Color32::from_rgb(255, 210, 120),
                HintKind::Mode => egui::Color32::from_rgb(150, 215, 175),
                _ => egui::Color32::from_gray(130),
            };
            let font = egui::FontId::proportional(12.0);
            let galley = ui.painter().layout_no_wrap(text, font, col);
            let x1 = strip.right() - 6.0;
            let plate = egui::Rect::from_min_max(
                egui::pos2(x1 - galley.size().x - 6.0, strip.top()),
                egui::pos2(x1, strip.bottom()),
            );
            ui.painter().rect_filled(plate, 0.0, crate::theme::PANEL);
            ui.painter().galley(
                egui::pos2(plate.left() + 3.0, strip.center().y - galley.size().y * 0.5),
                galley,
                col,
            );
        });
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        struct Keys {
            space: bool,
            cut: bool,
            del: bool,
            undo: bool,
            redo: bool,
            save: bool,
            marker: bool,
            region_in: bool,
            region_out: bool,
            loop_toggle: bool,
            esc: bool,
            copy: bool,
            paste: bool,
        }
        let k = ctx.input(|i| Keys {
            space: i.key_pressed(egui::Key::Space),
            cut: i.key_pressed(egui::Key::S) && !i.modifiers.ctrl,
            del: i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
            undo: i.modifiers.ctrl && !i.modifiers.shift && i.key_pressed(egui::Key::Z),
            redo: i.modifiers.ctrl
                && (i.modifiers.shift && i.key_pressed(egui::Key::Z)
                    || i.key_pressed(egui::Key::Y)),
            save: i.modifiers.ctrl && i.key_pressed(egui::Key::S),
            marker: i.key_pressed(egui::Key::M),
            region_in: i.key_pressed(egui::Key::I),
            region_out: i.key_pressed(egui::Key::O),
            loop_toggle: i.key_pressed(egui::Key::L),
            // Ctrl+C/Ctrl+V НЕ приходят как key_pressed: egui-winit
            // перехватывает их и шлёт Event::Copy / Event::Paste(..)
            // (регрессия п.21 — «Ctrl+V не работает»)
            esc: i.key_pressed(egui::Key::Escape),
            copy: i.events.iter().any(|e| matches!(e, egui::Event::Copy))
                || (i.modifiers.ctrl && i.key_pressed(egui::Key::C)),
            paste: i.events.iter().any(|e| matches!(e, egui::Event::Paste(_)))
                || (i.modifiers.ctrl && i.key_pressed(egui::Key::V)),
        });
        if k.space {
            self.on_transport_toggle();
        }
        if k.cut {
            self.cut_action();
        }
        if k.del {
            self.delete_action();
        }
        if k.undo {
            self.undo();
        }
        if k.redo {
            self.redo();
        }
        if k.save {
            self.save();
        }
        if k.marker {
            self.marker_toggle_at_cursor();
        }
        if k.region_in {
            self.set_region_edge_from_cursor(true);
        }
        if k.region_out {
            self.set_region_edge_from_cursor(false);
        }
        if k.loop_toggle {
            self.toggle_loop();
        }
        if k.esc {
            self.escape_action();
        }
        if k.copy {
            self.copy_selection();
        }
        if k.paste {
            self.paste_at_cursor();
        }
    }

    fn handle_drops(&mut self, ctx: &egui::Context) {
        // во время DnD-hover winit/X11 не шлёт движение мыши (координаты
        // XdndPosition выброшены в winit) — читаем указатель напрямую;
        // на Wayland вернётся None → честный fallback на playhead
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        let dropped: Vec<PathBuf> =
            ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).collect());
        if hovering && let Some(p) = dnd_pointer_pos(ctx) {
            self.dnd_hover = Some(p);
        }
        if hovering {
            ctx.request_repaint(); // движение мыши при DnD не шлётся — опрос
        }
        // Позиция живёт, ПОКА идёт hover, и забирается ТОЛЬКО при дропе
        // (dnd_keep_hover, тест 47): прежний take() на каждом кадре стирал
        // позицию в том же кадре, где она ставилась, — к дропу всегда None
        // (дроп падал на playhead) и силуэт не рисовался вовсе.
        if !dnd_keep_hover(hovering, !dropped.is_empty()) {
            self.dnd_hover = None;
        }
        let target = if dropped.is_empty() {
            None
        } else {
            self.dnd_hover.take().and_then(|pos| {
                let g = self.tl_geom.clone()?;
                if !g.in_lanes(pos) {
                    return None;
                }
                let lane = g.lane_at_y(pos.y)?;
                let at = self.snap(g.x_to_tick(pos.x).max(0), None);
                Some((Tick(at), lane))
            })
        };
        for path in dropped {
            if path.extension().map(|e| e == "ktproj").unwrap_or(false) {
                match KtApp::open(&path) {
                    Ok(app) => *self = app,
                    Err(e) => self.note(format!("{}: {e}", path.display())),
                }
            } else if self
                .pending_morpher
                .as_ref()
                .map(|p| !p.batch)
                .unwrap_or(false)
            {
                // дроп файла в режиме размещения Replace-morpher'а: файл
                // становится новым источником (фикс D3 v1: там путь терялся).
                // Блок ставится на первую Morph-дорожку к плейхеду.
                let at = self.player.transport.playhead();
                self.commit_morph_placement(0, at, Some(path.to_string_lossy().to_string()));
            } else if let Some((at, lane)) = target {
                self.add_media_clip_at(&path.to_string_lossy(), at, Some(lane));
            } else {
                self.add_media_clip(&path.to_string_lossy());
            }
        }
    }
}
/// Действие транспорт-машины Space (п.15 раунда 6, тест 58).
#[derive(Debug, PartialEq, Eq)]
pub enum TransportAction {
    Pause,
    Resume { seek: Option<i64> },
    SeekStart(i64),
}

/// Окно «двойного Space» = стоп (мс).
pub const STATUS_TTL_MS: u64 = 6000;
pub const DOUBLE_PAUSE_MS: u128 = 400;

/// Режим размещения morpher'а (порт pendingMorpherPlacement v1):
/// цели заморожены в момент запуска, kind выбран пунктом меню.
pub struct PendingMorph {
    pub targets: Vec<ClipId>,
    pub batch: bool,
}

/// Состояние перф-панели: rolling-графики, дельты счётчиков, журнал проблем.
/// Кэш coverage-опросов таймлайна (п.1).
#[derive(Default)]
pub struct CoverageCache {
    stamp: Option<(u64, u64, (u32, u32))>,
    /// Arc: снапшот на кадр — клон указателя, не карты (п.1 v1.4).
    pub per_clip: Arc<HashMap<ClipId, Vec<std::ops::Range<i64>>>>,
    /// Троттлинг при игре (п.1 раунда 5): epoch кэша меняется КАЖДОЙ
    /// вставкой → пересчёт шёл 60 раз/с с мьютексом кэша (контеншен с
    /// декодерами и аудио-рендером). Полоски освежаются не чаще 4 Гц.
    last_refresh: Option<std::time::Instant>,
}

/// Кэш имён клипов (п.1: HashMap'ы и O(n²) suppress пересобирались кадр).
#[derive(Default)]
pub struct NamesCache {
    rev: Option<u64>,
    /// Arc: снапшоты на кадр — клон указателя, не структур (п.1 v1.4).
    pub names: Arc<HashMap<ClipId, String>>,
    pub suppress: Arc<std::collections::HashSet<ClipId>>,
    pub media_paths: Arc<HashMap<MediaId, String>>,
}

impl KtApp {
    /// Runs покрытия per-clip; пересчёт только при смене epoch кэша,
    /// версии модели или качества превью.
    pub fn coverage_runs(&mut self) -> Arc<HashMap<ClipId, Vec<std::ops::Range<i64>>>> {
        use kt_time::AUDIO_BURST_FRAMES;
        let q = self.player.preview_quality();
        let stamp = (self.player.cache.epoch(), self.model_rev, (q.width, q.height));
        let throttled = self.player.transport.is_playing()
            && self
                .coverage_cache
                .last_refresh
                .map(|t| t.elapsed().as_millis() < 250)
                .unwrap_or(false);
        if self.coverage_cache.stamp != Some(stamp) && !throttled {
            self.coverage_cache.stamp = Some(stamp);
            self.coverage_cache.last_refresh = Some(std::time::Instant::now());
            let mut per_clip = HashMap::new();
            for c in &self.model.project().clips {
                let (kind, b0, b1) = match c.kind {
                    ClipKind::Audio => {
                        let f0 = c.source_in.to_audio_frames();
                        let f1 = (c.source_in + c.duration).to_audio_frames();
                        (
                            kt_cache::BurstKind::Audio,
                            f0.div_euclid(AUDIO_BURST_FRAMES as i64),
                            (f1 - 1).div_euclid(AUDIO_BURST_FRAMES as i64) + 1,
                        )
                    }
                    ClipKind::Video => (
                        kt_cache::BurstKind::video(Some(q)),
                        c.source_in.0,
                        c.source_in.0 + c.duration.0,
                    ),
                };
                let runs = self.player.cache.coverage(
                    kt_engine::source_id(c.media),
                    kind,
                    b0..b1.max(b0 + 1),
                );
                per_clip.insert(c.id, runs);
            }
            self.coverage_cache.per_clip = Arc::new(per_clip);
        }
        self.coverage_cache.per_clip.clone()
    }

    /// Имена клипов и множество «подавить имя» (партнёр пары) по model_rev.
    #[allow(clippy::type_complexity)]
    pub fn names(
        &mut self,
    ) -> (
        Arc<HashMap<ClipId, String>>,
        Arc<std::collections::HashSet<ClipId>>,
        Arc<HashMap<MediaId, String>>,
    ) {
        if self.names_cache.rev != Some(self.model_rev) {
            self.names_cache.rev = Some(self.model_rev);
            let mut names = HashMap::new();
            let mut suppress = std::collections::HashSet::new();
            let p = self.model.project();
            let media_names: HashMap<MediaId, String> = p
                .media
                .iter()
                .map(|m| {
                    let name = std::path::Path::new(&m.path)
                        .file_name()
                        .map(|f| f.to_string_lossy().to_string())
                        .unwrap_or_else(|| m.path.clone());
                    (m.id, name)
                })
                .collect();
            for c in &p.clips {
                if let Some(n) = media_names.get(&c.media) {
                    names.insert(c.id, n.clone());
                }
                // подавить имя у АУДИО-половины A/V-пары (рисуется на видео)
                if c.kind == ClipKind::Audio
                    && let Some(g) = c.link_group
                    && p.clips.iter().any(|o| {
                        o.kind == ClipKind::Video && o.link_group == Some(g) && o.media == c.media
                    })
                {
                    suppress.insert(c.id);
                }
            }
            self.names_cache.names = Arc::new(names);
            self.names_cache.suppress = Arc::new(suppress);
            self.names_cache.media_paths =
                Arc::new(p.media.iter().map(|m| (m.id, m.path.clone())).collect());
        }
        (
            self.names_cache.names.clone(),
            self.names_cache.suppress.clone(),
            self.names_cache.media_paths.clone(),
        )
    }
}

/// Задержка следующей перерисовки: 60 fps только когда есть что
/// анимировать, в простое — редкий тик (статусбар/watchdog).
/// Чистая функция — тест 20-repaint (п.1).
pub fn repaint_delay(animating: bool) -> std::time::Duration {
    if animating {
        std::time::Duration::from_millis(16)
    } else {
        std::time::Duration::from_millis(250)
    }
}

/// Позиция мыши в координатах окна (points) во время DnD файла.
/// winit 0.30/X11 выбрасывает координаты XdndPosition и не шлёт движение
/// мыши при перетаскивании файла (п.2 v1.4) — опрашиваем X-сервер напрямую
/// (x11rb). На Wayland/без X11 → None (fallback: дроп на playhead).
#[cfg(target_os = "linux")]
fn dnd_pointer_pos(ctx: &egui::Context) -> Option<egui::Pos2> {
    use x11rb::connection::Connection;
    use x11rb::protocol::xproto::ConnectionExt;
    static X11: std::sync::OnceLock<Option<(x11rb::rust_connection::RustConnection, u32)>> =
        std::sync::OnceLock::new();
    let conn = X11
        .get_or_init(|| {
            x11rb::connect(None)
                .ok()
                .map(|(c, screen)| {
                    let root = c.setup().roots[screen].root;
                    (c, root)
                })
        })
        .as_ref()?;
    let reply = conn.0.query_pointer(conn.1).ok()?.reply().ok()?;
    let inner = ctx.input(|i| i.viewport().inner_rect)?;
    let ppp = ctx.pixels_per_point();
    let p = egui::pos2(
        reply.root_x as f32 / ppp - inner.min.x,
        reply.root_y as f32 / ppp - inner.min.y,
    );
    (p.x >= 0.0 && p.y >= 0.0).then_some(p)
}

#[cfg(not(target_os = "linux"))]
fn dnd_pointer_pos(_ctx: &egui::Context) -> Option<egui::Pos2> {
    None
}

/// Жизнь dnd-позиции (п.2 v1.4, тест 47): держится при hover'е И на кадре
/// дропа (hovered_files к этому кадру уже пуст); гаснет только когда нет
/// ни того, ни другого.
pub fn dnd_keep_hover(hovering: bool, dropping: bool) -> bool {
    hovering || dropping
}

/// Зона ресайза borderless-окна по позиции у края (п.22 р.6, тест 64):
/// углы приоритетнее рёбер, центр → None.
/// Зона ресайза окна: ДВА радиуса (пп.11-13 р.8) — тонкая кромка и
/// заметно более крупный угол, иначе в углы невозможно попасть мышью.
pub fn resize_zone(
    win: egui::Rect,
    pos: egui::Pos2,
    edge: f32,
    corner: f32,
) -> Option<egui::ResizeDirection> {
    use egui::ResizeDirection::*;
    if !win.contains(pos) {
        return None;
    }
    let (dl, dr) = (pos.x - win.left(), win.right() - pos.x);
    let (dt, db) = (pos.y - win.top(), win.bottom() - pos.y);
    // углы приоритетнее рёбер и «ловятся» на большем расстоянии
    let (cl, cr, ct, cb) = (dl <= corner, dr <= corner, dt <= corner, db <= corner);
    if cl && ct {
        return Some(NorthWest);
    }
    if cr && ct {
        return Some(NorthEast);
    }
    if cl && cb {
        return Some(SouthWest);
    }
    if cr && cb {
        return Some(SouthEast);
    }
    Some(match (dl <= edge, dr <= edge, dt <= edge, db <= edge) {
        (true, ..) => West,
        (_, true, ..) => East,
        (_, _, true, _) => North,
        (_, _, _, true) => South,
        _ => return None,
    })
}

/// Курсор для направления ресайза.
pub fn resize_cursor(dir: egui::ResizeDirection) -> egui::CursorIcon {
    use egui::ResizeDirection::*;
    match dir {
        North | South => egui::CursorIcon::ResizeVertical,
        East | West => egui::CursorIcon::ResizeHorizontal,
        NorthEast | SouthWest => egui::CursorIcon::ResizeNeSw,
        NorthWest | SouthEast => egui::CursorIcon::ResizeNwSe,
    }
}

/// Ширина «хватаемой» кромки окна и радиус угла (п.12 р.8: 4 px требовали
/// пиксельного прицеливания).
pub const WIN_EDGE: f32 = 8.0;
pub const WIN_CORNER: f32 = 16.0;

/// State-машина деатача превью (п.7 v1.4, тест 51): закрытие окна всегда
/// возвращает превью; иначе кнопка переключает.
/// Что показывать в правом низу строки состояния (п.14 р.8). Приоритет:
/// наведение → активный режим → временное сообщение → дежурная строка.
/// Всё в одном месте: ни попапов, ни оверлеев поверх таймлайна.
#[derive(Debug, PartialEq)]
pub enum HintKind {
    Hover,
    Mode,
    Status,
    Idle,
}

/// Записать подсказку в накопитель кадра там, где `&mut self` недоступен
/// (внутри ui-замыканий панелей). Попапа НЕ создаёт (п.14 р.8).
pub fn hint_into(out: &mut Option<String>, r: egui::Response, text: &str) -> egui::Response {
    if r.hovered() {
        *out = Some(text.to_string());
    }
    r
}

pub fn status_line(
    hover: Option<&str>,
    mode: Option<&str>,
    status: Option<(&str, u64)>, // (текст, возраст в мс)
    status_ttl_ms: u64,
    idle: &str,
) -> (String, HintKind) {
    if let Some(h) = hover {
        return (h.to_string(), HintKind::Hover);
    }
    if let Some(m) = mode {
        return (m.to_string(), HintKind::Mode);
    }
    if let Some((s, age)) = status
        && age < status_ttl_ms
    {
        return (s.to_string(), HintKind::Status);
    }
    (idle.to_string(), HintKind::Idle)
}

/// Отложенный одиночный клик по превью (пп.10-11 р.7): одиночный =
/// play/pause (ждёт окно double-click), двойной = detach/attach.
/// Чистая машина — тест 74.
#[derive(Debug, PartialEq)]
pub enum PreviewClick {
    None,
    TogglePlay,
    ToggleDetach,
}

#[derive(Debug, PartialEq)]
pub enum PendingOp {
    Keep,
    Arm,
    Clear,
}

pub const PREVIEW_CLICK_DELAY_MS: u128 = 300;

pub fn preview_click_step(
    pending_ms: Option<u128>,
    clicked: bool,
    double_clicked: bool,
    delay_ms: u128,
) -> (PreviewClick, PendingOp) {
    if double_clicked {
        // двойной отменяет отложенный одиночный (play не дёргается)
        return (PreviewClick::ToggleDetach, PendingOp::Clear);
    }
    if clicked {
        return (PreviewClick::None, PendingOp::Arm);
    }
    match pending_ms {
        Some(ms) if ms >= delay_ms => (PreviewClick::TogglePlay, PendingOp::Clear),
        _ => (PreviewClick::None, PendingOp::Keep),
    }
}

pub fn preview_detached_next(detached: bool, toggle: bool, close_requested: bool) -> bool {
    if close_requested {
        false
    } else if toggle {
        !detached
    } else {
        detached
    }
}

/// Мёртвая зона слайдера скорости вокруг нуля (доля хода).
const RATE_DEAD: f32 = 0.07;
/// |rate| на краю мёртвой зоны и на краю слайдера.
const RATE_MIN: f32 = 0.25;
const RATE_MAX: f32 = 2.0;

/// «Второе дыхание» шаттла (п.4 р.8): если во время драга мышь ушла за
/// пределы виджета, скорость растёт дальше края шкалы — до ±8×. Виджет
/// визуально не меняется. `over_px` — на сколько курсор вышел за край
/// (>=0), `k` — сколько пикселей даёт удвоение.
pub fn overshoot_rate(base: f32, over_px: f32, k: f32) -> f32 {
    if over_px <= 0.0 || base == 0.0 {
        return base;
    }
    let mult = 2f32.powf(over_px / k.max(1.0));
    let out = base.abs() * mult;
    out.min(ENGINE_RATE_MAX) * base.signum()
}

/// Потолок скорости движка (кольцо и упреждение рассчитаны под него).
pub const ENGINE_RATE_MAX: f32 = kt_engine::player::ENGINE_RATE_MAX;
/// Пикселей ухода за виджет на удвоение скорости.
pub const OVERSHOOT_PX_PER_DOUBLE: f32 = 60.0;

/// Слайдер [−1..1] → скорость [−2..2]: лог по модулю, мёртвая зона у нуля
/// отдаёт ровно 0 (пп.4-5 v1.4, тест 46). Симметрично для реверса.
pub fn slider_to_rate(t: f32) -> f32 {
    let a = t.abs();
    if a < RATE_DEAD {
        // строгое: край зоны — уже RATE_MIN (round-trip, тест 46)
        return 0.0;
    }
    let x = ((a - RATE_DEAD) / (1.0 - RATE_DEAD)).clamp(0.0, 1.0);
    let r = RATE_MIN * (RATE_MAX / RATE_MIN).powf(x);
    r.copysign(t)
}

/// Обратное отображение (тест 46): rate → позиция слайдера.
pub fn rate_to_slider(rate: f32) -> f32 {
    let a = rate.abs();
    if a < RATE_MIN * 0.999 {
        return 0.0;
    }
    let x = (a.clamp(RATE_MIN, RATE_MAX) / RATE_MIN).ln() / (RATE_MAX / RATE_MIN).ln();
    (RATE_DEAD + x * (1.0 - RATE_DEAD)).copysign(rate)
}

pub fn format_rate(r: f32) -> String {
    if r == 0.0 {
        "0 (стоп)".into()
    } else {
        format!("{r:+.2}×")
    }
}

/// Ключ кадра превью: тик, выровненный по сетке fps проекта (тест 39).
pub fn preview_key_tick(grid: kt_time::FpsGrid, t: Tick) -> i64 {
    grid.align_floor(t).0
}

pub struct PerfState {
    pub quality: crate::graphs::RollingGraph,
    pub decode: crate::graphs::RollingGraph,
    pub churn: crate::graphs::RollingGraph,
    last_sample: std::time::Instant,
    // прошлые кумулятивные значения для дельт
    last_cache: kt_cache::CacheStats,
    last_decode: (u64, u64),
    last_io: (u64, u64),
    last_underruns: (u64, u64),
    frames: u32,
    /// Кумулятив реально показанных НОВЫХ видеокадров (выровненный ключ).
    pub video_frames_shown: u64,
    /// Сглаженный V-fps последнего окна замера (п.23 р.7: показывается в
    /// тайм-коде тулбара).
    pub vfps: f32,
    last_video_shown: u64,
    /// Пропущенные ячейки сетки кадров при игре (диагностика джиттера).
    pub video_cells_skipped: u64,
    pub diag_open: bool,
    pub log: std::collections::VecDeque<String>,
    /// Дедуп подряд идущих одинаковых сообщений: (текст, повторов).
    last_msg: Option<(String, u32)>,
    last_offline: usize,
    last_hit_warn: Option<std::time::Instant>,
}

impl PerfState {
    fn new() -> PerfState {
        use crate::graphs::{Axis, RollingGraph};
        PerfState {
            quality: RollingGraph::new(
                "Качество (FPS / hit%)",
                Axis::Percent,
                vec![
                    ("FPS", egui::Color32::from_rgb(120, 230, 90), 120.0),
                    // честный видео-fps: показанные НОВЫЕ кадры (п.1 v1.4;
                    // «FPS» — это ui-кадры и о видео ничего не говорит)
                    ("V-fps", egui::Color32::from_rgb(255, 145, 80), 120.0),
                    ("A-hit", egui::Color32::from_rgb(114, 214, 255), 100.0),
                    ("V-hit", egui::Color32::from_rgb(240, 190, 70), 100.0),
                ],
            ),
            // FFmpeg-активность: порт «avcpp Activity» v1 (те же 5 серий
            // и цвета), но по ЖИВЫМ счётчикам kt-ffmpeg::iostats
            decode: RollingGraph::new(
                "FFmpeg-активность (/с)",
                Axis::LinearAuto,
                vec![
                    ("Всего", egui::Color32::from_rgb(255, 145, 80), 10_000.0),
                    ("Open", egui::Color32::from_rgb(255, 205, 105), 500.0),
                    ("Read", egui::Color32::from_rgb(244, 99, 99), 10_000.0),
                    ("V-dec", egui::Color32::from_rgb(95, 209, 155), 1500.0),
                    ("A-dec", egui::Color32::from_rgb(130, 173, 255), 1500.0),
                ],
            ),
            churn: RollingGraph::new(
                "Кэш-чёрн (/с)",
                Axis::LinearAuto,
                vec![
                    ("evict", egui::Color32::from_rgb(255, 160, 120), 2000.0),
                    ("miss", egui::Color32::from_rgb(200, 160, 255), 2000.0),
                    // самая ценная метрика v1: вытеснено ДО первого чтения
                    ("no-read", egui::Color32::from_rgb(255, 90, 90), 2000.0),
                ],
            ),
            last_sample: std::time::Instant::now(),
            last_cache: Default::default(),
            last_decode: (0, 0),
            last_io: (0, 0),
            last_underruns: (0, 0),
            frames: 0,
            video_frames_shown: 0,
            vfps: 0.0,
            last_video_shown: 0,
            video_cells_skipped: 0,
            diag_open: false,
            log: std::collections::VecDeque::new(),
            last_msg: None,
            last_offline: 0,
            last_hit_warn: None,
        }
    }

    fn note(&mut self, msg: String) {
        // подряд идущие одинаковые сообщения схлопываются в «×N»
        // (в v1 всплеск забивал весь лог за секунды)
        if let Some((last, n)) = &mut self.last_msg
            && *last == msg
        {
            *n += 1;
            if let Some(line) = self.log.back_mut() {
                *line = format!("[{}] {} ×{}", chrono_like_now(), msg, n);
            }
            return;
        }
        self.last_msg = Some((msg.clone(), 1));
        let now = chrono_like_now();
        self.log.push_back(format!("[{now}] {msg}"));
        while self.log.len() > 800 {
            self.log.pop_front();
        }
    }
}

/// hh:mm:ss без зависимости chrono.
/// Файл автосохранения безымянного проекта (порт v1
/// defaultAutosaveProjectPath: ~/.local/share/kinotrain/).
pub fn autosave_path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("kinotrain").join("autosave-project.ktproj")
}

fn chrono_like_now() -> String {
    let secs =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let s = secs % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3600, s % 3600 / 60, s % 60)
}

impl KtApp {
    /// Сэмплер метрик раз в 500 мс (порт performanceTimer).
    fn sample_perf(&mut self) {
        self.perf.frames += 1;
        let dt = self.perf.last_sample.elapsed().as_secs_f32();
        if dt < 0.5 {
            return;
        }
        self.perf.last_sample = std::time::Instant::now();
        let ord = std::sync::atomic::Ordering::Relaxed;

        // saturating: пересоздание плеера обнуляет счётчики нового кэша,
        // а perf.last_* хранит кумулятивы старого — голое вычитание давало
        // u64-wrap и «10870945131100372992 evictions/с» (п.24)
        let cache = self.player.cache.stats();
        let lc = self.perf.last_cache;
        let d_ah = cache.audio_hits.saturating_sub(lc.audio_hits);
        let d_am = cache.audio_misses.saturating_sub(lc.audio_misses);
        let d_vh = cache.video_hits.saturating_sub(lc.video_hits);
        let d_vm = cache.video_misses.saturating_sub(lc.video_misses);
        let d_ev = cache.evictions.saturating_sub(lc.evictions);
        self.perf.last_cache = cache;

        let a_dec = self.player.decode_stats.audio_decodes.load(ord);
        let v_dec = self.player.decode_stats.video_decodes.load(ord);
        let (la, lv) = self.perf.last_decode;
        self.perf.last_decode = (a_dec, v_dec);
        // FFmpeg-активность: opens (demux+декодеры, вкл. reopen на seek) и
        // прочитанные пакеты (порт avcpp open/read v1)
        let (opens, reads) = kt_media::ffmpeg::io_stats();
        let (lo, lr) = self.perf.last_io;
        self.perf.last_io = (opens, reads);

        let fps = self.perf.frames as f32 / dt;
        self.perf.frames = 0;
        let a_hit =
            if d_ah + d_am > 0 { d_ah as f32 / (d_ah + d_am) as f32 * 100.0 } else { 100.0 };
        let v_hit =
            if d_vh + d_vm > 0 { d_vh as f32 / (d_vh + d_vm) as f32 * 100.0 } else { 100.0 };
        let v_shown = self.perf.video_frames_shown;
        let v_fps = v_shown.saturating_sub(self.perf.last_video_shown) as f32 / dt;
        self.perf.last_video_shown = v_shown;
        self.perf.vfps = v_fps; // п.23 р.7: тайм-код тулбара
        self.perf.quality.push(&[fps.min(120.0), v_fps.min(120.0), a_hit, v_hit]);
        let (r_open, r_read) =
            (opens.saturating_sub(lo) as f32 / dt, reads.saturating_sub(lr) as f32 / dt);
        let (r_vdec, r_adec) =
            (v_dec.saturating_sub(lv) as f32 / dt, a_dec.saturating_sub(la) as f32 / dt);
        self.perf.decode.push(&[r_open + r_read + r_vdec + r_adec, r_open, r_read, r_vdec, r_adec]);
        let d_unused = cache.unused_evictions.saturating_sub(lc.unused_evictions);
        self.perf.churn.push(&[d_ev as f32 / dt, (d_am + d_vm) as f32 / dt, d_unused as f32 / dt]);

        // журнал проблем
        let ur = self.player.underruns.load(ord);
        let uc = self.player.shared.callback_underruns.load(ord);
        let (lur, luc) = self.perf.last_underruns;
        if ur > lur {
            self.perf.note(format!("render underruns +{} (итого {ur})", ur - lur));
        }
        if uc > luc {
            self.perf.note(format!("callback underruns +{} (итого {uc})", uc - luc));
        }
        self.perf.last_underruns = (ur, uc);
        if d_ev as f32 / dt > 500.0 {
            self.perf.note(format!("всплеск eviction: {:.0}/с — кэшу тесно", d_ev as f32 / dt));
        }
        // порт главного предупреждения v1: кадры выселены ДО первого чтения
        if d_unused > 0 {
            self.perf.note(format!(
                "сброшено {d_unused} записей кэша до первого чтения (prefetch-thrash)"
            ));
        }
        // порог (в v1 порогов не было вовсе): низкий видео-hit при игре
        if v_hit < 80.0
            && self.player.transport.is_playing()
            && self
                .perf
                .last_hit_warn
                .map(|t| t.elapsed().as_secs() >= 10)
                .unwrap_or(true)
        {
            self.perf.last_hit_warn = Some(std::time::Instant::now());
            self.perf.note(format!("видео-hit {v_hit:.0}% при воспроизведении — декод не успевает"));
        }
        let offline_now = self.offline.lock().unwrap().len();
        if offline_now != self.perf.last_offline {
            self.perf.note(format!("доступность медиа изменилась: офлайн {offline_now}"));
            self.perf.last_offline = offline_now;
        }
    }

    fn draw_diagnostics(&mut self, ctx: &egui::Context) {
        if !self.perf.diag_open {
            return;
        }
        let mut open = self.perf.diag_open;
        egui::Window::new("Диагностика")
            .open(&mut open)
            .default_size([560.0, 460.0])
            .min_size([420.0, 300.0])
            .resizable(true)
            .vscroll(true)
            .show(ctx, |ui| {
                // занятость видео-кэша от НАСТОЯЩЕГО лимита (в v1 константа
                // лимита была продублирована руками и врала в 384 раза)
                let used = self.player.cache.video_bytes() as f64;
                let limit = self.player.cache.max_video_bytes() as f64;
                let occ = if limit > 0.0 { (used / limit * 100.0).min(100.0) } else { 0.0 };
                ui.horizontal(|ui| {
                    ui.weak(format!(
                        "Занятость видео-кэша: {occ:.0}% ({:.0} / {:.0} МБ)",
                        used / 1e6,
                        limit / 1e6
                    ));
                    let cache = self.player.cache.stats();
                    ui.weak(format!("| no-read drops всего: {}", cache.unused_evictions));
                });
                let (bar, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 6.0),
                    egui::Sense::hover(),
                );
                let pnt = ui.painter_at(bar);
                pnt.rect_filled(bar, 3.0, egui::Color32::from_gray(30));
                let fill_w = bar.width() * (occ as f32 / 100.0);
                let col = if occ > 90.0 {
                    egui::Color32::from_rgb(230, 120, 90)
                } else {
                    egui::Color32::from_rgb(95, 209, 155)
                };
                pnt.rect_filled(
                    egui::Rect::from_min_size(bar.min, egui::vec2(fill_w, bar.height())),
                    3.0,
                    col,
                );
                ui.add_space(6.0);
                self.perf.churn.draw(ui, 110.0);
                ui.separator();
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Журнал проблем").strong());
                    if ui.button("Очистить").clicked() {
                        self.perf.log.clear();
                        self.perf.last_msg = None;
                    }
                    if ui.button("Сброс графиков").clicked() {
                        self.perf.quality.clear();
                        self.perf.decode.clear();
                        self.perf.churn.clear();
                    }
                });
                egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                    for line in &self.perf.log {
                        ui.monospace(line);
                    }
                    if self.perf.log.is_empty() {
                        ui.weak("проблем не замечено");
                    }
                });
            });
        self.perf.diag_open = open;
    }
}

impl eframe::App for KtApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // детект софт-рендера — один раз (пп. 17, 19)
        if self.sw_render.is_none() {
            let sw = _frame
                .wgpu_render_state()
                .map(|rs| {
                    let info = rs.adapter.get_info();
                    self.gpu_info =
                        Some(format!("{} / {:?}", info.name, info.backend));
                    let name = info.name.to_lowercase();
                    info.device_type == eframe::wgpu::DeviceType::Cpu
                        || name.contains("llvmpipe")
                        || name.contains("softpipe")
                        || name.contains("swiftshader")
                })
                .unwrap_or(false);
            self.sw_render = Some(sw);
            if sw {
                self.warn(
                    "Нет аппаратного ускорения графики (софт-рендер) — интерфейс может                      подтормаживать, glow превью отключён",
                );
            }
        }

        self.draw_all(ui);
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // последний проект + финальный сейв при выходе
        if let Some(p) = &self.project_path.clone() {
            storage.set_string("last_project", p.to_string_lossy().to_string());
            let ui = Some(self.ui_state());
            self.model.set_ui_state(ui);
            let _ = std::fs::write(p, self.model.project().to_json());
        }
    }
}

pub fn run() -> eframe::Result {
    let arg = std::env::args().nth(1);
    // GPU: дефолт egui-wgpu — HighPerformance, т.е. дискретная карта; на
    // гибридной графике это PRIME-копирование каждого кадра и тормоза ВСЕЙ
    // системы (п.1 v1.4). UI-нагрузка скромная — берём энергоэффективный
    // адаптер; WGPU_POWER_PREF=high_performance возвращает дискретный.
    let mut wgpu_options = eframe::egui_wgpu::WgpuConfiguration::default();
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut wgpu_options.wgpu_setup {
        setup.power_preference = eframe::wgpu::PowerPreference::from_env()
            .unwrap_or(eframe::wgpu::PowerPreference::LowPower);
    }
    // borderless (п.22 р.6): свой титлбар в тулбар-строке; KT_DECORATIONS=1
    // возвращает системную рамку (страховка от капризов WM)
    let decorations = std::env::var_os("KT_DECORATIONS").is_some();
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1440.0, 860.0])
            .with_decorations(decorations)
            .with_title("Kinotrain"),
        wgpu_options,
        ..Default::default()
    };
    eframe::run_native(
        "kinotrain",
        options,
        Box::new(move |cc| {
            crate::theme::apply(&cc.egui_ctx);
            crate::theme::apply_debug_flags(&cc.egui_ctx); // п.23 р.6
            crate::icons::install_fonts(&cc.egui_ctx);
            let restored = arg
                .clone()
                .or_else(|| cc.storage.and_then(|s| s.get_string("last_project")))
                .map(std::path::PathBuf::from)
                .filter(|p| p.exists())
                // v1: если last_project нет — восстановление из автосейва
                .or_else(|| Some(autosave_path()).filter(|p| p.exists()));
            let app = match restored {
                Some(path) => {
                    let mut a = KtApp::open(&path).unwrap_or_else(|e| KtApp::new_empty(Some(e)));
                    if path == autosave_path() {
                        // восстановленный recovery остаётся «безымянным»
                        a.project_path = None;
                        a.note("Восстановлен автосейв безымянного проекта");
                    }
                    a
                }
                None => KtApp::new_empty(Some(
                    "Открой проект: kt-app <file.ktproj> или перетащи его на окно".into(),
                )),
            };
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod repaint_tests {
    use super::repaint_delay;

    /// Тест 58 (п.15 раунда 6; заменяет прежние правила «из паузы при
    /// регионе — всегда в начало» с санкции пользователя): транспорт-машина
    /// Space — пауза на месте; resume с места; двойной Space (в окне
    /// DOUBLE_PAUSE_MS) = стоп в начало региона/0; resume при loop и
    /// курсоре вне региона — с начала региона (wrap работает изнутри).
    #[test]
    fn t58_transport_toggle_rules() {
        use super::{KtApp, TransportAction as A};
        let toggle = KtApp::transport_toggle;
        // играет → пауза (курсор не трогаем)
        assert_eq!(toggle(true, false, 500, Some((100, 200)), true), A::Pause);
        // пауза → resume с места (регион есть, но курсор внутри/loop любой)
        assert_eq!(toggle(false, false, 150, Some((100, 200)), true), A::Resume { seek: None });
        assert_eq!(toggle(false, false, 150, Some((100, 200)), false), A::Resume { seek: None });
        assert_eq!(toggle(false, false, 5, None, false), A::Resume { seek: None });
        // loop включён, курсор ВНЕ региона → resume с начала региона
        assert_eq!(
            toggle(false, false, 500, Some((100, 200)), true),
            A::Resume { seek: Some(100) }
        );
        // t73 (п.29 р.7, санкционированная правка t58): loop выключен,
        // курсор НА/ЗА концом региона (после авто-паузы) → Play играет
        // регион заново с начала; раньше Resume{None} тем же кадром
        // попадал обратно в окно авто-паузы — «play ничего не делает»
        assert_eq!(
            toggle(false, false, 500, Some((100, 200)), false),
            A::Resume { seek: Some(100) }
        );
        assert_eq!(
            toggle(false, false, 200, Some((100, 200)), false),
            A::Resume { seek: Some(100) }
        );
        // ...но игра ДО региона не трогается (cursor < start)
        assert_eq!(toggle(false, false, 50, Some((100, 200)), false), A::Resume { seek: None });
        // двойной Space в паузе → стоп в начало региона (или 0)
        assert_eq!(toggle(false, true, 150, Some((100, 200)), false), A::SeekStart(100));
        assert_eq!(toggle(false, true, 150, None, false), A::SeekStart(0));
        // пустой регион игнорируется
        assert_eq!(toggle(false, true, 150, Some((100, 100)), true), A::SeekStart(0));
        assert_eq!(toggle(false, false, 500, Some((100, 100)), true), A::Resume { seek: None });
    }

    /// Тест 39 (п.1 v1.4): ключ превью выровнен по сетке fps — внутри
    /// одной ячейки кадра ключ стабилен (текстура не перезаливается),
    /// на границе — новый. Счётчик V-fps растёт только на новом ключе.
    #[test]
    fn t39_preview_key_is_grid_aligned() {
        use super::preview_key_tick;
        let grid = kt_time::FpsGrid::new(30); // шаг 3000 тиков
        assert_eq!(preview_key_tick(grid, kt_time::Tick(0)), 0);
        assert_eq!(preview_key_tick(grid, kt_time::Tick(1)), 0);
        assert_eq!(preview_key_tick(grid, kt_time::Tick(2999)), 0, "внутри ячейки ключ тот же");
        assert_eq!(preview_key_tick(grid, kt_time::Tick(3000)), 3000, "граница — новый ключ");
        // симуляция UI 60 fps (шаг 1500 тиков) на проекте 30 fps:
        // из 60 ui-кадров ровно 30 дают новый ключ
        let mut shown = 0u64;
        let mut prev: Option<i64> = None;
        for ui_frame in 0..60i64 {
            let key = preview_key_tick(grid, kt_time::Tick(ui_frame * 1500));
            if prev != Some(key) {
                shown += 1;
            }
            prev = Some(key);
        }
        assert_eq!(shown, 30, "V-fps считает кадры проекта, а не ui-кадры");
    }

    /// Тест 46 (пп.4-5 v1.4): маппинг слайдера скорости — симметрия по
    /// знаку, мёртвая зона отдаёт ровно 0, границы ±2, round-trip.
    #[test]
    fn t46_rate_slider_mapping() {
        use super::{rate_to_slider, slider_to_rate};
        // мёртвая зона: ровно 0 (стоп), не крошечная скорость
        for t in [0.0f32, 0.03, -0.05, 0.069, -0.069] {
            assert_eq!(slider_to_rate(t), 0.0, "t={t} в мёртвой зоне");
        }
        // границы и опорные точки
        assert!((slider_to_rate(1.0) - 2.0).abs() < 1e-5);
        assert!((slider_to_rate(-1.0) + 2.0).abs() < 1e-5);
        // симметрия знака
        for t in [0.1f32, 0.3, 0.7, 1.0] {
            assert_eq!(slider_to_rate(t), -slider_to_rate(-t));
        }
        // round-trip на представимых скоростях
        for r in [0.25f32, 0.5, 1.0, 1.5, 2.0, -0.25, -1.0, -2.0] {
            let back = slider_to_rate(rate_to_slider(r));
            assert!((back - r).abs() < 1e-4, "round-trip {r} → {back}");
        }
        // rate=0 маппится в центр слайдера
        assert_eq!(rate_to_slider(0.0), 0.0);
    }

    /// Тест 47 (дополнение, регресс п.2 раунда 5): dnd-позиция живёт при
    /// hover'е и на кадре дропа — прежний безусловный take() стирал её
    /// каждый кадр, дроп всегда падал на playhead, силуэт не рисовался.
    #[test]
    fn t47_dnd_hover_survives_until_drop() {
        use super::dnd_keep_hover;
        assert!(dnd_keep_hover(true, false), "во время hover — живёт");
        assert!(dnd_keep_hover(false, true), "на кадре дропа (hover уже пуст) — живёт");
        assert!(dnd_keep_hover(true, true));
        assert!(!dnd_keep_hover(false, false), "без hover и без дропа — гаснет");
    }

    /// Тест 74 (пп.10-11 р.7): машина отложенного клика по превью —
    /// одиночный ждёт окно double-click и даёт play/pause; двойной
    /// отменяет одиночный и переключает detach.
    #[test]
    fn t74_preview_click_machine() {
        use super::{PendingOp as O, PreviewClick as C, preview_click_step};
        // клик → взводим, действия нет
        assert_eq!(preview_click_step(None, true, false, 300), (C::None, O::Arm));
        // окно ещё не истекло → ждём
        assert_eq!(preview_click_step(Some(100), false, false, 300), (C::None, O::Keep));
        // истекло → play/pause
        assert_eq!(preview_click_step(Some(300), false, false, 300), (C::TogglePlay, O::Clear));
        assert_eq!(preview_click_step(Some(999), false, false, 300), (C::TogglePlay, O::Clear));
        // двойной клик отменяет отложенный одиночный и детачит
        assert_eq!(
            preview_click_step(Some(120), false, true, 300),
            (C::ToggleDetach, O::Clear)
        );
        // двойной без pending — тоже детач
        assert_eq!(preview_click_step(None, false, true, 300), (C::ToggleDetach, O::Clear));
        // без клика и без pending — ничего
        assert_eq!(preview_click_step(None, false, false, 300), (C::None, O::Keep));
    }

    /// Тест 81 (п.4 р.8): «второе дыхание» шаттла — за пределами виджета
    /// скорость растёт до ±8×, симметрично по знаку и монотонно.
    #[test]
    fn t81_overshoot_rate() {
        use super::{ENGINE_RATE_MAX, OVERSHOOT_PX_PER_DOUBLE as K, overshoot_rate};
        // внутри виджета множителя нет
        assert_eq!(overshoot_rate(2.0, 0.0, K), 2.0);
        assert_eq!(overshoot_rate(-2.0, -5.0, K), -2.0);
        // ровно одно удвоение
        assert!((overshoot_rate(2.0, K, K) - 4.0).abs() < 1e-4);
        assert!((overshoot_rate(-2.0, K, K) + 4.0).abs() < 1e-4, "симметрия по знаку");
        // потолок движка
        assert_eq!(overshoot_rate(2.0, K * 10.0, K), ENGINE_RATE_MAX);
        assert_eq!(overshoot_rate(-2.0, K * 10.0, K), -ENGINE_RATE_MAX);
        // ноль (стоп) остаётся нулём
        assert_eq!(overshoot_rate(0.0, K * 3.0, K), 0.0);
        // монотонность
        let mut prev = 0.0;
        for i in 0..20 {
            let r = overshoot_rate(1.0, i as f32 * 10.0, K);
            assert!(r >= prev, "не монотонно на {i}");
            prev = r;
        }
    }

    /// Тест 78 (п.14 р.8): единственная строка подсказок — приоритеты
    /// наведение → режим → временное сообщение → дежурная строка.
    #[test]
    fn t78_status_line_priority() {
        use super::{HintKind as K, STATUS_TTL_MS, status_line};
        let idle = "дежурная";
        // наведение перебивает всё
        let (t, k) = status_line(Some("hover"), Some("mode"), Some(("st", 0)), STATUS_TTL_MS, idle);
        assert_eq!((t.as_str(), k), ("hover", K::Hover));
        // режим перебивает временное сообщение
        let (t, k) = status_line(None, Some("mode"), Some(("st", 0)), STATUS_TTL_MS, idle);
        assert_eq!((t.as_str(), k), ("mode", K::Mode));
        // временное сообщение живёт до TTL
        let (t, k) = status_line(None, None, Some(("st", STATUS_TTL_MS - 1)), STATUS_TTL_MS, idle);
        assert_eq!((t.as_str(), k), ("st", K::Status));
        // ...и уступает дежурной строке после
        let (t, k) = status_line(None, None, Some(("st", STATUS_TTL_MS)), STATUS_TTL_MS, idle);
        assert_eq!((t.as_str(), k), (idle, K::Idle));
        // пусто → дежурная
        let (t, k) = status_line(None, None, None, STATUS_TTL_MS, idle);
        assert_eq!((t.as_str(), k), (idle, K::Idle));
    }

    /// Тест 77 (пп.11-13 р.8, расширяет t64): зоны ресайза окна с ДВУМЯ
    /// радиусами — углы «ловятся» на большем расстоянии и приоритетнее
    /// рёбер (раньше угол был 4×4 px и в него невозможно было попасть).
    #[test]
    fn t77_resize_zones_edge_and_corner() {
        use super::{WIN_CORNER, WIN_EDGE, resize_zone};
        use eframe::egui::{Rect, ResizeDirection::*, pos2};
        let w = Rect::from_min_max(pos2(0.0, 0.0), pos2(400.0, 300.0));
        let z = |x: f32, y: f32| resize_zone(w, pos2(x, y), WIN_EDGE, WIN_CORNER);
        // углы — по радиусу WIN_CORNER, даже вдали от самой кромки
        assert_eq!(z(12.0, 12.0), Some(NorthWest));
        assert_eq!(z(389.0, 14.0), Some(NorthEast));
        assert_eq!(z(10.0, 290.0), Some(SouthWest));
        assert_eq!(z(392.0, 288.0), Some(SouthEast));
        // рёбра — по радиусу WIN_EDGE
        assert_eq!(z(3.0, 150.0), Some(West));
        assert_eq!(z(397.0, 150.0), Some(East));
        assert_eq!(z(200.0, 3.0), Some(North));
        assert_eq!(z(200.0, 297.0), Some(South));
        // за кромкой ребра, но ещё не в углу — ничего
        assert_eq!(z(200.0, 150.0), None, "центр");
        assert_eq!(z(30.0, 150.0), None, "в стороне от кромки");
        // точные границы
        assert_eq!(z(WIN_EDGE, 150.0), Some(West));
        assert_eq!(z(WIN_EDGE + 0.1, 150.0), None);
        assert_eq!(z(WIN_CORNER, WIN_CORNER), Some(NorthWest));
        assert_eq!(z(WIN_CORNER + 0.1, WIN_CORNER + 0.1), None);
        // вне окна
        assert_eq!(z(-1.0, 150.0), None);
    }

    /// Тест 51 (п.7 v1.4): state-машина деатача превью — закрытие окна
    /// всегда возвращает превью в главное, кнопка переключает.
    #[test]
    fn t51_preview_detach_state() {
        use super::preview_detached_next as next;
        assert!(next(false, true, false), "кнопка отделяет");
        assert!(!next(true, true, false), "кнопка возвращает");
        assert!(!next(true, false, true), "закрытие окна возвращает");
        assert!(!next(true, true, true), "закрытие сильнее кнопки");
        assert!(!next(false, false, false), "покой");
        assert!(next(true, false, false), "остаётся отделённым");
    }

    /// Тест 20-repaint (п.1): в простое НЕ 60 fps.
    #[test]
    fn t20_idle_repaint_is_slow_playing_is_60fps() {
        assert_eq!(repaint_delay(true).as_millis(), 16);
        assert!(repaint_delay(false).as_millis() >= 100, "простой = редкий тик");
    }
}
